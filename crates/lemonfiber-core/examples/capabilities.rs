//! Writes the capability vocabulary to stdout.
//!
//! `just capabilities` redirects it to the committed artefact. It refuses to write
//! one that is untrue: a capability no bundled service declares, or a name a bundled
//! service declares that the vocabulary does not carry, comes back here as a refusal
//! naming what disagreed rather than as a file with a gap in it.

fn main() {
    match lemonfiber_core::plugin::vocabulary() {
        Ok(text) => print!("{text}"),
        Err(problem) => {
            eprintln!("{problem}");
            std::process::exit(1);
        }
    }
}
