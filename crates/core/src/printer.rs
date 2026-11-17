//! A Bambu Lab printer in LAN mode: its MQTT reports, the commands Binder sends and the certificate pin.
//! The connection itself lives in the app; this is the part that needs no network.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PrinterState {
    pub gcode_state: Option<String>,
    pub percent: Option<u8>,
    pub remaining_min: Option<u32>,
    pub layer: Option<u32>,
    pub total_layers: Option<u32>,
    pub nozzle_c: Option<f32>,
    pub nozzle_target_c: Option<f32>,
    pub bed_c: Option<f32>,
    pub bed_target_c: Option<f32>,
    pub job: Option<String>,
    /// Health messages now on, as Bambu's 16 hex digit codes.
    pub hms: Vec<String>,
    /// The print's own error code (8 hex digits), when there is one.
    pub print_error: Option<String>,
    pub light_on: Option<bool>,
    /// The part cooling fan.
    pub fan_percent: Option<u8>,
}

/// Bambu sends numbers, and sometimes numbers as strings.
fn number(v: &Value) -> Option<f64> {
    v.as_f64().or_else(|| v.as_str()?.trim().parse().ok())
}

fn text(v: &Value) -> Option<String> {
    v.as_str().map(str::to_string)
}

impl PrinterState {
    /// Applies one report. Reports after the first carry only what changed, so absent fields keep their value.
    pub fn merge(&mut self, report: &Value) {
        let Some(p) = report.get("print").and_then(Value::as_object) else { return };
        let set_f = |slot: &mut Option<f32>, key: &str| {
            if let Some(v) = p.get(key).and_then(number) {
                *slot = Some(v as f32);
            }
        };
        set_f(&mut self.nozzle_c, "nozzle_temper");
        set_f(&mut self.nozzle_target_c, "nozzle_target_temper");
        set_f(&mut self.bed_c, "bed_temper");
        set_f(&mut self.bed_target_c, "bed_target_temper");
        let set_u = |slot: &mut Option<u32>, key: &str| {
            if let Some(v) = p.get(key).and_then(number) {
                *slot = Some(v.max(0.0) as u32);
            }
        };
        set_u(&mut self.remaining_min, "mc_remaining_time");
        set_u(&mut self.layer, "layer_num");
        set_u(&mut self.total_layers, "total_layer_num");
        if let Some(v) = p.get("mc_percent").and_then(number) {
            self.percent = Some(v.clamp(0.0, 100.0) as u8);
        }
        if let Some(v) = p.get("gcode_state").and_then(text) {
            self.gcode_state = Some(v);
        }
        if let Some(v) = p.get("subtask_name").and_then(text) {
            self.job = Some(v);
        }
        if let Some(list) = p.get("hms").and_then(Value::as_array) {
            self.hms = list
                .iter()
                .filter_map(|h| Some(format!("{:08X}{:08X}", number(h.get("attr")?)? as u32, number(h.get("code")?)? as u32)))
                .collect();
        }
        if let Some(e) = p.get("print_error").and_then(number) {
            self.print_error = (e as u32 != 0).then(|| format!("{:08X}", e as u32));
        }
        if let Some(lights) = p.get("lights_report").and_then(Value::as_array) {
            if let Some(l) = lights.iter().find(|l| l.get("node").and_then(Value::as_str) == Some("chamber_light")) {
                self.light_on = l.get("mode").and_then(Value::as_str).map(|m| m == "on");
            }
        }
        if let Some(f) = p.get("cooling_fan_speed").and_then(number) {
            // Bambu reports the fan in sixteenths, 0 to 15.
            self.fan_percent = Some(((f.clamp(0.0, 15.0) / 15.0) * 100.0).round() as u8);
        }
    }
}

/// How a print ended, for a notification and the log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum PrintOutcome {
    Done { job: String, minutes: Option<u32> },
    Failed { job: String, layer: Option<u32> },
}

fn job_name(job: &str) -> &str {
    if job.trim().is_empty() { "Print" } else { job.trim() }
}

impl PrintOutcome {
    pub fn job(&self) -> &str {
        match self {
            PrintOutcome::Done { job, .. } | PrintOutcome::Failed { job, .. } => job_name(job),
        }
    }

    pub fn title(&self) -> String {
        match self {
            PrintOutcome::Done { .. } => format!("{} is done", self.job()),
            PrintOutcome::Failed { .. } => format!("{} failed", self.job()),
        }
    }

    pub fn body(&self) -> String {
        match self {
            PrintOutcome::Done { minutes: Some(m), .. } if *m >= 60 => format!("Took {} h {} min", m / 60, m % 60),
            PrintOutcome::Done { minutes: Some(m), .. } => format!("Took {m} min"),
            PrintOutcome::Failed { layer: Some(l), .. } => format!("Stopped at layer {l}"),
            _ => String::new(),
        }
    }
}

/// Follows one printer's reports and says when a print it saw running ends.
#[derive(Debug, Default)]
pub struct PrintWatch {
    printing_since: Option<i64>,
    was_printing: bool,
    /// Whether a report came before this one; a print already running at the first has no known start.
    started: bool,
}

impl PrintWatch {
    pub fn seen(&mut self, s: &PrinterState, now: i64) -> Option<PrintOutcome> {
        let state = s.gcode_state.as_deref().unwrap_or("");
        let printing = matches!(state, "RUNNING" | "PAUSE");
        let job = s.job.clone().unwrap_or_default();
        let out = match (self.was_printing, state) {
            (true, "FINISH") => Some(PrintOutcome::Done { job, minutes: self.printing_since.map(|t| ((now - t).max(0) / 60) as u32) }),
            (true, "FAILED") => Some(PrintOutcome::Failed { job, layer: s.layer }),
            _ => None,
        };
        if printing && !self.was_printing && self.started {
            self.printing_since = Some(now);
        }
        self.started = true;
        if !printing {
            self.printing_since = None;
        }
        self.was_printing = printing;
        out
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrinterCommand {
    Pause,
    Resume,
    Stop,
    PushAll,
    /// The chamber light.
    Light { on: bool },
    /// The part cooling fan, 0 to 100.
    Fan { percent: u8 },
}

pub fn command(seq: u32, kind: PrinterCommand) -> String {
    let seq = seq.to_string();
    let (section, name) = match kind {
        PrinterCommand::Pause => ("print", "pause"),
        PrinterCommand::Resume => ("print", "resume"),
        PrinterCommand::Stop => ("print", "stop"),
        PrinterCommand::PushAll => ("pushing", "pushall"),
        PrinterCommand::Light { on } => {
            let mode = if on { "on" } else { "off" };
            return serde_json::json!({ "system": { "sequence_id": seq, "command": "ledctrl", "led_node": "chamber_light", "led_mode": mode,
                "led_on_time": 500, "led_off_time": 500, "loop_times": 0, "interval_time": 0 } })
            .to_string();
        }
        PrinterCommand::Fan { percent } => {
            let s = ((f64::from(percent.min(100)) / 100.0) * 255.0).round() as u32;
            return serde_json::json!({ "print": { "sequence_id": seq, "command": "gcode_line", "param": format!("M106 P1 S{s}\n") } }).to_string();
        }
    };
    serde_json::json!({ section: { "sequence_id": seq, "command": name } }).to_string()
}

pub fn report_topic(serial: &str) -> String {
    format!("device/{serial}/report")
}

pub fn request_topic(serial: &str) -> String {
    format!("device/{serial}/request")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PinCheck {
    Matches,
    /// Nothing was pinned yet; this is the certificate's SHA-256 to pin.
    New(String),
    Mismatch,
}

pub fn cert_matches(pinned: Option<&str>, der: &[u8]) -> PinCheck {
    let hash: String = Sha256::digest(der).iter().map(|b| format!("{b:02x}")).collect();
    match pinned {
        None => PinCheck::New(hash),
        Some(p) if p.eq_ignore_ascii_case(&hash) => PinCheck::Matches,
        Some(_) => PinCheck::Mismatch,
    }
}

/// Bambu's own words for its health and error codes, from the list it publishes.
pub mod hms {
    use std::collections::HashMap;

    #[derive(Debug, Default, Clone)]
    pub struct Catalog(HashMap<String, String>);

    impl Catalog {
        /// Reads `https://e.bambulab.com/query.php?lang=en`.
        pub fn parse(json: &str) -> Option<Catalog> {
            let v: serde_json::Value = serde_json::from_str(json).ok()?;
            let data = v.get("data")?;
            let mut map = HashMap::new();
            for list in ["device_hms", "device_error"] {
                for e in data.get(list).and_then(|l| l.get("en")).and_then(|l| l.as_array()).into_iter().flatten() {
                    if let (Some(code), Some(text)) = (e.get("ecode").and_then(|c| c.as_str()), e.get("intro").and_then(|t| t.as_str())) {
                        if !text.trim().is_empty() {
                            map.insert(code.to_ascii_uppercase(), text.trim().to_string());
                        }
                    }
                }
            }
            Some(Catalog(map))
        }

        pub fn len(&self) -> usize {
            self.0.len()
        }

        pub fn is_empty(&self) -> bool {
            self.0.is_empty()
        }

        /// The message for a code, or the code itself with how serious it is.
        pub fn describe(&self, code: &str) -> Option<String> {
            let code = code.to_ascii_uppercase();
            if let Some(t) = self.0.get(&code) {
                return Some(t.clone());
            }
            if code.len() != 16 {
                return Some(format!("Printer error {code}."));
            }
            let severity = match &code[8..12] {
                "0001" => "fatal",
                "0002" => "serious",
                "0003" => "common",
                _ => "info",
            };
            let parts: Vec<&str> = (0..4).map(|i| &code[i * 4..i * 4 + 4]).collect();
            Some(format!("Printer message {} ({severity}).", parts.join("-")))
        }
    }
}

/// The A1 and P1 camera on port 6000: a TLS stream of JPEG frames after an 80 byte sign-in.
pub mod camera {
    /// The sign-in: two words that say what it is, the user `bblp` and the access code, each in 32 bytes.
    pub fn auth(code: &str) -> [u8; 80] {
        let mut a = [0u8; 80];
        a[0..4].copy_from_slice(&0x40u32.to_le_bytes());
        a[4..8].copy_from_slice(&0x3000u32.to_le_bytes());
        a[16..20].copy_from_slice(b"bblp");
        let c = code.as_bytes();
        let n = c.len().min(32);
        a[48..48 + n].copy_from_slice(&c[..n]);
        a
    }

    /// Cuts the stream into frames: a 16 byte header whose first word is the JPEG's length, then the JPEG.
    #[derive(Debug, Default)]
    pub struct Frames {
        buf: Vec<u8>,
    }

    impl Frames {
        pub fn push(&mut self, bytes: &[u8]) -> Vec<Vec<u8>> {
            self.buf.extend_from_slice(bytes);
            let mut out = vec![];
            loop {
                if self.buf.len() < 16 {
                    break;
                }
                let len = u32::from_le_bytes([self.buf[0], self.buf[1], self.buf[2], self.buf[3]]) as usize;
                if len > 8 << 20 {
                    // Not a header: start over rather than wait for gigabytes.
                    self.buf.clear();
                    break;
                }
                if self.buf.len() < 16 + len {
                    break;
                }
                let jpeg: Vec<u8> = self.buf[16..16 + len].to_vec();
                self.buf.drain(..16 + len);
                if jpeg.starts_with(&[0xFF, 0xD8]) && jpeg.ends_with(&[0xFF, 0xD9]) {
                    out.push(jpeg);
                }
            }
            out
        }
    }
}

/// Every print that ended, for the Printer screen and the weekly note.
pub mod history {
    use super::PrintOutcome;
    use crate::{Db, Result};
    use rusqlite::params;
    use serde::Serialize;

    #[derive(Debug, Clone, PartialEq, Serialize)]
    pub struct Print {
        pub at: i64,
        pub job: String,
        pub ok: bool,
        pub minutes: Option<u32>,
        pub layer: Option<u32>,
    }

    pub fn record(db: &mut Db, at: i64, o: &PrintOutcome) -> Result<()> {
        let (ok, minutes, layer) = match o {
            PrintOutcome::Done { minutes, .. } => (true, *minutes, None),
            PrintOutcome::Failed { layer, .. } => (false, None, *layer),
        };
        db.conn.execute("INSERT INTO prints (at, job, ok, minutes, layer) VALUES (?1, ?2, ?3, ?4, ?5)", params![at, o.job(), ok, minutes, layer])?;
        Ok(())
    }

    pub fn list(db: &Db, limit: usize) -> Result<Vec<Print>> {
        let mut st = db.conn.prepare("SELECT at, job, ok, minutes, layer FROM prints ORDER BY at DESC, rowid DESC LIMIT ?1")?;
        let rows = st.query_map([limit as i64], |r| Ok(Print { at: r.get(0)?, job: r.get(1)?, ok: r.get(2)?, minutes: r.get(3)?, layer: r.get(4)? }))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// `3 prints, 3 h 5 min, 1 failed` for `from <= at < to`, or `None` without prints.
    pub fn summary(db: &Db, from: i64, to: i64) -> Result<Option<String>> {
        let (n, minutes, failed): (i64, i64, i64) = db.conn.query_row(
            "SELECT COUNT(*), COALESCE(SUM(minutes), 0), COALESCE(SUM(CASE WHEN ok THEN 0 ELSE 1 END), 0) FROM prints WHERE at >= ?1 AND at < ?2",
            params![from, to],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        if n == 0 {
            return Ok(None);
        }
        let time = if minutes >= 60 { format!("{} h {} min", minutes / 60, minutes % 60) } else { format!("{minutes} min") };
        Ok(Some(format!("{n} print{}, {time}, {failed} failed", if n == 1 { "" } else { "s" })))
    }
}
