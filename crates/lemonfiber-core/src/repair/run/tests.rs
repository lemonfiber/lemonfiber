use super::proving::{judged, proved};
use super::remembering::wrong;
use super::{beyond, putting_right, reversing, Beyond, Consent, NOWHERE_TO_LOOK};
use crate::app::fixtures::ctx_at;
use crate::condition::{Conditions, Fault};
use crate::doctor::{Category, Finding, Verdict};
use crate::error::{Problem, Remedy, Severity};
use crate::repair::{Attempt, Outcome, Repair, ATTEMPTS};

fn problem() -> Problem {
    Problem::new(
        crate::error::codes::vpn::PORT_MISMATCH,
        Severity::Warning,
        "it is on the wrong port",
        "peers cannot reach it",
        Remedy::new("move it"),
    )
}

fn finding(check: &str, verdict: Verdict) -> Finding {
    Finding::in_category(Category::Vpn, check, "the forwarded port", verdict)
}

fn repair(check: &str) -> Repair {
    Repair {
        check: check.to_owned(),
        does: "move the client".to_owned(),
        effects: Vec::new(),
        reversible: false,
    }
}

/// What a run found has to reach the store before anything is offered, or every
/// question asked of it — declined? tried too often? — is asked about a check it has
/// never heard of, and answered no.
#[test]
fn what_a_run_found_is_remembered_before_anything_is_offered() {
    let ctx = ctx_at("repair-remembers");
    let found = vec![
        finding("vpn.port-forward-client", Verdict::Warn(problem())),
        finding("vpn.egress", Verdict::Pass { note: None }),
    ];

    let conditions = super::remembered(&ctx, &found);

    assert!(conditions
        .get("vpn.port-forward-client")
        .is_some_and(crate::condition::Condition::is_raised));
    // A check that has never been wrong gets no entry at all. The store remembers
    // faults, and inventing a cleared condition for something that never failed would
    // fill it with things that never happened — which is also why a repair asks it
    // about the checks that did fail and no others.
    assert!(conditions.get("vpn.egress").is_none());

    // And it survives to the next run, which is the whole point of a store.
    assert!(crate::app::conditions::load(&ctx)
        .get("vpn.port-forward-client")
        .is_some());
}

/// A rehearsal reads the store and does not add to it.
///
/// What this file holds is how often a fault has been seen and how often a fix left
/// it standing, and the offer decides what is worth offering again from those
/// counts. A rehearsal that recorded a sighting would move them, so the next real
/// run would decide differently because somebody had asked a question.
#[test]
fn a_rehearsed_repair_reads_the_store_and_writes_nothing_to_it() {
    let ctx = ctx_at("repair-rehearsed").rehearsing();
    let found = vec![finding("vpn.port-forward-client", Verdict::Warn(problem()))];

    let conditions = super::remembered(&ctx, &found);

    // The report a rehearsal gives is built from the reading, so the reading happens.
    assert!(conditions
        .get("vpn.port-forward-client")
        .is_some_and(crate::condition::Condition::is_raised));
    // Against the real file, not against what the function said it did.
    let kept = crate::app::fixtures::scratch("repair-rehearsed").join("conditions.json");
    assert!(!kept.exists(), "a rehearsal wrote {}", kept.display());
    assert!(crate::app::conditions::load(&ctx)
        .get("vpn.port-forward-client")
        .is_none());
}

/// A pass says nothing is wrong and a skip says there was nothing to look at. An
/// unverified check is the careful one: it could not be established, which is not the
/// same as finding it broken.
#[test]
fn only_a_finding_that_says_something_is_wrong_is_remembered_as_a_fault() {
    assert!(wrong(&finding("a", Verdict::Warn(problem()))).is_some());
    assert!(wrong(&finding("a", Verdict::Fail(problem()))).is_some());
    assert!(wrong(&finding("a", Verdict::Pass { note: None })).is_none());
    assert!(wrong(&finding(
        "a",
        Verdict::Skipped {
            reason: "nothing to look at".to_owned()
        }
    ))
    .is_none());
    assert!(wrong(&finding(
        "a",
        Verdict::Unverified {
            reason: "could not be established".to_owned(),
            remedy: Remedy::new("try again"),
        }
    ))
    .is_none());
}

/// All three answers the proof can give, and what each makes of an attempt that ran.
#[test]
fn an_attempt_and_its_proof_together_say_what_happened() {
    assert_eq!(judged(Attempt::carried(), Some(true)), Outcome::Fixed);
    assert_eq!(judged(Attempt::carried(), Some(false)), Outcome::FixFailed);
    // Could not be established afterwards: neither fixed nor demonstrably still
    // broken, and reported as what it is rather than as the worse of the two.
    assert!(matches!(
        judged(Attempt::carried(), None),
        Outcome::Stopped { .. }
    ));
    // One that stopped is not judged by the proof at all — what it left behind is
    // what the operator needs, whatever the checks say now.
    assert_eq!(
        judged(
            Attempt::Stopped {
                leaving: "half of it".to_owned()
            },
            Some(true)
        ),
        Outcome::Stopped {
            leaving: "half of it".to_owned()
        }
    );
}

/// The question a repair is judged by. Absent means the fault is gone; unverified means
/// nobody can say, which must not be mistaken for either answer.
#[test]
fn whether_a_check_now_passes_has_three_answers() {
    let check = "vpn.port-forward-client";
    assert_eq!(proved(&[], check), Some(true));
    assert_eq!(
        proved(&[finding(check, Verdict::Pass { note: None })], check),
        Some(true)
    );
    assert_eq!(
        proved(&[finding(check, Verdict::Warn(problem()))], check),
        Some(false)
    );
    assert_eq!(
        proved(
            &[finding(
                check,
                Verdict::Unverified {
                    reason: "could not be established".to_owned(),
                    remedy: Remedy::new("try again"),
                }
            )],
            check
        ),
        None
    );
}

/// Said only where something could still have been offered, and only while the fault is
/// actually standing — clearing a condition does not reset its attempts, so a fault that
/// went away would otherwise still be announced as beyond repair.
#[test]
fn only_a_standing_fault_with_a_repair_is_reported_as_beyond_one() {
    let mut conditions = Conditions::new();
    let fault = Fault::new(
        "vpn.port",
        Severity::Warning,
        "wrong port",
        "incoming connections are not reaching the client",
        "move it",
    );
    conditions.observe("vpn.port-forward-client", Some(&fault), "1000");
    for _ in 0..ATTEMPTS {
        conditions.attempted("vpn.port-forward-client");
    }

    let named =
        |beyond: Vec<Beyond>| -> Vec<String> { beyond.into_iter().map(|one| one.check).collect() };

    assert_eq!(
        named(beyond(
            &conditions.all(),
            &[repair("vpn.port-forward-client")]
        )),
        vec!["vpn.port-forward-client".to_owned()]
    );

    // Nothing could have repaired it, so nothing has run out of ways to.
    assert!(named(beyond(&conditions.all(), &[])).is_empty());

    // And once it clears, it is not something that outlasted its repairs any more.
    conditions.observe("vpn.port-forward-client", None, "2000");
    assert!(named(beyond(
        &conditions.all(),
        &[repair("vpn.port-forward-client")]
    ))
    .is_empty());
}

/// A run given no consent offers what it found and puts none of it right, which
/// is what a surface asks for before it has anything to show anybody.
#[tokio::test]
async fn a_run_with_no_consent_offers_and_acts_on_none_of_it() {
    // Read as one value rather than unwrapped through a branch nothing takes:
    // nothing is wrong on this machine that lemonfiber could put right, so the
    // offer is empty — and an empty offer still names itself, because consent
    // to nothing is a thing somebody can give.
    let offer = putting_right(&ctx_at("repair-offering"), &Consent::Offer, false)
        .await
        .ok()
        .map(|report| (report.acted, report.offered.len(), report.agreement));

    assert_eq!(offer, Some((false, 0, crate::repair::agreement(&[]))));
}

/// A run that may not act is refused the checks that disturb, before anything
/// is assembled to run.
///
/// The tunnel staying up is proved from `tests/`, over an engine that records
/// what it was asked to do. What is proved here is that the refusal comes first:
/// this context has no container engine behind it, so a run that reached the
/// checks at all would answer with something else entirely.
#[tokio::test]
async fn an_offer_asked_to_disturb_is_refused_before_a_check_is_built() {
    let refused = putting_right(&ctx_at("repair-offer-disturbing"), &Consent::Offer, true)
        .await
        .err()
        .map(|problem| (problem.code, problem.remedies.len()));

    // Two remedies, because there are two different things the caller might
    // have wanted: the checks' findings, which is a diagnosis, or the repairs
    // carried out, which is the yes.
    assert_eq!(refused, Some((super::OFFER_CANNOT_DISTURB, 2)));
}

/// Consent given for an offer that is not the offer that stands is refused, and
/// nothing is carried out — which is the whole of what a request boundary costs
/// a flow that a terminal gets for nothing.
#[tokio::test]
async fn consent_given_for_an_offer_that_has_moved_on_is_refused() {
    let consent = Consent::Given {
        offer: "deadbeef".to_owned(),
        repairs: vec!["vpn.port-forward-client".to_owned()],
    };
    let refused = putting_right(&ctx_at("repair-stale"), &consent, false)
        .await
        .err()
        .map(|problem| problem.code);

    assert_eq!(refused, Some(super::STALE));
}

/// A run that cannot say where lemonfiber keeps its own files has nothing to
/// read a reversal out of, and says so rather than reporting that there was
/// nothing to put back — which is what a machine with a clean journal says.
#[tokio::test]
async fn an_undo_with_nowhere_to_look_says_so_rather_than_finding_nothing() {
    let refused = reversing(&ctx_at("repair-nowhere"))
        .await
        .err()
        .map(|problem| problem.code);

    assert_eq!(refused, Some(NOWHERE_TO_LOOK));
}

/// With a layout to read, the reversal is the one [`super::retract`] gives, in
/// the report an envelope carries.
#[tokio::test]
async fn an_undo_that_knows_where_to_look_answers_with_what_went_back() {
    let dir = std::env::temp_dir().join(format!(
        "lemonfiber-reversing-{}-{}",
        std::process::id(),
        "layout"
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(dir.join("config"));
    let _ = std::fs::create_dir_all(dir.join("data"));
    let settings = crate::config::Settings {
        env_file: Some(dir.join("config").join(".env")),
        stack_dir: Some(dir.join("data").join("stack")),
        ..crate::config::Settings::default()
    };
    let ctx = crate::test_support::a_context().settings(settings).build();

    // Nothing has been repaired here, so there is nothing to put back — said as
    // an empty reversal rather than as a failure.
    let reversal = reversing(&ctx).await.ok().map(|it| it.reversed.len());

    assert_eq!(reversal, Some(0));
}
