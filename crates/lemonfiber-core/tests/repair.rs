//! Carrying out repairs, driven end to end through the runner.
//!
//! From here rather than from a `#[cfg(test)]` module for the reason the provider and
//! credential checks are: the app layer is compiled twice, and a path exercised only
//! in-crate has its coverage counted from the copy that never ran.
//!
//! The check driven is written for the purpose rather than being one of the nine real
//! ones. What is being proved is the *sequence* — offer, confirm, act, prove, remember —
//! and a test that needed a live VPN gateway and a torrent client to reach it would be a
//! test nobody writes, which is exactly how the two defects this runner was rewritten to
//! fix got in.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use lemonfiber_core::app::repair::{mend, mending};
use lemonfiber_core::app::Ctx;
use lemonfiber_core::config::Settings;
use lemonfiber_core::doctor::{Category, Check, Finding, Mend, Verdict};
use lemonfiber_core::error::{Code, Problem, Remedy, Severity};
use lemonfiber_core::platform::Environment;
use lemonfiber_core::repair::{Attempt, Outcome, Repair, Stance};
use lemonfiber_core::stack::Source;

/// The repository's own stack, so the checks assemble against a real description.
fn project() -> &'static Path {
    static PROJECT: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    PROJECT
        .get_or_init(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/media-stack"))
}

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

/// Checks whose repair leaves the fault standing however often it runs.
fn checks(attempt: Attempt) -> Vec<Box<dyn Check>> {
    vec![Box::new(Sticky::new(attempt))]
}

/// Checks whose repair actually works.
fn settling() -> Vec<Box<dyn Check>> {
    vec![Box::new(Sticky::settling(Attempt::Carried))]
}

/// A run that was not told to act says what could be put right and puts none of it right.
/// That is the default, and the default is what most runs are.
#[tokio::test]
async fn a_run_that_may_not_act_offers_and_changes_nothing() {
    let report = mending(
        &ctx("report-only"),
        &checks(Attempt::Carried),
        Stance::ReportOnly,
        |_| true,
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
    let report = mending(&ctx("fixed"), &settling(), Stance::Unattended, |_| true).await;

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
    let first = mending(&context, &checks(Attempt::Carried), Stance::Ask, |_| false).await;

    assert_eq!(
        first.mended.first().map(|mended| &mended.outcome),
        Some(&Outcome::Declined)
    );

    // The next run does not ask again, because nothing has changed since they said no.
    let again = mending(&context, &checks(Attempt::Carried), Stance::Ask, |_| true).await;
    assert!(again.offered.is_empty(), "it was already declined");
}

/// A repair that stopped partway says what it left behind, and is not judged by asking the
/// check again — what the operator needs is the state it was left in.
#[tokio::test]
async fn a_repair_that_stopped_says_what_it_left() {
    let report = mending(
        &ctx("stopped"),
        &checks(Attempt::Stopped {
            leaving: "half of it".to_owned(),
        }),
        Stance::Unattended,
        |_| true,
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
        let report = mending(
            &context,
            &checks(Attempt::Carried),
            Stance::Unattended,
            |_| true,
        )
        .await;
        assert_eq!(
            report.mended.first().map(|mended| &mended.outcome),
            Some(&Outcome::FixFailed),
            "the check this test drives is never actually put right"
        );
    }

    let past = mending(
        &context,
        &checks(Attempt::Carried),
        Stance::Unattended,
        |_| true,
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
#[tokio::test]
async fn a_stack_with_nothing_mendable_offers_nothing() {
    let report = mend(&ctx("real"), Stance::ReportOnly, false, |_| true).await;

    assert!(report.is_ok_and(|report| report.offered.is_empty()));
}
