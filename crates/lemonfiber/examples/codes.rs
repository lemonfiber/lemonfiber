//! Writes the published code registry, or the reference rendered from it, to stdout.
//!
//! `just codes` runs it twice, redirecting each to its committed artefact: `json`
//! first, then `reference`, which reads the file `json` just wrote. The comparisons
//! live in tests, so a stale artefact fails the build rather than this binary.

use std::process::ExitCode;

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        Some("json") => {
            print!("{}", lemonfiber::codes::render_json());
            ExitCode::SUCCESS
        }
        Some("reference") => {
            let published = std::fs::read_to_string(lemonfiber::codes::CODES_JSON);
            match published
                .map_err(|error| error.to_string())
                .and_then(|text| lemonfiber::codes::render_reference(&text))
            {
                Ok(reference) => {
                    print!("{reference}");
                    ExitCode::SUCCESS
                }
                Err(why) => {
                    eprintln!("{}: {why}", lemonfiber::codes::CODES_JSON);
                    ExitCode::FAILURE
                }
            }
        }
        _ => {
            eprintln!("usage: codes json|reference");
            ExitCode::from(2)
        }
    }
}
