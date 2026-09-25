//! Writes the error-code reference to stdout.
//!
//! `just codes` redirects it to the committed artefact. The comparison lives in a
//! test, so a stale artefact fails the build rather than this binary.

fn main() {
    print!("{}", lemonfiber::codes::render());
}
