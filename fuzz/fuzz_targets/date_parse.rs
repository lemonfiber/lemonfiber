#![no_main]
//! A hand-rolled parser over split/parse with several numeric conversions —
//! small, total, and exactly the shape where an unconsidered input slips
//! through. Cheap to fuzz, so worth fuzzing.

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(text) = std::str::from_utf8(data) {
        let _ = lemonfiber_manifest::Date::parse(text);
    }
});
