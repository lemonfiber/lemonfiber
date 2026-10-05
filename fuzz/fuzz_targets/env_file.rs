#![no_main]
//! The environment file is written by lemonfiber, edited by the operator, and holds
//! values that services wrote — an API key read out of a container's own
//! configuration among them. Every file must be read as settings and verbatim lines,
//! never a panic, and what is read must be written back as it was read: a second
//! reading that differed from the first is a setting that changed by being looked at.

use libfuzzer_sys::fuzz_target;
use lemonfiber_core::config::env::EnvFile;

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let file = EnvFile::parse(text);
    let rendered = file.render();
    assert_eq!(EnvFile::parse(&rendered).render(), rendered);
    for key in file.keys() {
        let _ = file.get(key);
    }
});
