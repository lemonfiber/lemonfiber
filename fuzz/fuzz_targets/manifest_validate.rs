#![no_main]
//! Validation runs on whatever survived parsing, and it is the part that
//! indexes, cross-references profiles against services, and compares dates. A
//! manifest that parses but violates the contract must produce violations, not
//! an index-out-of-bounds.

use libfuzzer_sys::fuzz_target;
use lemonfiber_manifest::{validate, Date, Manifest};

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let Ok(manifest) = Manifest::from_toml(text) else {
        return;
    };
    // A fixed date keeps the run reproducible: a crash found today must still
    // reproduce tomorrow, which it would not if "today" moved.
    let Some(today) = Date::parse("2026-01-01") else {
        return;
    };
    let _ = validate(&manifest, today);
});
