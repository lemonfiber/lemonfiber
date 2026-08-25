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

mod common;

use common::stack::project;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use lemonfiber_core::app::repair::{mend, mending, Confirm, Consent, Report};
use lemonfiber_core::app::{dispatch, Command, Ctx};
use lemonfiber_core::config::Settings;
use lemonfiber_core::doctor::{Category, Check, Finding, Mend, Verdict};
use lemonfiber_core::error::{Code, Problem, Remedy, Severity};
use lemonfiber_core::platform::Environment;
use lemonfiber_core::repair::{Attempt, Outcome, Repair, Stance, Writing};
use lemonfiber_core::stack::Source;

/// A context whose records land in a scratch directory of this test's own.
fn ctx(name: &str) -> Ctx {
    let dir = std::env::temp_dir().join(format!("lemonfiber-repair-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    Ctx::new(
        Arc::new(lemonfiber_core::adapters::Local),
        Arc::new(lemonfiber_core::adapters::Daemon::local()),
        Arc::new(lemonfiber_core::adapters::System),
        Arc::new(lemonfiber_core::adapters::Disk),
        Source::External(project()),
        Settings {
            env_file: Some(dir.join(".env")),
            ..Settings::default()
        },
        Environment::MacOs,
    )
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
