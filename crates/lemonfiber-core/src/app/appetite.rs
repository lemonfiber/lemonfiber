//! What the operator asked to hear about, kept between runs.
//!
//! Read wherever a digest is built and written whenever the answer changes, so the
//! preset chosen once at setup is the one still in force a month later — and so
//! the individual events switched on or off since are too.
//!
//! Read best-effort and written strictly, the same way the quality choice is. A
//! run that cannot read the file falls back to the quiet default, which is the
//! safe direction: an operator hears less than they asked for rather than being
//! refused a command. A run that cannot *write* it says so, because that is the
//! operator's explicit action and silently losing it would leave them believing
//! they had changed something they had not.

use std::path::PathBuf;

use crate::alert::Wants;
use crate::config::store;
use crate::error::{Diagnose, Problem};

use super::Ctx;

/// What the operator asked to hear about, or the quiet default where they have not
/// said and where the answer cannot be read.
#[must_use]
pub fn recorded(ctx: &Ctx) -> Wants {
    super::record::kept(path(ctx).as_deref())
}

/// Record the answer where the next run — and a backup — will find it.
///
/// Reported rather than swallowed on failure: this is the operator's explicit
/// action, and an answer that quietly did not persist is worse than one that
/// visibly could not.
///
/// # Errors
///
/// Where there is nowhere configured to keep it, or the file cannot be written.
pub fn record(ctx: &Ctx, wants: &Wants) -> Result<(), Box<Problem>> {
    let path = path(ctx).ok_or_else(|| Box::new(store::Failure::Nowhere.problem()))?;
    store::write(&path, &serde_json::to_string(wants).unwrap_or_default())
        .map_err(|failure| Box::new(failure.problem()))
}

/// Where the answer is kept: beside the environment file, in the configuration
/// directory a backup captures, or nowhere when nothing is configured. Equal to
/// [`crate::config::paths::Paths::notifications`].
fn path(ctx: &Ctx) -> Option<PathBuf> {
    super::targets::beside_env(ctx, "notifications.json")
}

#[cfg(test)]
mod tests {
    use super::{record, recorded};
    use crate::alert::{Appetite, Wants};

    /// Where a test's scratch answer lives. Naming it does not touch it.
    fn scratch(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("lemonfiber-appetite-{}-{name}", std::process::id()))
    }

    /// A context whose environment file is in an emptied scratch directory, so the
    /// answer lands beside it and concurrent tests do not share one.
    fn ctx_at(name: &str) -> crate::app::Ctx {
        let dir = scratch(name);
        let _ = std::fs::remove_dir_all(&dir);
        ctx_with(Some(dir.join(".env")))
    }

    /// A context with the given environment file, or none at all.
    fn ctx_with(env_file: Option<std::path::PathBuf>) -> crate::app::Ctx {
        let settings = crate::config::Settings {
            env_file,
            ..crate::config::Settings::default()
        };
        crate::app::Ctx::new(
            std::sync::Arc::new(crate::test_support::Scripted(Ok(
                crate::test_support::spoke(""),
            ))),
            std::sync::Arc::new(crate::test_support::Reporting::absent()),
            std::sync::Arc::new(crate::adapters::System),
            std::sync::Arc::new(crate::adapters::Disk),
            crate::test_support::stack(),
            settings,
            crate::platform::Environment::MacOs,
        )
    }

    #[test]
    fn the_answer_given_once_is_the_one_in_force_next_run() {
        let ctx = ctx_at("round-trip");
        let mut wants = Wants::preset(Appetite::WithCompletions);
        wants.set("update.available", true);
        assert!(record(&ctx, &wants).is_ok());
        assert_eq!(recorded(&ctx), wants);
    }

    #[test]
    fn a_machine_nobody_has_answered_on_gets_the_quiet_default() {
        assert_eq!(recorded(&ctx_at("fresh")), Wants::default());
    }

    #[test]
    fn an_unreadable_answer_falls_back_rather_than_refusing_the_command() {
        // Hearing less than was asked for is the safe direction; being unable to run
        // a command over a corrupt preferences file is not.
        let ctx = ctx_at("corrupt");
        assert!(record(&ctx, &Wants::preset(Appetite::Everything)).is_ok());
        let written = scratch("corrupt").join("notifications.json");
        assert!(
            written.exists(),
            "the answer was written in the first place"
        );
        assert!(
            crate::config::store::write(&written, "not json at all").is_ok(),
            "and is then replaced with something unparsable"
        );
        assert_eq!(recorded(&ctx), Wants::default());
    }

    #[test]
    fn an_answer_that_cannot_be_written_is_reported_rather_than_swallowed() {
        // Somewhere to keep it, and still no way to write it — a directory sits
        // where the file must go.
        let ctx = ctx_at("blocked");
        let blocked = scratch("blocked").join("notifications.json");
        assert!(
            std::fs::create_dir_all(&blocked).is_ok(),
            "the blocking directory"
        );
        assert!(record(&ctx, &Wants::preset(Appetite::Everything)).is_err());
    }

    #[test]
    fn an_answer_with_nowhere_to_go_is_reported_rather_than_swallowed() {
        // The operator changed something; telling them it worked when it did not is
        // worse than telling them it could not.
        let ctx = ctx_with(None);
        assert!(record(&ctx, &Wants::preset(Appetite::Everything)).is_err());
        assert_eq!(recorded(&ctx), Wants::default());
    }
}
