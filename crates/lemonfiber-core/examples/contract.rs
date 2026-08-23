//! Writes the machine-readable contract to stdout.
//!
//! `just contract` redirects it to the committed artefact. The comparison lives
//! in a test, so a stale artefact fails the build rather than this binary.

fn main() {
    let contract = lemonfiber_core::contract::Contract::describe();
    match contract.to_json() {
        Some(text) => print!("{text}"),
        None => std::process::exit(1),
    }
}
