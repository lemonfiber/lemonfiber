#![no_main]
//! `plugin.toml` is written by whoever wrote the plugin and read on the machine of
//! whoever installs it. A malformed one must be refused with a diagnostic, never a
//! panic: a panic here is an install that stops without saying why.

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // The installer reads the file as a string, so only valid UTF-8 reaches the parser.
    if let Ok(text) = std::str::from_utf8(data) {
        let _ = lemonfiber_plugin::Manifest::from_toml(text);
    }
});
