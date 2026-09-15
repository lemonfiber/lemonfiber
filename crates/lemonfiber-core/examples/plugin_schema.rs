//! Writes the generated `plugin.toml` schema to stdout.
//!
//! `just plugin-schema` redirects it to the committed artefact. The comparison lives
//! in a test, so a stale artefact fails the build rather than this binary.

fn main() {
    match lemonfiber_core::plugin::schema() {
        Some(text) => print!("{text}"),
        None => std::process::exit(1),
    }
}
