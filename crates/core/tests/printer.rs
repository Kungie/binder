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
            ..Default::default()
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

fn st(state: &str, job: &str, layer: u32) -> PrinterState {
    PrinterState { gcode_state: Some(state.into()), job: Some(job.into()), layer: Some(layer), ..Default::default() }
}

#[test]
fn running_then_finish_is_done_with_the_time() {
    let mut w = PrintWatch::default();
    assert_eq!(w.seen(&st("IDLE", "", 0), 900), None);
    assert_eq!(w.seen(&st("RUNNING", "Benchy", 1), 1000), None);
    assert_eq!(w.seen(&st("RUNNING", "Benchy", 80), 4000), None);
    let out = w.seen(&st("FINISH", "Benchy", 200), 1000 + 2 * 3600 + 14 * 60).unwrap();
    assert_eq!(out, PrintOutcome::Done { job: "Benchy".into(), minutes: Some(134) });
    assert_eq!((out.title(), out.body()), ("Benchy is done".to_string(), "Took 2 h 14 min".to_string()));
    assert_eq!(PrintOutcome::Done { job: "Clip".into(), minutes: Some(38) }.body(), "Took 38 min");
}

#[test]
fn paused_then_failed_gives_the_layer() {
    let mut w = PrintWatch::default();
    w.seen(&st("RUNNING", "Vase", 10), 0);
    w.seen(&st("PAUSE", "Vase", 112), 100);
    let out = w.seen(&st("FAILED", "Vase", 112), 200).unwrap();
    assert_eq!(out, PrintOutcome::Failed { job: "Vase".into(), layer: Some(112) });
    assert_eq!((out.title(), out.body()), ("Vase failed".to_string(), "Stopped at layer 112".to_string()));
}

#[test]
fn finish_seen_first_is_quiet() {
    let mut w = PrintWatch::default();
    assert_eq!(w.seen(&st("FINISH", "Benchy", 200), 0), None);
}

#[test]
fn one_outcome_per_print() {
    let mut w = PrintWatch::default();
    w.seen(&st("RUNNING", "Benchy", 1), 0);
    assert!(w.seen(&st("FINISH", "Benchy", 200), 60).is_some());
    assert_eq!(w.seen(&st("FINISH", "Benchy", 200), 120), None);
}

#[test]
fn an_unknown_job_and_time_still_read_well() {
    let out = PrintOutcome::Done { job: String::new(), minutes: None };
    assert_eq!((out.title(), out.body()), ("Print is done".to_string(), String::new()));
}

#[test]
fn a_print_already_running_at_start_has_no_time() {
    let mut w = PrintWatch::default();
    w.seen(&st("RUNNING", "Benchy", 150), 5000);
    assert_eq!(w.seen(&st("FINISH", "Benchy", 200), 6000), Some(PrintOutcome::Done { job: "Benchy".into(), minutes: None }));
}

#[test]
fn hms_errors_light_and_fan_are_read() {
    let mut s = PrinterState::default();
    s.merge(&serde_json::json!({"print": {
        "hms": [{"attr": 0x0700_2000u32, "code": 0x0002_0001u32}],
        "print_error": 0x0300_4001u32,
        "lights_report": [{"node": "chamber_light", "mode": "on"}],
        "cooling_fan_speed": "15"
    }}));
    assert_eq!(s.hms, vec!["0700200000020001".to_string()]);
    assert_eq!(s.print_error.as_deref(), Some("03004001"));
    assert_eq!((s.light_on, s.fan_percent), (Some(true), Some(100)));
    // A report without hms keeps the list; an empty one clears it, and print_error 0 means none.
    s.merge(&serde_json::json!({"print": {"cooling_fan_speed": "0"}}));
    assert_eq!(s.hms.len(), 1);
    s.merge(&serde_json::json!({"print": {"hms": [], "print_error": 0, "lights_report": [{"node": "chamber_light", "mode": "off"}]}}));
    assert!(s.hms.is_empty());
    assert_eq!((s.print_error, s.light_on, s.fan_percent), (None, Some(false), Some(0)));
}

#[test]
fn light_and_fan_commands_are_bambu_json() {
    let v = |s: String| serde_json::from_str::<Value>(&s).unwrap();
    let on = v(command(3, PrinterCommand::Light { on: true }));
    assert_eq!(on["system"]["command"], "ledctrl");
    assert_eq!((on["system"]["led_node"].as_str(), on["system"]["led_mode"].as_str()), (Some("chamber_light"), Some("on")));
    assert_eq!(v(command(4, PrinterCommand::Light { on: false }))["system"]["led_mode"], "off");
    let fan = v(command(5, PrinterCommand::Fan { percent: 50 }));
    assert_eq!((fan["print"]["command"].as_str(), fan["print"]["param"].as_str()), (Some("gcode_line"), Some("M106 P1 S128\n")));
    assert_eq!(v(command(6, PrinterCommand::Fan { percent: 250 }))["print"]["param"], "M106 P1 S255\n");
    // The interface sends them as JSON.
    let parsed: PrinterCommand = serde_json::from_value(serde_json::json!({"light": {"on": true}})).unwrap();
    assert_eq!(parsed, PrinterCommand::Light { on: true });
    let pause: PrinterCommand = serde_json::from_value(serde_json::json!("pause")).unwrap();
    assert_eq!(pause, PrinterCommand::Pause);
}

#[test]
fn the_hms_catalog_reads_bambus_list_and_names_the_severity() {
    use binder_core::printer::hms::Catalog;
    let json = r#"{"result":0,"data":{"device_hms":{"en":[{"ecode":"0700200000020001","intro":"AMS A Slot 1 filament has run out."},{"ecode":"0500020000030012","intro":""}]},
        "device_error":{"en":[{"ecode":"03004001","intro":"The nozzle is clogged."}]}}}"#;
    let c = Catalog::parse(json).unwrap();
    assert_eq!(c.len(), 2);
    assert_eq!(c.describe("0700200000020001").as_deref(), Some("AMS A Slot 1 filament has run out."));
    assert_eq!(c.describe("03004001").as_deref(), Some("The nozzle is clogged."));
    // Unknown or empty: the code and how serious it is, from the code's own severity field.
    assert_eq!(c.describe("0500020000030012").as_deref(), Some("Printer message 0500-0200-0003-0012 (common)."));
    assert_eq!(c.describe("0C00010000010001").as_deref(), Some("Printer message 0C00-0100-0001-0001 (fatal)."));
    assert!(Catalog::parse("not json").is_none());
}

#[test]
fn the_camera_signs_in_and_frames_come_out_whole() {
    use binder_core::printer::camera::{Frames, auth};
    let a = auth("12345678");
    assert_eq!(a.len(), 80);
    assert_eq!(&a[0..4], &0x40u32.to_le_bytes());
    assert_eq!(&a[4..8], &0x3000u32.to_le_bytes());
    assert_eq!(&a[16..20], b"bblp");
    assert_eq!(&a[48..56], b"12345678");
    assert!(a[56..].iter().all(|b| *b == 0));
    let jpeg = |n: u8| [vec![0xFF, 0xD8], vec![n; 5], vec![0xFF, 0xD9]].concat();
    let frame = |j: &[u8]| [(j.len() as u32).to_le_bytes().to_vec(), vec![0; 12], j.to_vec()].concat();
    let stream = [frame(&jpeg(1)), frame(&jpeg(2))].concat();
    let mut f = Frames::default();
    // Bytes arrive in any pieces.
    let mut out = vec![];
    for chunk in stream.chunks(7) {
        out.extend(f.push(chunk));
    }
    assert_eq!(out, vec![jpeg(1), jpeg(2)]);
    // Something that isn't a JPEG is dropped, and the stream carries on.
    let mut f = Frames::default();
    assert!(f.push(&frame(&[1, 2, 3, 4])).is_empty());
    assert_eq!(f.push(&frame(&jpeg(3))), vec![jpeg(3)]);
}
