//! The error-code reference, rendered from the codes the crates declare.
//!
//! `just codes` writes it to the committed artefact. The comparison lives in a test,
//! so a stale artefact fails the build rather than the program that emits it.
//!
//! What each code means is written for operators elsewhere. This is the inventory
//! that document is held to: every code, and nothing that is not one.

mod scan;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

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

/// Every code the crates declare and ship, with the file each is declared in.
///
/// The root is the workspace the crates live in. It is passed rather than found so
/// that nothing here has to know where it was built, and so a caller can point the
/// reader somewhere it will fail.
///
/// One entry per declaration, so a code declared in two places arrives as two
/// entries rather than as one.
///
/// # Errors
///
/// Returns what stopped the read: a root holding no sources, a file the reader lost
/// its place in, or a code declared with something other than a literal name. Each
/// would otherwise shorten the list silently, which is the one way an inventory
/// taken this way could still be wrong while agreeing with itself.
pub fn declared(root: &Path) -> Result<Vec<(PathBuf, String)>, Vec<String>> {
    scan::declared(root)
}

/// The whole reference, or every reason the codes could not be read.
///
/// # Errors
///
/// The same reasons [`declared`] gives.
pub fn render(root: &Path) -> Result<String, Vec<String>> {
    let codes: BTreeSet<String> = declared(root)?.into_iter().map(|(_, code)| code).collect();

    let mut ordered: Vec<&String> = codes.iter().collect();
    ordered.sort_by_key(|code| ordering(code));

    let mut out = String::from(PREAMBLE);
    for code in ordered {
        out.push_str("- `");
        out.push_str(code);
        out.push_str("`\n");
    }
    Ok(out)
}

/// Where a code sorts: by family, then by number within it.
///
/// Sorted as text, `PROVIDER-10` would come between `PROVIDER-1` and `PROVIDER-2`,
/// and a family reaching ten entries would shuffle the artefact rather than extend
/// it. A code that is not a family and a number sorts on its whole self.
fn ordering(code: &str) -> (String, Option<u32>, String) {
    let Some((family, number)) = code.rsplit_once('-') else {
        return (code.to_owned(), None, code.to_owned());
    };
    let Ok(within) = number.parse::<u32>() else {
        return (code.to_owned(), None, code.to_owned());
    };
    (family.to_owned(), Some(within), String::new())
}

#[cfg(test)]
mod tests {
    use super::{declared, ordering, render, CODES_PATH};
    use std::path::{Path, PathBuf};

    /// The workspace this crate was built from.
    fn workspace_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    /// The reference as the crates declare it now.
    fn fresh() -> String {
        render(&workspace_root()).unwrap_or_default()
    }

    /// The committed artefact and the declarations must agree.
    ///
    /// A code added, renamed or removed without regenerating fails here rather than
    /// leaving a document claiming to list every code lemonfiber can raise while it
    /// lists a different set.
    #[test]
    fn the_committed_reference_still_matches_the_codes_the_crates_declare() {
        let stored = std::fs::read_to_string(workspace_root().join(CODES_PATH)).unwrap_or_default();

        assert_eq!(
            stored,
            fresh(),
            "the error-code reference is out of date — regenerate it with `just codes`"
        );
    }

    /// Nothing stopped the reader.
    ///
    /// Separated from the comparison because the two fail for opposite reasons: an
    /// artefact that disagrees is regenerated, and a read that could not finish is a
    /// declaration to write differently.
    #[test]
    fn every_declaration_in_the_workspace_is_accounted_for() {
        let complaints = render(&workspace_root()).err().unwrap_or_default();

        assert!(
            complaints.is_empty(),
            "the reader could not account for every code declaration: {complaints:?}"
        );
    }

    /// The reference holds the codes, and holds them once each.
    #[test]
    fn it_lists_a_code_from_every_family_the_crates_declare() {
        let text = fresh();

        for code in [
            "SETUP-1",
            "CONFIG-1",
            "STACK-1",
            "FORM-1",
            "ENV-1",
            "DOCKER-1",
            "PROC-1",
            "LIFE-1",
            "STORAGE-1",
            "QUAL-1",
            "VPN-1",
            "CRED-1",
            "PROVIDER-1",
            "WIRING-1",
            "SEED-1",
            "BACKUP-1",
            "RESTORE-1",
            "BUNDLE-1",
            "WATCH-1",
            "ACK-1",
            "WORD-1",
            "TUI-1",
        ] {
            assert_eq!(
                text.matches(&format!("- `{code}`\n")).count(),
                1,
                "{code} is listed once"
            );
        }
    }

    /// A code the tests alone declare is not one lemonfiber can raise.
    ///
    /// Named rather than counted, because a reader that had simply stopped early
    /// would also list none of them.
    #[test]
    fn it_leaves_out_the_codes_that_exist_only_inside_tests() {
        let text = fresh();

        for code in ["BUNDLE-0", "TEST-1", "TEST-2", "WORD-7", "WORD-8", "WORD-9"] {
            assert!(!text.contains(&format!("`{code}`")), "{code} is not raised");
        }
    }

    /// A bitrate is not a code.
    ///
    /// Each of these is written in the crates in the shape a code is written in, and
    /// none is one. They stay out because the reader looks for the call that declares
    /// a code rather than for text shaped like one.
    #[test]
    fn it_takes_nothing_that_merely_reads_like_a_code() {
        let text = fresh();

        for written in ["AAC-320", "MP3-320", "UTF-8"] {
            assert!(!text.contains(written), "{written} is not a code");
        }
    }

    /// A family reaching ten entries extends the list rather than reordering it.
    #[test]
    fn it_orders_a_family_by_number_rather_than_by_text() {
        let mut codes = ["PROVIDER-10", "PROVIDER-2", "PROVIDER-1", "ACK-1"];
        codes.sort_by_key(|code| ordering(code));

        assert_eq!(codes, ["ACK-1", "PROVIDER-1", "PROVIDER-2", "PROVIDER-10"]);
    }

    /// A code shaped like nothing in particular still sorts somewhere fixed.
    #[test]
    fn it_orders_a_code_that_is_not_a_family_and_a_number_on_its_whole_self() {
        assert_eq!(
            ordering("PROVIDER-X"),
            ("PROVIDER-X".to_owned(), None, "PROVIDER-X".to_owned())
        );
        assert_eq!(
            ordering("SOMETHING"),
            ("SOMETHING".to_owned(), None, "SOMETHING".to_owned())
        );
    }

    /// Pointed somewhere with no crates in it, the reference is not rendered at all.
    #[test]
    fn it_renders_nothing_from_a_root_that_holds_no_sources() {
        assert!(render(Path::new("/nowhere-that-holds-a-workspace")).is_err());
        assert!(declared(Path::new("/nowhere-that-holds-a-workspace")).is_err());
    }

    /// Every code is reported with the file that declares it.
    #[test]
    fn it_says_where_each_code_is_declared() {
        let found = declared(&workspace_root()).unwrap_or_default();
        let leaking = found.iter().find(|(_, code)| code == "VPN-1");

        assert_eq!(
            leaking.map(|(path, _)| path.to_string_lossy().replace('\\', "/")),
            Some("lemonfiber-core/src/doctor/vpn/leak.rs".to_owned())
        );
    }
}
