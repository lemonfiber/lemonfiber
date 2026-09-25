#![no_main]
//! A read's query string arrives from whatever a browser or a script sent. Every
//! one must come back as a command or a refusal, never a panic: a panic here is a
//! request that takes the serving task down with it.

use libfuzzer_sys::fuzz_target;
use lemonfiber_api::read::table::{named, wanted, OFFERED};

fuzz_target!(|data: &[u8]| {
    // The first byte chooses which read is asked, so every read's parameters are
    // reached; the rest is the query string, which arrives as text.
    let Some((&which, rest)) = data.split_first() else {
        return;
    };
    let Some(read) = OFFERED.get(usize::from(which) % OFFERED.len()) else {
        return;
    };
    if let Ok(query) = std::str::from_utf8(rest) {
        if let Ok(given) = wanted(read, Some(query)) {
            let _ = named(read, given);
        }
    }
});
