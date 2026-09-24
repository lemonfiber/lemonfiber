//! The error-code reference, rendered from the registry every code is declared in.
//!
//! `just codes` writes it to the committed artefact. The comparison lives in a test,
//! so a stale artefact fails the build rather than the program that emits it.
//!
//! What each code means is written for operators elsewhere. This is the inventory
//! that document is held to: every code, and nothing that is not one.

use lemonfiber_core::error::codes::EVERY;

/// Where the generated artefact is kept, relative to the workspace root.
pub const CODES_PATH: &str = "reference/error-codes.md";

/// What the artefact opens with, before the first code.
const PREAMBLE: &str = "\
# `lemonfiber` — error codes

Generated from the codes the crates declare. Run `just codes` to rewrite it.

Every code lemonfiber can raise, and nothing else. A code is a family and a number,
it is never recycled, and it is the token to search for. What each one means, and
what to do about it, is written for operators at
<https://docs.lemonfiber.app/fixing/every-error-by-code/>.

";

/// The reference, as it is committed.
#[must_use]
pub fn render() -> String {
    let mut out = String::from(PREAMBLE);
    for code in EVERY {
        out.push_str("- `");
        out.push_str(code.as_str());
        out.push_str("`\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{render, CODES_PATH};
    use std::path::Path;

    #[test]
    fn the_committed_reference_still_matches_the_codes_the_registry_declares() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let stored = std::fs::read_to_string(root.join(CODES_PATH)).unwrap_or_default();

        assert_eq!(
            stored,
            render(),
            "the error-code reference is out of date — regenerate it with `just codes`"
        );
    }
}
