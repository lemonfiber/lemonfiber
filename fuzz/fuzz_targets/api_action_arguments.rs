#![no_main]
//! An action's arguments arrive as a JSON body from whatever a browser or a script
//! sent. Every body must come back as a command or a refusal, never a panic: a panic
//! here is a request that takes the serving task down with it.

use libfuzzer_sys::fuzz_target;
use lemonfiber_api::actions::{named, Arguments, OFFERED};

fuzz_target!(|data: &[u8]| {
    // The first byte chooses which action is asked, so every action's arguments are
    // reached; the rest is the body, read the way the router reads it.
    let Some((&which, body)) = data.split_first() else {
        return;
    };
    let Some(action) = OFFERED.get(usize::from(which) % OFFERED.len()) else {
        return;
    };
    if let Ok(given) = serde_json::from_slice::<Arguments>(body) {
        let _ = named(action, given);
    }
});
