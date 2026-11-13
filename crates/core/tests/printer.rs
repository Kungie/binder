use binder_core::printer::*;
use serde_json::Value;

fn report(name: &str) -> Value {
    let path = format!("{}/tests/fixtures/bambu/{name}.json", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn full_report_fills_state() {
    let mut s = PrinterState::default();
    s.merge(&report("full"));
    assert_eq!(
        s,
        PrinterState {
            gcode_state: Some("RUNNING".into()),
            percent: Some(42),
            remaining_min: Some(73),
            layer: Some(120),
            total_layers: Some(300),
            nozzle_c: Some(219.8),
            nozzle_target_c: Some(220.0),
            bed_c: Some(54.9),
            bed_target_c: Some(55.0),
            job: Some("case-lid".into()),
        }
    );
}

#[test]
fn delta_keeps_other_fields() {
    let mut s = PrinterState::default();
    s.merge(&report("full"));
    s.merge(&report("delta"));
    assert_eq!((s.percent, s.nozzle_c), (Some(43), Some(220.1)));
    assert_eq!((s.layer, s.job.as_deref(), s.bed_c), (Some(120), Some("case-lid"), Some(54.9)));
    // A message without a print section changes nothing.
    let before = s.clone();
    s.merge(&serde_json::json!({"info": {"command": "get_version"}}));
    assert_eq!(s, before);
}

#[test]
fn commands_are_bambu_json() {
    let v = |s: String| serde_json::from_str::<Value>(&s).unwrap();
    assert_eq!(v(command(7, PrinterCommand::Pause)), serde_json::json!({"print": {"sequence_id": "7", "command": "pause"}}));
    assert_eq!(v(command(8, PrinterCommand::Resume))["print"]["command"], "resume");
    assert_eq!(v(command(9, PrinterCommand::Stop))["print"]["command"], "stop");
    assert_eq!(v(command(10, PrinterCommand::PushAll)), serde_json::json!({"pushing": {"sequence_id": "10", "command": "pushall"}}));
    assert_eq!(report_topic("SERIAL1"), "device/SERIAL1/report");
    assert_eq!(request_topic("SERIAL1"), "device/SERIAL1/request");
}

#[test]
fn pin_new_then_match_then_mismatch() {
    let der = b"a made-up certificate";
    let PinCheck::New(pin) = cert_matches(None, der) else { panic!("first sight is New") };
    assert_eq!(pin.len(), 64);
    assert!(pin.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    assert_eq!(cert_matches(Some(&pin), der), PinCheck::Matches);
    assert_eq!(cert_matches(Some(&pin.to_uppercase()), der), PinCheck::Matches);
    assert_eq!(cert_matches(Some(&pin), b"another certificate"), PinCheck::Mismatch);
}
