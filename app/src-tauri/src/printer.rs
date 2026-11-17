//! One MQTT connection per Bambu printer, over TLS with the printer's certificate pinned on first sight.
//! The access code lives in the login Keychain. It never reaches a file, a log or the interface.

use crate::AppState;
use binder_core::config::PrinterConfig;
use binder_core::printer::{self, PinCheck, PrinterCommand, PrinterState};
use rumqttc::{AsyncClient, ConnectionError, Event, MqttOptions, Packet, QoS, TlsConfiguration, Transport};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, SignatureScheme};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

pub const KEYCHAIN_SERVICE: &str = "io.github.kungie.binder.printer";
const CHANGED: &str = "The printer's certificate changed. Remove the printer and add it again if you trust it.";

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PrinterView {
    pub config: PrinterConfig,
    pub state: PrinterState,
    pub online: bool,
    pub error: Option<String>,
}

struct Live {
    view: PrinterView,
    client: Option<AsyncClient>,
    task: Option<tauri::async_runtime::JoinHandle<()>>,
    seq: u32,
}

#[derive(Default)]
pub struct Printers(Mutex<HashMap<String, Live>>);

impl Printers {
    pub fn view(&self, serial: &str) -> Option<PrinterView> {
        self.0.lock().unwrap().get(serial).map(|l| l.view.clone())
    }

    fn client(&self, serial: &str) -> Option<(AsyncClient, u32)> {
        let mut map = self.0.lock().unwrap();
        let live = map.get_mut(serial)?;
        live.seq = live.seq.wrapping_add(1);
        Some((live.client.clone()?, live.seq))
    }

    pub fn stop(&self, serial: &str) {
        if let Some(live) = self.0.lock().unwrap().remove(serial) {
            if let Some(task) = live.task {
                task.abort();
            }
        }
    }
}

/// Trust on first use: the first certificate seen is pinned, any other one after that is refused.
#[derive(Debug)]
struct Pin {
    pinned: Mutex<Option<String>>,
    /// A fingerprint pinned during this connection, waiting to be saved to the config.
    fresh: Mutex<Option<String>>,
    changed: AtomicBool,
}

impl ServerCertVerifier for Pin {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _name: &ServerName<'_>,
        _ocsp: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        let mut pinned = self.pinned.lock().unwrap();
        match printer::cert_matches(pinned.as_deref(), end_entity.as_ref()) {
            PinCheck::Matches => Ok(ServerCertVerified::assertion()),
            PinCheck::New(hash) => {
                *pinned = Some(hash.clone());
                *self.fresh.lock().unwrap() = Some(hash);
                Ok(ServerCertVerified::assertion())
            }
            PinCheck::Mismatch => {
                self.changed.store(true, Ordering::SeqCst);
                Err(rustls::Error::General(CHANGED.into()))
            }
        }
    }

    fn verify_tls12_signature(&self, message: &[u8], cert: &CertificateDer<'_>, dss: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_signed(cert.as_ref(), dss.scheme, message, dss.signature())
    }

    fn verify_tls13_signature(&self, message: &[u8], cert: &CertificateDer<'_>, dss: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, rustls::Error> {
        // TLS 1.3 forbids PKCS#1 v1.5 signatures in the handshake.
        if matches!(dss.scheme, SignatureScheme::RSA_PKCS1_SHA256 | SignatureScheme::RSA_PKCS1_SHA384 | SignatureScheme::RSA_PKCS1_SHA512) {
            return Err(rustls::Error::PeerMisbehaved(rustls::PeerMisbehaved::SignedHandshakeWithUnadvertisedSigScheme));
        }
        verify_signed(cert.as_ref(), dss.scheme, message, dss.signature())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        SCHEMES.iter().map(|(s, _, _)| *s).collect()
    }
}

/// The key types a printer certificate can carry.
#[derive(Debug, PartialEq, Clone, Copy)]
enum KeyKind {
    Rsa,
    EcP256,
    EcP384,
}

/// The handshake signatures Binder checks, each with the key it needs and ring's algorithm for it.
const SCHEMES: &[(SignatureScheme, KeyKind, &dyn ring::signature::VerificationAlgorithm)] = &[
    (SignatureScheme::RSA_PSS_SHA256, KeyKind::Rsa, &ring::signature::RSA_PSS_2048_8192_SHA256),
    (SignatureScheme::RSA_PSS_SHA384, KeyKind::Rsa, &ring::signature::RSA_PSS_2048_8192_SHA384),
    (SignatureScheme::RSA_PSS_SHA512, KeyKind::Rsa, &ring::signature::RSA_PSS_2048_8192_SHA512),
    (SignatureScheme::RSA_PKCS1_SHA256, KeyKind::Rsa, &ring::signature::RSA_PKCS1_2048_8192_SHA256),
    (SignatureScheme::RSA_PKCS1_SHA384, KeyKind::Rsa, &ring::signature::RSA_PKCS1_2048_8192_SHA384),
    (SignatureScheme::RSA_PKCS1_SHA512, KeyKind::Rsa, &ring::signature::RSA_PKCS1_2048_8192_SHA512),
    (SignatureScheme::ECDSA_NISTP256_SHA256, KeyKind::EcP256, &ring::signature::ECDSA_P256_SHA256_ASN1),
    (SignatureScheme::ECDSA_NISTP384_SHA384, KeyKind::EcP384, &ring::signature::ECDSA_P384_SHA384_ASN1),
];

/// Checks a handshake signature against the certificate's key. Bambu printers send X.509 version 1
/// certificates, which webpki (and so rustls's own check) refuses, so the key is read here instead.
fn verify_signed(cert: &[u8], scheme: SignatureScheme, message: &[u8], signature: &[u8]) -> Result<HandshakeSignatureValid, rustls::Error> {
    let (kind, key) = public_key(cert).ok_or(rustls::Error::InvalidCertificate(rustls::CertificateError::BadEncoding))?;
    let (_, want, alg) = SCHEMES
        .iter()
        .find(|(s, _, _)| *s == scheme)
        .ok_or(rustls::Error::PeerMisbehaved(rustls::PeerMisbehaved::SignedHandshakeWithUnadvertisedSigScheme))?;
    if *want != kind {
        return Err(rustls::Error::InvalidCertificate(rustls::CertificateError::BadSignature));
    }
    ring::signature::UnparsedPublicKey::new(*alg, key)
        .verify(message, signature)
        .map(|_| HandshakeSignatureValid::assertion())
        .map_err(|_| rustls::Error::InvalidCertificate(rustls::CertificateError::BadSignature))
}

/// One DER element: its tag, its contents and what follows it.
fn der(input: &[u8]) -> Option<(u8, &[u8], &[u8])> {
    let (&tag, rest) = input.split_first()?;
    let (&first, rest) = rest.split_first()?;
    let (len, rest) = if first < 0x80 {
        (first as usize, rest)
    } else {
        let n = (first & 0x7f) as usize;
        if n == 0 || n > 4 || rest.len() < n {
            return None;
        }
        (rest[..n].iter().fold(0usize, |l, b| (l << 8) | *b as usize), &rest[n..])
    };
    (rest.len() >= len).then(|| (tag, &rest[..len], &rest[len..]))
}

const RSA: &[u8] = &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x01];
const EC: &[u8] = &[0x2a, 0x86, 0x48, 0xce, 0x3d, 0x02, 0x01];
const P256: &[u8] = &[0x2a, 0x86, 0x48, 0xce, 0x3d, 0x03, 0x01, 0x07];
const P384: &[u8] = &[0x2b, 0x81, 0x04, 0x00, 0x22];

/// The public key in a certificate of any X.509 version: its kind and the bytes ring takes
/// (an RSAPublicKey for RSA, the uncompressed point for EC).
fn public_key(cert: &[u8]) -> Option<(KeyKind, &[u8])> {
    let (0x30, cert, _) = der(cert)? else { return None };
    let (0x30, tbs, _) = der(cert)? else { return None };
    let mut rest = tbs;
    // Version 1 has no [0] version field; later versions do.
    if let Some((0xa0, _, after)) = der(rest) {
        rest = after;
    }
    // serialNumber, signature, issuer, validity, subject
    for _ in 0..5 {
        rest = der(rest)?.2;
    }
    let (0x30, spki, _) = der(rest)? else { return None };
    let (0x30, alg, spki) = der(spki)? else { return None };
    let (0x03, bits, _) = der(spki)? else { return None };
    let (&0, key) = bits.split_first()? else { return None };
    let (0x06, oid, params) = der(alg)? else { return None };
    let kind = if oid == RSA {
        KeyKind::Rsa
    } else if oid == EC {
        match der(params)? {
            (0x06, curve, _) if curve == P256 => KeyKind::EcP256,
            (0x06, curve, _) if curve == P384 => KeyKind::EcP384,
            _ => return None,
        }
    } else {
        return None;
    };
    Some((kind, key))
}

pub fn keychain(serial: &str) -> keyring::Result<keyring::Entry> {
    keyring::Entry::new(KEYCHAIN_SERVICE, serial)
}

/// Changes one printer's view and tells the interface: only when something changed, and while only temperatures
/// and progress move, at most every 2 seconds (a printing Bambu reports several times a second).
fn report(app: &AppHandle, serial: &str, change: impl FnOnce(&mut PrinterView)) {
    static SENT: std::sync::Mutex<Option<std::collections::HashMap<String, std::time::Instant>>> = std::sync::Mutex::new(None);
    let view = {
        let state = app.state::<AppState>();
        let mut map = state.printers.0.lock().unwrap();
        let Some(live) = map.get_mut(serial) else { return };
        let before = live.view.clone();
        change(&mut live.view);
        let v = &live.view;
        if *v == before {
            return;
        }
        let big = v.online != before.online || v.error != before.error || v.state.gcode_state != before.state.gcode_state;
        let mut sent = SENT.lock().unwrap_or_else(|e| e.into_inner());
        let sent = sent.get_or_insert_with(Default::default);
        let now = std::time::Instant::now();
        if !big && sent.get(serial).is_some_and(|t| now.duration_since(*t) < std::time::Duration::from_secs(2)) {
            return;
        }
        sent.insert(serial.to_string(), now);
        live.view.clone()
    };
    let _ = app.emit("printer://state", view);
}

fn save_pin(app: &AppHandle, serial: &str, hash: String) {
    let state = app.state::<AppState>();
    let mut config = state.config.lock().unwrap();
    if let Some(p) = config.printers.iter_mut().find(|p| p.serial == serial) {
        p.cert_sha256 = Some(hash);
        let _ = config.save(&state.config_path);
    }
}

/// Starts (or restarts) the worker for one printer.
pub fn start(app: &AppHandle, config: PrinterConfig) {
    let serial = config.serial.clone();
    let state = app.state::<AppState>();
    state.printers.stop(&serial);
    let view = PrinterView { config: config.clone(), state: PrinterState::default(), online: false, error: None };
    state.printers.0.lock().unwrap().insert(serial.clone(), Live { view, client: None, task: None, seq: 0 });
    let task = tauri::async_runtime::spawn(run(app.clone(), config));
    if let Some(live) = state.printers.0.lock().unwrap().get_mut(&serial) {
        live.task = Some(task);
    }
}

async fn run(app: AppHandle, config: PrinterConfig) {
    let serial = config.serial.clone();
    let Ok(code) = keychain(&serial).and_then(|e| e.get_password()) else {
        report(&app, &serial, |v| v.error = Some("No access code in the Keychain. Remove the printer and add it again.".into()));
        return;
    };
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let pin = Arc::new(Pin { pinned: Mutex::new(config.cert_sha256.clone()), fresh: Mutex::new(None), changed: AtomicBool::new(false) });
    let tls = match rustls::ClientConfig::builder_with_provider(provider).with_safe_default_protocol_versions() {
        Ok(b) => b.dangerous().with_custom_certificate_verifier(pin.clone()).with_no_client_auth(),
        Err(e) => return report(&app, &serial, |v| v.error = Some(format!("TLS setup failed: {e}"))),
    };
    let mut opts = MqttOptions::new(format!("binder-{}", std::process::id()), config.address.clone(), 8883);
    opts.set_credentials("bblp", code)
        .set_keep_alive(Duration::from_secs(30))
        .set_max_packet_size(1 << 20, 1 << 16)
        .set_transport(Transport::tls_with_config(TlsConfiguration::Rustls(Arc::new(tls))));
    let (client, mut events) = AsyncClient::new(opts, 10);
    if let Some(live) = app.state::<AppState>().printers.0.lock().unwrap().get_mut(&serial) {
        live.client = Some(client.clone());
    }

    let mut failures = 0u32;
    let mut prints = printer::PrintWatch::default();
    loop {
        match events.poll().await {
            Ok(Event::Incoming(Packet::ConnAck(_))) => {
                failures = 0;
                if let Some(hash) = pin.fresh.lock().unwrap().take() {
                    save_pin(&app, &serial, hash);
                }
                let _ = client.subscribe(printer::report_topic(&serial), QoS::AtMostOnce).await;
                let _ = client.publish(printer::request_topic(&serial), QoS::AtMostOnce, false, printer::command(0, PrinterCommand::PushAll)).await;
                report(&app, &serial, |v| (v.online, v.error) = (true, None));
            }
            Ok(Event::Incoming(Packet::Publish(p))) => {
                if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&p.payload) {
                    let mut outcome = None;
                    report(&app, &serial, |v| {
                        v.state.merge(&json);
                        v.online = true;
                        outcome = prints.seen(&v.state, chrono::Utc::now().timestamp());
                    });
                    if let Some(o) = outcome {
                        print_ended(&app, &o);
                    }
                }
            }
            Ok(_) => {}
            Err(e) => {
                if pin.changed.load(Ordering::SeqCst) {
                    return report(&app, &serial, |v| (v.online, v.error) = (false, Some(CHANGED.into())));
                }
                let message = match e {
                    ConnectionError::ConnectionRefused(_) => "The printer refused the access code.",
                    _ => "Can't reach the printer.",
                };
                report(&app, &serial, |v| (v.online, v.error) = (false, Some(message.into())));
                failures += 1;
                tokio::time::sleep(Duration::from_secs(if failures == 1 { 5 } else { 30 })).await;
            }
        }
    }
}

/// Logs how a print ended for the brief and, with the switch on, tells the owner.
fn print_ended(app: &AppHandle, o: &printer::PrintOutcome) {
    use binder_core::events::EventKind;
    use tauri_plugin_notification::NotificationExt;
    let state = app.state::<AppState>();
    let kind = if matches!(o, printer::PrintOutcome::Done { .. }) { EventKind::PrintDone } else { EventKind::PrintFailed };
    let _ = state.db.lock().unwrap().log_event(chrono::Utc::now().timestamp(), kind, o.job(), &o.body());
    let _ = app.emit("brief://changed", ());
    if state.config.lock().unwrap().print_notify {
        let _ = app.notification().builder().title(o.title()).body(o.body()).show();
    }
}

pub async fn send(app: &AppHandle, serial: &str, kind: PrinterCommand) -> Result<(), String> {
    let (client, seq) = app.state::<AppState>().printers.client(serial).ok_or("The printer isn't connected.")?;
    client
        .publish(printer::request_topic(serial), QoS::AtMostOnce, false, printer::command(seq, kind))
        .await
        .map_err(|_| "The printer isn't connected.".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const CERT: &[u8] = include_bytes!("../tests/fixtures/printer/cert.der");
    const MSG: &[u8] = include_bytes!("../tests/fixtures/printer/msg.bin");
    const SIG: &[u8] = include_bytes!("../tests/fixtures/printer/sig.bin");

    #[test]
    fn the_key_is_read_from_a_version_1_certificate() {
        let (kind, key) = public_key(CERT).unwrap();
        assert_eq!(kind, KeyKind::Rsa);
        assert_eq!(key[0], 0x30, "an RSAPublicKey SEQUENCE");
    }

    #[test]
    fn a_version_1_certificate_still_verifies() {
        assert!(verify_signed(CERT, SignatureScheme::RSA_PKCS1_SHA256, MSG, SIG).is_ok());
    }

    #[test]
    fn a_wrong_signature_is_refused() {
        let mut bad = SIG.to_vec();
        bad[10] ^= 1;
        assert!(verify_signed(CERT, SignatureScheme::RSA_PKCS1_SHA256, MSG, &bad).is_err());
        assert!(verify_signed(CERT, SignatureScheme::RSA_PKCS1_SHA256, b"other params", SIG).is_err());
        assert!(verify_signed(CERT, SignatureScheme::ECDSA_NISTP256_SHA256, MSG, SIG).is_err());
    }

    #[test]
    fn garbage_is_not_a_certificate() {
        assert!(public_key(b"\x30\x03\x02\x01").is_none());
        assert!(public_key(&[]).is_none());
    }
}
