//! The full reset — putting the stack back to lemonfiber's own state.
//!
//! Drift is preserved by default: an operator's edit is theirs to keep. A reset is the
//! escape hatch from that — the explicit, confirmed decision to let lemonfiber's state
//! win and discard the edits. Because it discards work, it names exactly what will be
//! lost first, and does nothing until confirmed: unconfirmed it only previews, so the
//! operator sees the diffs before deciding.
//!
//! A reset restores lemonfiber's state *including* the operator's recorded quality choice
//! — that is a managed setting, not drift — so the stack is written carrying the recorded
//! preset, with only the hand-edits on top of it reverted.

use crate::error::{Diagnose, Problem};
use crate::model::ResetReport;

use super::Ctx;

/// Revert the stack to lemonfiber's state, or — until `confirm` — preview what that would
/// discard.
///
/// # Errors
///
/// Returns the [`Problem`] a surface renders when the recorded choice cannot be read or a
/// file cannot be written.
pub(super) fn reset(ctx: &Ctx, confirm: bool) -> Result<ResetReport, Box<Problem>> {
    let selection = super::quality::load_selection(ctx)?;
    let record = super::targets::beside_env(ctx, "materialised.json");
    let into = ctx.settings.stack_dir.as_deref();

    let reverted = if confirm {
        super::materialise::reset_stack(ctx.stack, into, record.as_deref(), Some(&selection))
            .map(|(_, edits)| edits)
    } else {
        super::materialise::pending_reverts(ctx.stack, into, record.as_deref(), Some(&selection))
    }
    .map_err(|failure| Box::new(failure.problem()))?;

    Ok(ResetReport {
        reverted,
        confirmed: confirm,
    })
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    use include_dir::{include_dir, Dir};

    use super::reset;
    use crate::app::Ctx;
    use crate::config::Settings;
    use crate::platform::Environment;
    use crate::stack::Source;
    use crate::test_support::{spoke, Reporting, Scripted};

    static STACKLET: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/tests/fixtures/stacklet");

    /// A directory of this test's own, and the env-file path within it.
    fn scratch(name: &str) -> (PathBuf, PathBuf) {
        let dir = std::env::temp_dir().join(format!("lemonfiber-reset-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        (dir.join("stack"), dir.join(".env"))
    }

    /// A context operating the embedded fixture stack, materialised under `into`.
    fn ctx(into: Option<PathBuf>, env: Option<PathBuf>) -> Ctx {
        Ctx::new(
            Arc::new(Scripted(Ok(spoke("")))),
            Arc::new(Reporting::absent()),
            Arc::new(crate::adapters::System),
            Arc::new(crate::adapters::Disk),
            Source::Embedded(&STACKLET),
            Settings {
                stack_dir: into,
                env_file: env,
                ..Settings::default()
            },
            Environment::LinuxNative,
        )
    }

    #[test]
    fn a_confirmed_reset_reverts_an_edited_file_to_lemonfibers() {
        let (into, env) = scratch("confirm");
        // Materialise, then the operator edits a file.
        let _ = reset(&ctx(Some(into.clone()), Some(env.clone())), true);
        let edited = "services:\n  sonarr:\n    image: my-own\n";
        let _ = std::fs::write(into.join("compose.yaml"), edited);

        let report = reset(&ctx(Some(into.clone()), Some(env)), true).unwrap_or_default();
        assert!(report.confirmed);
        assert!(
            report
                .reverted
                .iter()
                .any(|edit| edit.path == "compose.yaml"),
            "the reverted edit is named"
        );
        // The edit is gone — the file is lemonfiber's own again.
        assert_ne!(
            std::fs::read_to_string(into.join("compose.yaml")).unwrap_or_default(),
            edited
        );
    }

    #[test]
    fn an_unconfirmed_reset_previews_and_writes_nothing() {
        let (into, env) = scratch("preview");
        let _ = reset(&ctx(Some(into.clone()), Some(env.clone())), true);
        let edited = "services:\n  sonarr:\n    image: my-own\n";
        let _ = std::fs::write(into.join("compose.yaml"), edited);

        let report = reset(&ctx(Some(into.clone()), Some(env)), false).unwrap_or_default();
        assert!(!report.confirmed);
        assert!(report
            .reverted
            .iter()
            .any(|edit| edit.path == "compose.yaml"));
        // The preview touched nothing.
        assert_eq!(
            std::fs::read_to_string(into.join("compose.yaml")).unwrap_or_default(),
            edited
        );
    }

    #[test]
    fn a_reset_with_nowhere_to_write_is_an_error() {
        // An embedded stack with no directory to materialise into cannot be reset.
        assert!(reset(&ctx(None, None), true).is_err());
    }

    #[test]
    fn a_reset_over_an_unreadable_choice_is_an_error() {
        let (into, env) = scratch("bad-choice");
        // A present-but-corrupt recorded choice cannot be read, so the reset stops rather
        // than guessing the state it would restore.
        let _ = std::fs::write(Path::new(&env).with_file_name("quality.json"), "not json");
        assert!(reset(&ctx(Some(into), Some(env)), true).is_err());
    }
}
