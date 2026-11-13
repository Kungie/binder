//! One MQTT connection per Bambu printer, over TLS with the printer's certificate pinned on first sight.
//! The access code lives in the login Keychain. It never reaches a file, a log or the interface.

use crate::AppState;
use binder_core::config::PrinterConfig;
use binder_core::printer::{self, PinCheck, PrinterCommand, PrinterState};
use rumqttc::{AsyncClient, ConnectionError, Event, MqttOptions, Packet, QoS, TlsConfiguration, Transport};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::CryptoProvider;
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

#[derive(Debug, Clone, Serialize)]
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
    provider: Arc<CryptoProvider>,
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
        rustls::crypto::verify_tls12_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }

    fn verify_tls13_signature(&self, message: &[u8], cert: &CertificateDer<'_>, dss: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider.signature_verification_algorithms.supported_schemes()
    }
}

pub fn keychain(serial: &str) -> keyring::Result<keyring::Entry> {
    keyring::Entry::new(KEYCHAIN_SERVICE, serial)
}

/// Changes one printer's view and tells the interface.
fn report(app: &AppHandle, serial: &str, change: impl FnOnce(&mut PrinterView)) {
    let view = {
        let state = app.state::<AppState>();
        let mut map = state.printers.0.lock().unwrap();
        let Some(live) = map.get_mut(serial) else { return };
        change(&mut live.view);
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
    let pin = Arc::new(Pin { pinned: Mutex::new(config.cert_sha256.clone()), fresh: Mutex::new(None), changed: AtomicBool::new(false), provider: provider.clone() });
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
                    report(&app, &serial, |v| {
                        v.state.merge(&json);
                        v.online = true;
                    });
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

pub async fn send(app: &AppHandle, serial: &str, kind: PrinterCommand) -> Result<(), String> {
    let (client, seq) = app.state::<AppState>().printers.client(serial).ok_or("The printer isn't connected.")?;
    client
        .publish(printer::request_topic(serial), QoS::AtMostOnce, false, printer::command(seq, kind))
        .await
        .map_err(|_| "The printer isn't connected.".to_string())
}
