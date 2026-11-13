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
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrinterCommand {
    Pause,
    Resume,
    Stop,
    PushAll,
}

pub fn command(seq: u32, kind: PrinterCommand) -> String {
    let (section, name) = match kind {
        PrinterCommand::Pause => ("print", "pause"),
        PrinterCommand::Resume => ("print", "resume"),
        PrinterCommand::Stop => ("print", "stop"),
        PrinterCommand::PushAll => ("pushing", "pushall"),
    };
    serde_json::json!({ section: { "sequence_id": seq.to_string(), "command": name } }).to_string()
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
