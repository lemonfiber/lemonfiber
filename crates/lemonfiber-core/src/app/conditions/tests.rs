use super::{dated, load, save, wrong};
use crate::app::fixtures::{ctx_at, scratch};
use crate::condition::{Conditions, Fault};
use crate::doctor::{Category, Finding, Verdict};
use crate::error::{Code, Problem, Remedy, Severity};
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
    let written_dir = scratch("corrupt");
    let written = written_dir.join("conditions.json");
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

const CODE: Code = Code::new("storage.full");

/// A finding whose verdict carries the full four parts a problem is made of.
fn found(verdict: Verdict) -> Finding {
    Finding {
        check: "storage.space".to_owned(),
        category: Category::Storage,
        title: "room on the volume".to_owned(),
        verdict,
        service: None,
        service_name: None,
        caused_by: None,
        said: None,
        onset: None,
        origin: crate::origin::Origin::Bundled,
    }
}

/// The problem a full volume amounts to.
fn full() -> Problem {
    Problem::new(
        CODE,
        "the volume has no room left",
        "nothing can be written, so imports will fail where they stand",
        Remedy::new("delete something, or move the library"),
    )
}

#[test]
fn what_a_finding_meant_survives_into_the_condition_it_raises() {
    // The verdict states what happened, what it means and what to do; the
    // condition is what every later surface reads. Keeping two of the three
    // here is how a doctor screen and a dashboard come to disagree about the
    // same problem.
    let remembered = wrong(&found(Verdict::Fail(full())));
    let parts = remembered.map(|fault| (fault.summary, fault.meaning));
    assert_eq!(
        parts,
        Some((
            "the volume has no room left".to_owned(),
            "nothing can be written, so imports will fail where they stand".to_owned(),
        ))
    );
}

#[test]
fn a_check_that_found_nothing_wrong_remembers_nothing() {
    // A pass and a skip say nothing is wrong and nothing was looked at, and an
    // unverified check found no fault — only that it could not say.
    for verdict in [
        Verdict::Pass { note: None },
        Verdict::Skipped {
            reason: "no volume configured".to_owned(),
        },
        Verdict::Unverified {
            reason: "the volume could not be read".to_owned(),
            remedy: Remedy::new("check the path exists"),
        },
    ] {
        assert!(wrong(&found(verdict)).is_none());
    }
}

/// A store holding the full volume as wrong since a fixed moment.
fn full_since(at: &str) -> Conditions {
    let mut conditions = Conditions::new();
    conditions.observe(
        "storage.space",
        wrong(&found(Verdict::Fail(full()))).as_ref(),
        at,
    );
    conditions
}

#[test]
fn a_finding_in_trouble_carries_when_the_stack_first_saw_it_wrong() {
    // Read back from the store rather than stamped by this run, so the doctor and the
    // health summary name one moment for one check, and a restart does not move it.
    let ctx = ctx_at("dated-standing");
    save(&ctx, &full_since("1000"));

    let found = dated(&ctx, vec![found(Verdict::Fail(full()))]);

    assert_eq!(
        found.first().and_then(|one| one.onset.clone()),
        Some("1000".to_owned())
    );
}

#[test]
fn a_check_that_came_right_and_went_wrong_again_carries_the_later_time() {
    let ctx = ctx_at("dated-again");
    let mut conditions = full_since("1000");
    conditions.observe("storage.space", None, "1500");
    save(&ctx, &conditions);

    let found = dated(&ctx, vec![found(Verdict::Fail(full()))]);

    assert_eq!(
        found.first().and_then(|one| one.onset.clone()),
        Some(ctx.stamp())
    );
    assert_ne!(
        ctx.stamp(),
        "1000",
        "the fixture's clock would hide the difference"
    );
}

#[test]
fn a_finding_first_seen_now_is_dated_now_and_the_next_run_reads_it_back() {
    let ctx = ctx_at("dated-first");

    let first = dated(&ctx, vec![found(Verdict::Fail(full()))]);

    assert_eq!(
        first.first().and_then(|one| one.onset.clone()),
        Some(ctx.stamp())
    );
    assert_eq!(
        load(&ctx)
            .get("storage.space")
            .map(|condition| condition.since.clone()),
        Some(ctx.stamp())
    );
}

#[test]
fn a_finding_that_says_nothing_is_wrong_carries_no_onset() {
    let ctx = ctx_at("dated-passing");
    save(&ctx, &full_since("1000"));

    let found = dated(&ctx, vec![found(Verdict::Pass { note: None })]);

    assert_eq!(found.first().and_then(|one| one.onset.clone()), None);
}

#[test]
fn a_save_leaves_what_another_run_wrote_meanwhile() {
    // A dashboard holds the store across a whole refresh. A diagnosis that wrote in
    // the middle of it would otherwise lose its fault to the dashboard's save, and the
    // next diagnosis would date that fault afresh.
    let ctx = ctx_at("save-merges");
    let mut held = load(&ctx);

    save(&ctx, &full_since("1000"));
    held.observe(
        "queue.stalled",
        Some(&Fault::new(
            "queue.stalled",
            Severity::Warning,
            "two downloads have not moved",
            "nothing is arriving for them",
            "check the indexer is answering",
        )),
        "2000",
    );
    save(&ctx, &held);

    let read_back = load(&ctx);
    assert_eq!(
        read_back
            .get("storage.space")
            .map(|condition| condition.since.clone()),
        Some("1000".to_owned())
    );
    assert!(read_back.get("queue.stalled").is_some());
}
