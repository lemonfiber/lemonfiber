#![no_main]
//! `plugin.toml` is written by whoever wrote the plugin and read on the machine of
//! whoever installs it. A malformed one must be refused with a diagnostic, never a
//! panic: a panic here is an install that stops without saying why.
//!
//! What parses is then held to every value rule the reader applies, because those
//! rules are where a stranger's text is taken apart character by character — and a
//! manifest the reader passes is one whose values are written into the stack's own
//! files, so each value that reaches one is checked against the rule that let it
//! through.

use libfuzzer_sys::fuzz_target;
use lemonfiber_plugin::refusing::carried::{is_directory, is_label, is_reference, is_route};

fuzz_target!(|data: &[u8]| {
    // The installer reads the file as a string, so only valid UTF-8 reaches the parser.
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let Ok(manifest) = lemonfiber_plugin::Manifest::from_toml(text) else {
        return;
    };
    if !lemonfiber_plugin::refusals(&manifest, &[]).is_empty() {
        return;
    }
    for service in &manifest.services {
        assert!(is_label(&service.id));
        assert!(is_reference(&service.image));
        assert!(is_directory(service.configuration()));
        assert!(is_label(manifest.entry(service).hostname));
    }
    for proof in &manifest.proofs {
        assert!(is_route(&proof.request.path));
    }
});
