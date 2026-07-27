#![no_main]
//! `stack.toml` is the one file a user is invited to edit by hand, so a
//! malformed one is an ordinary Tuesday rather than an attack. Parsing it must
//! fail with a diagnostic, never panic — a panic here is a stack that will not
//! start and cannot say why.

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Only valid UTF-8 reaches the parser in practice: the caller reads the
    // file as a string, so non-UTF-8 is rejected before this point.
    if let Ok(text) = std::str::from_utf8(data) {
        let _ = lemonfiber_manifest::Manifest::from_toml(text);
    }
});
