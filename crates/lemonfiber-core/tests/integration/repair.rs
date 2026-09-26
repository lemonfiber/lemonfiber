//! Carrying out repairs, driven end to end through the runner.
//!
//! From here rather than from a `#[cfg(test)]` module for the reason the provider and
//! credential checks are: the app layer is compiled twice, and a path exercised only
//! in-crate has its coverage counted from the copy that never ran.
//!
//! The check driven is written for the purpose rather than being one of the ten real
//! ones. What is being proved is the *sequence* — offer, confirm, act, prove, remember —
//! and a test that needed a live VPN gateway and a torrent client to reach it would be a
//! test nobody writes, which is exactly how the two defects this runner was rewritten to
//! fix got in.

use common::tunnel;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use crate::common;
use async_trait::async_trait;
use lemonfiber_core::app::{dispatch, Command, Ctx};
use lemonfiber_core::config::{Protocols, Settings};
use lemonfiber_core::doctor::{Category, Check, Finding, Mend, Verdict};
use lemonfiber_core::error::{Code, Problem, Remedy, Severity};
use lemonfiber_core::repair::run::{mend, mending, Confirm, Consent, Report};
use lemonfiber_core::repair::{Attempt, Outcome, Repair, Stance, Writing};

/// A context whose records land in a scratch directory of this test's own.
fn ctx(name: &str) -> Ctx {
    let dir = lemonfiber_fixtures::scratch::Scratch::new(&format!("repair-{name}")).kept();
    lemonfiber_testing::a_live_context()
        .settings(Settings {
            env_file: Some(dir.join(".env")),
            ..Settings::default()
        })
        .build()
}

/// The same context over an engine the test keeps hold of, and a stack whose
/// torrents the VPN category applies to.
///
/// The engine is handed over as an `Arc` the caller keeps, so what state a run left
/// the tunnel in can be asked once the run is over. The transport is a silent fake
/// because none of these tests is about what an HTTP service answers, and a suite
/// assembled for real would otherwise reach the network.
fn ctx_over(name: &str, engine: Arc<tunnel::Fake>) -> Ctx {
    let dir = lemonfiber_fixtures::scratch::Scratch::named(&format!("repair-{name}")).kept();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    lemonfiber_testing::a_live_context()
        .engine(engine)
        .settings(Settings {
            env_file: Some(dir.join(".env")),
            // Torrents configured, or the VPN category does not apply and the
            // killswitch is never reached whatever this run was asked for.
            protocols: Protocols::both(),
            ..Settings::default()
        })
        .build()
        .with_http(lemonfiber_fixtures::http::Fake::silent())
}

/// A gateway and a client, both up and both reporting the tunnel's address — the
/// stack the killswitch test has something to prove against.
fn contained() -> Vec<tunnel::Behavior> {
    vec![
        tunnel::Behavior::up("gluetun", Some("185.65.1.1")),
        tunnel::Behavior::up("qbittorrent", Some("185.65.1.1")),
    ]
}

/// The check this test drives, and the repair it offers for what it finds.
///
/// It always finds the same thing wrong, so a run always has something to offer — and it
/// counts what it was asked to do, which is how "was this carried out?" is answered without
/// reaching for anything real.
struct Sticky {
    attempt: Attempt,
    /// Whether putting it right actually settles it, which is what the second look asks.
    settles: bool,
    mended: AtomicUsize,
}

impl Sticky {
    /// One whose repair works: after it has been mended, it finds nothing wrong.
    fn settling(attempt: Attempt) -> Self {
        Self {
            attempt,
            settles: true,
            mended: AtomicUsize::new(0),
        }
    }

    /// One whose repair does not: it keeps finding the same thing wrong however often it
    /// is mended, which is the fault lemonfiber eventually has to admit it cannot fix.
    fn new(attempt: Attempt) -> Self {
        Self {
            attempt,
            settles: false,
            mended: AtomicUsize::new(0),
        }
    }
}

/// What it says is wrong.
const CHECK: &str = "test.always-wrong";

#[async_trait]
impl Check for Sticky {
    fn category(&self) -> Category {
        Category::Vpn
    }

    async fn run(&self) -> Vec<Finding> {
        if self.settles && self.mended.load(Ordering::Relaxed) > 0 {
            return vec![Finding::in_category(
                Category::Vpn,
                CHECK,
                "something this test can mend",
                Verdict::Pass { note: None },
            )];
        }
        vec![Finding::in_category(
            Category::Vpn,
            CHECK,
            "something this test can mend",
            Verdict::Warn(Problem::new(
                Code::new("TEST-1"),
                Severity::Warning,
                "it is wrong",
                "it matters",
                Remedy::new("put it right"),
            )),
        )]
    }

    fn mender(&self) -> Option<&dyn Mend> {
        Some(self)
    }
}

#[async_trait]
impl Mend for Sticky {
    fn repairs(&self, found: &[Finding]) -> Vec<Repair> {
        found
            .iter()
            .filter(|finding| finding.check == CHECK)
            .map(|finding| Repair {
                check: finding.check.clone(),
                does: "put it right".to_owned(),
                effects: Vec::new(),
                reversible: false,
            })
            .collect()
    }

    async fn mend(&self, _repair: &Repair) -> Attempt {
        self.mended.fetch_add(1, Ordering::Relaxed);
        self.attempt.clone()
    }
}

/// Run and prove over the same checks.
///
/// One set for both halves, deliberately: what says whether a repair settled the fault is
/// the check's own state after being mended, so proving against a second, freshly built
/// set would be asking a check nobody had repaired.
async fn drive(
    ctx: &Ctx,
    checks: &[Box<dyn Check>],
    stance: Stance,
    confirm: &dyn Confirm,
) -> Report {
    // No services: these checks are about the errand rather than about anything running,
    // so there is nothing for one finding to be downstream of.
    mending(ctx, &[], checks, checks, stance, confirm).await
}

/// Answers every question the same way.
struct Always(bool);

impl Confirm for Always {
    fn agreed(&self, _repair: &Repair) -> bool {
        self.0
    }
}

/// Checks whose repair leaves the fault standing however often it runs.
fn checks(attempt: Attempt) -> Vec<Box<dyn Check>> {
    vec![Box::new(Sticky::new(attempt))]
}

/// Checks whose repair actually works.
fn settling() -> Vec<Box<dyn Check>> {
    vec![Box::new(Sticky::settling(Attempt::carried()))]
}

/// A run that was not told to act says what could be put right and puts none of it right.
/// That is the default, and the default is what most runs are.
#[tokio::test]
async fn a_run_that_may_not_act_offers_and_changes_nothing() {
    let report = drive(
        &ctx("report-only"),
        &checks(Attempt::carried()),
        Stance::ReportOnly,
        &Always(true),
    )
    .await;

    assert!(!report.acted);
    assert_eq!(report.offered.len(), 1);
    assert!(report.mended.is_empty(), "nothing was carried out");
}

/// Carried out, and then proved: the check that raised the finding is asked again, and only
/// its answer earns `Fixed`.
#[tokio::test]
async fn a_repair_that_worked_is_reported_as_fixed() {
    let report = drive(
        &ctx("fixed"),
        &settling(),
        Stance::Unattended,
        &Always(true),
    )
    .await;

    assert_eq!(
        report.mended.first().map(|mended| &mended.outcome),
        Some(&Outcome::Fixed)
    );
}

/// Said no, and remembered as such — so it stops being offered until the fault has been
/// away and genuinely come back.
#[tokio::test]
async fn a_declined_repair_is_left_alone_and_not_offered_again() {
    let context = ctx("declined");
    let first = drive(
        &context,
        &checks(Attempt::carried()),
        Stance::Ask,
        &Always(false),
    )
    .await;

    assert_eq!(
        first.mended.first().map(|mended| &mended.outcome),
        Some(&Outcome::Declined)
    );

    // The next run does not ask again, because nothing has changed since they said no.
    let again = drive(
        &context,
        &checks(Attempt::carried()),
        Stance::Ask,
        &Always(true),
    )
    .await;
    assert!(again.offered.is_empty(), "it was already declined");
}

/// A repair that must not go ahead is never attempted. What the operator set is theirs,
/// and lemonfiber putting its own value back over it — however sure it is — is the
/// behaviour that makes people stop trusting a tool that changes things.
#[tokio::test]
async fn a_repair_that_would_write_over_an_operators_own_change_is_refused() {
    /// A check whose repair would touch something the operator owns.
    struct Theirs(Sticky);

    #[async_trait]
    impl Check for Theirs {
        fn category(&self) -> Category {
            self.0.category()
        }
        async fn run(&self) -> Vec<Finding> {
            self.0.run().await
        }
        fn mender(&self) -> Option<&dyn Mend> {
            Some(self)
        }
    }

    #[async_trait]
    impl Mend for Theirs {
        fn repairs(&self, found: &[Finding]) -> Vec<Repair> {
            self.0.repairs(found)
        }
        async fn mend(&self, repair: &Repair) -> Attempt {
            self.0.mend(repair).await
        }
        async fn may_proceed(&self, _repair: &Repair) -> Writing {
            Writing::Adopted
        }
    }

    let checks: Vec<Box<dyn Check>> = vec![Box::new(Theirs(Sticky::new(Attempt::carried())))];
    let report = drive(&ctx("theirs"), &checks, Stance::Unattended, &Always(true)).await;

    assert_eq!(
        report.mended.first().map(|mended| &mended.outcome),
        Some(&Outcome::WouldOverwrite)
    );
}

/// A repair that stopped partway says what it left behind, and is not judged by asking the
/// check again — what the operator needs is the state it was left in.
#[tokio::test]
async fn a_repair_that_stopped_says_what_it_left() {
    let report = drive(
        &ctx("stopped"),
        &checks(Attempt::Stopped {
            leaving: "half of it".to_owned(),
        }),
        Stance::Unattended,
        &Always(true),
    )
    .await;

    assert_eq!(
        report.mended.first().map(|mended| &mended.outcome),
        Some(&Outcome::Stopped {
            leaving: "half of it".to_owned()
        })
    );
}

/// A repair that ran and left the fault standing spends one of the few attempts it is
/// given; after enough of them it stops being offered, and the run hands over the support
/// bundle rather than going quiet about a fault it cannot mend.
#[tokio::test]
async fn a_repair_that_keeps_failing_stops_being_offered_and_says_where_to_go() {
    let context = ctx("exhausted");

    // Each run mends and the check keeps failing, so each spends one attempt.
    for _ in 0..3 {
        let report = drive(
            &context,
            &checks(Attempt::carried()),
            Stance::Unattended,
            &Always(true),
        )
        .await;
        assert_eq!(
            report.mended.first().map(|mended| &mended.outcome),
            Some(&Outcome::FixFailed),
            "the check this test drives is never actually put right"
        );
    }

    let past = drive(
        &context,
        &checks(Attempt::carried()),
        Stance::Unattended,
        &Always(true),
    )
    .await;
    assert!(past.offered.is_empty(), "it has had its chances");
    assert_eq!(
        past.beyond.first().map(|beyond| beyond.check.as_str()),
        Some(CHECK)
    );
    assert!(past
        .beyond
        .first()
        .and_then(|beyond| beyond.remedy.detail.as_deref())
        .is_some_and(|detail| detail.contains("lemonfiber support")));
}

/// The whole errand over the real checks: nothing here can be mended, so nothing is
/// offered — and the run says so rather than failing.
///
/// Also the only path that assembles its own checks to prove with, since a real run has no
/// caller to hand it any.
#[tokio::test]
async fn a_stack_with_nothing_mendable_offers_nothing() {
    let context = ctx("real");
    let report = mend(&context, Stance::ReportOnly, false, &Always(true)).await;
    assert!(report.is_ok_and(|report| report.offered.is_empty()));

    // Asked to act rather than only look, and still with nothing to act on.
    let acting = mend(&context, Stance::Unattended, false, &Always(true)).await;
    assert!(acting.is_ok_and(|report| report.mended.is_empty() && report.acted));
}

/// The offer, asked for the way every surface asks for it.
///
/// Through the dispatcher rather than through [`mend`] directly, because that is the
/// one entry a browser and a command line both go in through — and from here as well
/// as in-crate, for the reason at the top of this file.
///
/// Nothing is wrong on the machine running this that lemonfiber could put right, so
/// what is being held is the shape of the answer: its own kind, and a run that acted
/// on none of what it found.
#[tokio::test]
async fn a_dispatched_offer_answers_under_its_own_kind_and_acts_on_none_of_it() {
    let json = dispatch(
        Command::Repair {
            consent: Consent::Offer,
            disruptive: false,
        },
        &ctx("dispatched-offer").with_http(lemonfiber_fixtures::http::Fake::silent()),
    )
    .await
    .ok()
    .map(|outcome| outcome.envelope().to_json().unwrap_or_default())
    .unwrap_or_default();

    assert!(json.contains(r#""kind":"repair""#), "{json}");
    assert!(json.contains(r#""acted":false"#), "{json}");
    // The offer names itself on the way out, which is the whole of what consent
    // crossing a request boundary has to be able to point at.
    assert!(json.contains(r#""agreement":"#), "{json}");
}

/// A run that may not act is refused the checks that disturb, and the tunnel it
/// would have taken away is still up.
///
/// The offer half of a repair is what an operator reads before deciding anything,
/// and the killswitch test proves itself by dropping the default route out from
/// under the download client. A run that did that to say what it *would* do has
/// already done something — and the release search beside it has spent one of the
/// indexers' daily allowance to say it.
#[tokio::test]
async fn an_offer_is_refused_the_checks_that_disturb_and_leaves_the_tunnel_up() {
    let engine = Arc::new(tunnel::Fake::linked(contained(), tunnel::Link::holding()));
    let context = ctx_over("offer-disruptive", Arc::clone(&engine));

    let refused = mend(&context, Stance::ReportOnly, true, &Always(true)).await;

    assert!(
        !engine.was_dropped(),
        "the tunnel was taken away to make an offer nobody had agreed to"
    );
    assert!(!engine.is_dropped(), "and it is still up");
    assert_eq!(
        refused.err().map(|problem| problem.code),
        Some(lemonfiber_core::error::codes::repair::OFFER_CANNOT_DISTURB)
    );
}

/// The same request with the agreement on it is not refused, and the widening it
/// asked for reaches the checks.
///
/// Which is the half of the rule that matters. What is refused is the pair, not the
/// widening: an operator who wants the killswitch proven while their stack is put
/// right still gets it, on the run that may act on what it finds.
#[tokio::test]
async fn a_run_that_may_act_still_gets_the_checks_that_disturb() {
    let engine = Arc::new(tunnel::Fake::linked(contained(), tunnel::Link::holding()));
    let context = ctx_over("standing-disruptive", Arc::clone(&engine));

    let report = mend(&context, Stance::Unattended, true, &Always(true)).await;

    assert!(report.is_ok_and(|report| report.acted), "it was allowed to");
    assert!(
        engine.was_dropped(),
        "the killswitch was proven the only way it can be"
    );
    assert!(!engine.is_dropped(), "and the tunnel was put back");
}

/// A declaration reaches all the way through the sequence: the repair is agreed to,
/// refused by the declaration, reported as refused, and never carried out.
///
/// From here rather than in-crate for the reason at the top of this file. The gate
/// itself answers correctly whichever copy asks it — that is asserted beside it — and
/// what this holds is that the runner *acts* on the answer, which is a claim about the
/// sequence the binary ships.
///
/// The outcome is `Unmanaged` rather than the answer given for a value somebody
/// changed: an operator told the wrong one goes looking for a change they did not make.
/// And the mender is asked whether it was ever told to write, because a run that wrote
/// and then reported `Unmanaged` would carry exactly the report this one does.
#[tokio::test]
async fn a_repair_the_declaration_refuses_is_reported_as_such_and_never_carried_out() {
    let (report, wrote) = declared(&["sonarr"], "mending-unmanaged", &["sonarr"]).await;

    assert_eq!(
        report.mended.first().map(|mended| &mended.outcome),
        Some(&Outcome::Unmanaged),
        "{report:?}"
    );
    assert!(
        !wrote.load(Ordering::Relaxed),
        "the mender was asked to write an area the operator declared theirs"
    );
}

/// And a declaration about somewhere else stops nothing: the same repair, the same
/// agreement, and the mender is asked to carry it out.
///
/// The half that keeps the rule honest. A gate that refused everything would pass the
/// test above and be useless, and an operator who declared one service theirs has said
/// nothing about the rest of their stack.
#[tokio::test]
async fn a_repair_writing_outside_every_declared_area_is_carried_out() {
    let (report, wrote) = declared(&["radarr"], "mending-elsewhere", &["sonarr"]).await;

    assert!(
        wrote.load(Ordering::Relaxed),
        "the mender was never asked to write: {report:?}"
    );
    assert_ne!(
        report.mended.first().map(|mended| &mended.outcome),
        Some(&Outcome::Unmanaged),
        "{report:?}"
    );
}

/// A name beneath a declared one is covered by it, the way every other write point
/// reads a declaration.
#[tokio::test]
async fn a_repair_writing_beneath_a_declared_area_is_refused_too() {
    let (report, wrote) = declared(
        &["config"],
        "mending-beneath",
        &["config/recyclarr/recyclarr.yml"],
    )
    .await;

    assert_eq!(
        report.mended.first().map(|mended| &mended.outcome),
        Some(&Outcome::Unmanaged),
        "{report:?}"
    );
    assert!(!wrote.load(Ordering::Relaxed), "{report:?}");
}

/// A mender that writes nothing an operator could have declared theirs is never held
/// by a declaration, whatever they wrote down.
#[tokio::test]
async fn a_repair_that_declares_no_write_is_not_held_by_anything() {
    let (report, wrote) = declared(&["sonarr"], "mending-declares-nothing", &[]).await;

    assert!(
        wrote.load(Ordering::Relaxed),
        "the mender was never asked to write: {report:?}"
    );
    assert_ne!(
        report.mended.first().map(|mended| &mended.outcome),
        Some(&Outcome::Unmanaged),
        "{report:?}"
    );
}

/// A consent that read no offer says the offer in front of it still stands.
///
/// The answer [`Confirm`] gives where nobody overrides it, which is the whole reason it
/// has one: a terminal asks about each repair in the same run that looked, so there is
/// no earlier offer for this one to have moved on from. Only consent that crossed a
/// request boundary read an offer this run has since looked again for, and only that
/// has anything to say no about. Asserted rather than left to the default's
/// obviousness, because the day somebody gives the trait an implementor that forgets to
/// override it is the day a stale answer is carried out against an offer nobody is
/// making any more.
#[test]
fn a_consent_that_read_no_offer_says_the_offer_in_front_of_it_still_stands() {
    assert!(
        Always(true).stands(&[]),
        "a run that looked and asked in one go has no older offer to have moved on from"
    );
}

/// The whole sequence over one check whose mender says what it writes, against a
/// context holding the areas the operator declared theirs.
///
/// The flag comes back with the report because the mender itself is handed to the
/// runner and not seen again, and "nothing was written" has to be a fact about the
/// mender rather than something inferred from what the report says.
async fn declared(areas: &[&str], name: &str, writes: &[&str]) -> (Report, Arc<AtomicBool>) {
    let asked = Arc::new(AtomicBool::new(false));
    let mender = Writes {
        areas: writes.iter().map(|area| (*area).to_owned()).collect(),
        asked: Arc::clone(&asked),
    };
    let checks: Vec<Box<dyn Check>> = vec![Box::new(Offering(mender))];
    let mut context = ctx(name);
    context.settings.unmanaged = areas
        .iter()
        .map(|area| ((*area).to_owned(), "mine to tune by hand".to_owned()))
        .collect();

    let report = mending(&context, &[], &checks, &checks, Stance::Ask, &Always(true)).await;
    (report, asked)
}

/// A mender that declares what it would write, and records being asked to write it.
struct Writes {
    /// What a repair here would write to, by the names a declaration uses.
    areas: Vec<String>,
    /// Set the moment it is asked to carry a repair out.
    asked: Arc<AtomicBool>,
}

#[async_trait]
impl Mend for Writes {
    fn repairs(&self, found: &[Finding]) -> Vec<Repair> {
        found
            .iter()
            .map(|finding| Repair {
                check: finding.check.clone(),
                does: "put it right".to_owned(),
                effects: Vec::new(),
                reversible: false,
            })
            .collect()
    }

    async fn mend(&self, _repair: &Repair) -> Attempt {
        self.asked.store(true, Ordering::Relaxed);
        Attempt::carried()
    }

    fn writes_to(&self, _repair: &Repair) -> Vec<String> {
        self.areas.clone()
    }
}

/// A check that always finds the one fault that mender answers for, so a run driven
/// through it always has something to offer.
struct Offering(Writes);

#[async_trait]
impl Check for Offering {
    fn category(&self) -> Category {
        Category::Vpn
    }

    async fn run(&self) -> Vec<Finding> {
        vec![Finding::in_category(
            Category::Vpn,
            CHECK,
            "something this test can mend",
            Verdict::Warn(Problem::new(
                Code::new("TEST-1"),
                Severity::Warning,
                "it is wrong",
                "it matters",
                Remedy::new("put it right"),
            )),
        )]
    }

    fn mender(&self) -> Option<&dyn Mend> {
        Some(&self.0)
    }
}

/// A repair that changed something is recorded, or is stopped saying it could not be.
///
/// Asked through the whole errand as well as in the crate, so every build of the step
/// that records a repair is driven down each of its three ways: nowhere to record it,
/// recorded, and not recordable.
#[tokio::test]
async fn a_repair_whose_change_cannot_be_recorded_is_stopped_saying_so() {
    let changed = || Attempt::Carried {
        changes: vec![lemonfiber_core::journal::Change {
            at: "1".to_owned(),
            operation: lemonfiber_core::repair::OPERATION.to_owned(),
            target: ".env".to_owned(),
            kind: lemonfiber_core::journal::Kind::Set {
                key: "FORWARDED_PORT".to_owned(),
                previous: None,
                current: "51413".to_owned(),
            },
        }],
    };

    let recorded = drive(
        &ctx("recordable"),
        &checks(changed()),
        Stance::Unattended,
        &Always(true),
    )
    .await;
    assert!(
        !matches!(
            recorded.mended.first().map(|mended| &mended.outcome),
            Some(Outcome::Stopped { .. })
        ),
        "{recorded:?}"
    );

    let dir = lemonfiber_fixtures::scratch::Scratch::new("repair-unrecordable").kept();
    let _ = std::fs::create_dir_all(dir.join("journal.jsonl.writing").join("held"));
    let unrecordable = lemonfiber_testing::a_live_context()
        .settings(Settings {
            env_file: Some(dir.join(".env")),
            ..Settings::default()
        })
        .build();
    let stopped = drive(
        &unrecordable,
        &checks(changed()),
        Stance::Unattended,
        &Always(true),
    )
    .await;
    assert!(
        matches!(
            stopped.mended.first().map(|mended| &mended.outcome),
            Some(Outcome::Stopped { leaving }) if leaving.contains("could not be recorded")
        ),
        "{stopped:?}"
    );

    let nowhere = lemonfiber_testing::a_live_context().build();
    let judged = drive(
        &nowhere,
        &checks(changed()),
        Stance::Unattended,
        &Always(true),
    )
    .await;
    assert!(
        !matches!(
            judged.mended.first().map(|mended| &mended.outcome),
            Some(Outcome::Stopped { .. })
        ),
        "{judged:?}"
    );
}
