//! Writes a generated schema of a stack's manifest files to stdout: `root` for
//! `stack.toml`, `service` for a service's file.
//!
//! `just stack-schema` redirects each to its committed artefact. The comparison lives
//! in a test, so a stale artefact fails the build rather than this binary.

fn main() {
    let which = std::env::args().nth(1).unwrap_or_default();
    let text = match which.as_str() {
        "root" => lemonfiber_core::stack::schema::root(),
        "service" => lemonfiber_core::stack::schema::service(),
        _ => None,
    };
    match text {
        Some(text) => print!("{text}"),
        None => std::process::exit(1),
    }
}
