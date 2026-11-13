use binder_core::remote::quote;
use std::process::Command;

#[test]
fn quote_survives_hostile_text() {
    for text in ["a b", "it's", "$(rm -rf ~)", "`id`", "two\nlines", "", "\"double\" and 'single'"] {
        let out = Command::new("sh").arg("-c").arg(format!("printf %s {}", quote(text))).output().unwrap();
        assert_eq!(String::from_utf8(out.stdout).unwrap(), text, "{text:?}");
    }
}
