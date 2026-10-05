#![no_main]
//! The `Host` and `Origin` headers arrive from whatever sent the request, and they are
//! read before anything else is: a request whose header names somewhere else is turned
//! away there. Every header must come back as an answer, never a panic: a panic here is
//! a request that takes the serving task down before it was even admitted.

use libfuzzer_sys::fuzz_target;
use lemonfiber_api::guard::{host_is_here, origin_is_here, Binding};

fuzz_target!(|data: &[u8]| {
    // The first byte chooses how the server is listening, so the loopback-only answer
    // and both answers past this machine are reached; the rest is the header, and after
    // a line break the one name the run answers to besides.
    let Some((&shape, rest)) = data.split_first() else {
        return;
    };
    let Ok(text) = std::str::from_utf8(rest) else {
        return;
    };
    let (header, named) = text.split_once('\n').unwrap_or((text, ""));
    let at = Binding {
        port: 8484,
        beyond: shape & 1 == 1,
        named: (shape & 2 == 2).then(|| named.to_owned()),
    };
    let _ = host_is_here(Some(header), &at);
    let _ = origin_is_here(Some(header), &at);
});
