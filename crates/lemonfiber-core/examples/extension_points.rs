//! Writes the published extension points to stdout.
//!
//! `just extension-points` redirects it to the committed artefact. The identities the
//! bundled rows already hold are read out of the doctor's own register, so a check
//! that is renamed moves this file rather than leaving a name a contribution could
//! take.

fn main() {
    match lemonfiber_core::plugin::points() {
        Some(text) => print!("{text}"),
        None => std::process::exit(1),
    }
}
