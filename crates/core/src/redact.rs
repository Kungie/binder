//! Masks secrets in session text before any model sees it. It errs on the side of masking.

use regex::{Captures, Regex};
use std::sync::LazyLock;

const MASK: &str = "[redacted]";

static PRIVATE_KEY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"-----BEGIN [A-Z0-9 ]*PRIVATE KEY-----[\s\S]*?(-----END [A-Z0-9 ]*PRIVATE KEY-----|\z)").unwrap());
static TOKENS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\bsk-[A-Za-z0-9_\-]{16,}|\bgh[pousr]_[A-Za-z0-9]{20,}|\bAKIA[0-9A-Z]{16}\b").unwrap());
static BEARER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(bearer)\s+[A-Za-z0-9._~+/=\-]+").unwrap());
static ASSIGNED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)\b(password|passwd|pwd|secret|token|api[_-]?key|access[_-]?code)(\s*[:=]\s*)["']?[^\s"',;]+"#).unwrap()
});
static LONG_RUN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[A-Za-z0-9+/_\-]{40,}={0,2}").unwrap());

pub fn redact(text: &str) -> String {
    let s = PRIVATE_KEY.replace_all(text, MASK);
    let s = TOKENS.replace_all(&s, MASK);
    let s = BEARER.replace_all(&s, |c: &Captures| format!("{} {MASK}", &c[1]));
    let s = ASSIGNED.replace_all(&s, |c: &Captures| format!("{}{}{MASK}", &c[1], &c[2]));
    // Long runs of base64 or hex are keys and hashes; a run that is all letters is just a long word.
    LONG_RUN
        .replace_all(&s, |c: &Captures| {
            let m = &c[0];
            if m.chars().any(|ch| ch.is_ascii_digit()) { MASK.to_string() } else { m.to_string() }
        })
        .into_owned()
}
