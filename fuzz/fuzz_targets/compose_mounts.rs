#![no_main]
//! Compose files are read for the mounts each service would see, through every
//! `extends` it names — and a stack the operator points lemonfiber at is theirs to
//! write, cycles and all. Every set of files must come back as an answer, never a
//! panic and never a walk that does not end.

use std::path::PathBuf;

use libfuzzer_sys::fuzz_target;
use lemonfiber_core::stack::mounts::crowded;

fuzz_target!(|data: &[u8]| {
    // A NUL ends one file and starts the next, and each is named by its position, so
    // an `extends` naming another file reaches one that exists.
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let files: Vec<(PathBuf, String)> = text
        .split('\0')
        .enumerate()
        .map(|(at, body)| (PathBuf::from(format!("compose/{at}.yml")), body.to_owned()))
        .collect();
    let _ = crowded(&files);
});
