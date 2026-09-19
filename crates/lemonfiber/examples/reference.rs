//! Writes the command reference, a page per command.
//!
//! `just reference` runs it from the workspace root. The comparison lives in a test,
//! so a stale artefact fails the build rather than this program.

fn main() -> std::process::ExitCode {
    match lemonfiber::reference::write(std::path::Path::new(".")) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("error: the command reference could not be written: {why}");
            std::process::ExitCode::FAILURE
        }
    }
}
