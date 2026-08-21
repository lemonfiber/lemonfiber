//! What the operator has been told, and what is still owed them, kept between
//! runs.
//!
//! The outbox was written to survive a channel that was down — an alert decided
//! and not yet delivered is held rather than dropped — and it could not, because
//! nothing wrote it anywhere. Every run started with an empty one, which means a
//! fault that arrived while a channel was refusing was owed until the process
//! ended and then forgotten, and a condition that resolved before anybody read it
//! left no history at all.
//!
//! Kept beside the configuration, with the conditions it is read against: the two
//! are one picture of what has happened, and a restore that brought back one
//! without the other would report every standing fault as new.

use crate::alert::Outbox;

use super::Ctx;

/// What the last run left, or an empty outbox where there is none.
///
/// An unreadable one costs the operator a repeat of something they were already
/// told, which is tiresome and the safe direction — the alternative is a fault
/// nobody hears about because a file said it had been delivered.
#[must_use]
pub(crate) fn load(ctx: &Ctx) -> Outbox {
    super::record::kept(path(ctx).as_deref())
}

/// Write it where the next run will read it.
pub(crate) fn save(ctx: &Ctx, outbox: &Outbox) {
    // Best effort, like the conditions it accompanies: a refresh that could not
    // write its history is one the next refresh starts afresh from.
    let _ = super::record::keep(path(ctx).as_deref(), outbox);
}

/// Where it is kept: beside the environment file, or nowhere on a machine with
/// nothing configured — which has nobody to owe anything to either.
fn path(ctx: &Ctx) -> Option<std::path::PathBuf> {
    super::targets::beside_env(ctx, "outbox.json")
}

#[cfg(test)]
mod tests {
    use super::{load, save};
    use crate::alert::{Alert, Moment, Outbox};
    use crate::test_support::a_context;

    /// A context whose environment file is in an emptied scratch directory.
    fn ctx_at(name: &str) -> crate::app::Ctx {
        let dir =
            std::env::temp_dir().join(format!("lemonfiber-outbox-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        a_context()
            .runner(std::sync::Arc::new(crate::test_support::Scripted(Ok(
                crate::test_support::spoke(""),
            ))))
            .settings(crate::config::Settings {
                env_file: Some(dir.join(".env")),
                ..crate::config::Settings::default()
            })
            .build()
    }

    /// One alert about the given check.
    fn alert(check: &str) -> Alert {
        Alert {
            check: check.to_owned(),
            kind: "service.down".to_owned(),
            moment: Moment::Onset,
            severity: crate::error::Severity::Warning,
            summary: "something happened".to_owned(),
            remedies: vec!["do this".to_owned()],
            affected: vec![check.to_owned()],
        }
    }

    #[test]
    fn what_is_owed_survives_the_run_that_owed_it() {
        // The case the outbox was written for and could not do: a channel refuses,
        // the alert is held, and the process ends. Without this it was forgotten.
        let ctx = ctx_at("owed");
        let mut outbox = Outbox::new();
        outbox.owe(vec![alert("service.sonarr")]);
        save(&ctx, &outbox);

        let read_back = load(&ctx);
        assert!(read_back.owes_anything());
        assert_eq!(read_back.owing().len(), 1);
    }

    #[test]
    fn what_was_delivered_stays_in_the_history() {
        // A condition that resolved before anybody read it is still something they
        // are owed the sight of.
        let ctx = ctx_at("history");
        let mut outbox = Outbox::new();
        outbox.owe(vec![alert("service.sonarr")]);
        outbox.delivered(&|_| 0);
        save(&ctx, &outbox);

        assert_eq!(load(&ctx).history().len(), 1);
    }

    #[test]
    fn a_machine_that_has_been_told_nothing_starts_with_nothing() {
        assert!(!load(&ctx_at("fresh")).owes_anything());
    }
}
