//! What is wrong, kept between runs.
//!
//! Everything the conditions are for needs a memory older than one process. How
//! long something has been broken, whether the operator has already been told,
//! whether a fault is steady or flapping — each is a comparison against a previous
//! run, and a store that started empty every time would answer all three wrongly:
//! nothing has lasted any time, everything is news, and nothing ever flaps.
//!
//! Kept with configuration rather than beside the stack, for the reason the
//! baseline is: it is a memory of what was observed, and a backup that restored
//! the stack without it would report a week-old fault as having just started.
//!
//! Best-effort, both ways. A store that cannot be read is an empty one — worse
//! answers for a run, never a refusal to run — and a store that cannot be written
//! costs the next run its history and nothing else. Neither is worth failing a
//! command over.

use crate::condition::Conditions;

use super::Ctx;

/// Read what the last run left, or an empty store where there is none.
///
/// A store that will not parse is treated as absent rather than reported. Unlike
/// the seeding baseline — where a lost record means an operator's edit could be
/// silently overwritten — the worst a lost condition store costs is that standing
/// faults read as new, which the next run corrects on its own.
#[must_use]
pub fn load(ctx: &Ctx) -> Conditions {
    super::record::kept(path(ctx).as_deref())
}

/// Write the store where the next run will read it.
pub fn save(ctx: &Ctx, conditions: &Conditions) {
    // Best effort on the way out, unlike the records an operator decided: a
    // refresh that could not write its history is one the next refresh starts
    // afresh from, which is a worse picture rather than a wrong claim.
    let _ = super::record::keep(path(ctx).as_deref(), conditions);
}

/// Where the store is kept: beside the environment file, or nowhere on a machine
/// with nothing configured — which has no faults to remember either.
fn path(ctx: &Ctx) -> Option<std::path::PathBuf> {
    super::targets::beside_env(ctx, "conditions.json")
}

#[cfg(test)]
mod tests {
    use super::{load, save};
    use crate::condition::{Conditions, Fault};
    use crate::error::Severity;

    /// A store with one thing wrong, raised at a fixed moment.
    fn stalled() -> Conditions {
        let mut conditions = Conditions::new();
        conditions.observe(
            "queue.stalled",
            Some(&Fault::new(
                "queue.stalled",
                Severity::Warning,
                "two downloads have not moved",
                "check the indexer is answering",
            )),
            "1000",
        );
        conditions
    }

    /// Where a test's scratch store lives. Naming it does not touch it, so a test
    /// can reach the file the module wrote rather than deleting it and proving
    /// something else.
    fn scratch(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("lemonfiber-cond-{}-{name}", std::process::id()))
    }

    /// A context whose environment file is in a scratch directory unique to the
    /// test, emptied first, so the store lands beside it and concurrent tests do
    /// not share one.
    fn ctx_at(name: &str) -> crate::app::Ctx {
        let dir = scratch(name);
        let _ = std::fs::remove_dir_all(&dir);
        let settings = crate::config::Settings {
            env_file: Some(dir.join(".env")),
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
    fn what_one_run_recorded_the_next_one_reads() {
        // The whole point: how long something has been broken is a comparison with a
        // previous run, and there is no previous run without this.
        let ctx = ctx_at("round-trip");
        save(&ctx, &stalled());
        let read_back = load(&ctx);
        assert_eq!(read_back, stalled());
        assert_eq!(
            read_back
                .get("queue.stalled")
                .map(|condition| condition.since.clone()),
            Some("1000".to_owned()),
            "and when it started survives, which is the whole reason to keep it"
        );
    }

    #[test]
    fn a_machine_that_has_never_recorded_anything_starts_empty() {
        assert!(load(&ctx_at("fresh")).is_empty());
    }

    #[test]
    fn a_store_that_will_not_parse_is_an_empty_one_rather_than_a_failure() {
        // Worse answers for a run, never a refusal to run.
        let ctx = ctx_at("corrupt");
        save(&ctx, &stalled());
        let written = scratch("corrupt").join("conditions.json");
        assert!(written.exists(), "the store was written in the first place");
        assert!(
            crate::config::store::write(&written, "not json at all").is_ok(),
            "and is then replaced with something unparsable"
        );
        assert!(load(&ctx).is_empty());
    }

    #[test]
    fn a_machine_with_nothing_configured_has_nowhere_to_keep_one() {
        let settings = crate::config::Settings::default();
        let ctx = crate::app::Ctx::new(
            std::sync::Arc::new(crate::test_support::Scripted(Ok(
                crate::test_support::spoke(""),
            ))),
            std::sync::Arc::new(crate::test_support::Reporting::absent()),
            std::sync::Arc::new(crate::adapters::System),
            std::sync::Arc::new(crate::adapters::Disk),
            crate::test_support::stack(),
            settings,
            crate::platform::Environment::MacOs,
        );
        // Saving is a no-op rather than an error, and loading gives an empty store.
        save(&ctx, &stalled());
        assert!(load(&ctx).is_empty());
    }
}
