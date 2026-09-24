use super::{load, save};
use crate::app::fixtures::{ctx_at, scratch};
use crate::condition::{Conditions, Fault};
use crate::error::Severity;
use crate::test_support::a_context;

/// A store with one thing wrong, raised at a fixed moment.
fn stalled() -> Conditions {
    let mut conditions = Conditions::new();
    conditions.observe(
        "queue.stalled",
        Some(&Fault::new(
            "queue.stalled",
            Severity::Warning,
            "two downloads have not moved",
            "nothing is arriving for them",
            "check the indexer is answering",
        )),
        "1000",
    );
    conditions
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
    let ctx = a_context()
        .runner(std::sync::Arc::new(crate::test_support::Scripted(Ok(
            crate::test_support::spoke(""),
        ))))
        .settings(settings)
        .build();
    // Saving is a no-op rather than an error, and loading gives an empty store.
    save(&ctx, &stalled());
    assert!(load(&ctx).is_empty());
}
