//! Writes the command reference to stdout.
//!
//! `just reference` redirects it to the committed artefact. The comparison lives in
//! a test, so a stale artefact fails the build rather than this binary.

fn main() {
    print!("{}", lemonfiber::reference::render());
}
