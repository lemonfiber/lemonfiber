use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use super::Deferred;
use crate::doctor::{examine, Category, Check, Finding, Narrowing, Verdict};

/// A check that passes, saying so under one id.
struct Passing;

#[async_trait]
impl Check for Passing {
    fn category(&self) -> Category {
        Category::Storage
    }

    async fn run(&self) -> Vec<Finding> {
        vec![Finding::in_category(
            Category::Storage,
            "storage.room",
            "room",
            Verdict::Pass { note: None },
        )]
    }
}

/// A deferred check counting how many times it was built.
fn counted(built: &Arc<AtomicUsize>) -> Deferred {
    let built = Arc::clone(built);
    Deferred::new(Category::Storage, Duration::from_secs(5), move || {
        let built = Arc::clone(&built);
        async move {
            built.fetch_add(1, Ordering::SeqCst);
            Box::new(Passing) as Box<dyn Check>
        }
    })
}

/// A run narrowed to another family never builds it, so its readings are never made.
#[tokio::test]
async fn a_run_that_does_not_reach_it_never_builds_it() {
    let built = Arc::new(AtomicUsize::new(0));
    let checks: Vec<Box<dyn Check>> = vec![Box::new(counted(&built))];

    let report = examine(&checks, &Narrowing::Category(Category::Vpn)).await;

    assert!(report.findings.is_empty());
    assert_eq!(built.load(Ordering::SeqCst), 0);
}

/// A run that reaches it builds it once and runs what it built, which is then what
/// a repair asks for its mender.
#[tokio::test]
async fn a_run_that_reaches_it_builds_it_once_and_runs_it() {
    let built = Arc::new(AtomicUsize::new(0));
    let deferred = counted(&built);
    assert!(
        deferred.mender().is_none(),
        "nothing built, nothing to mend"
    );
    assert_eq!(deferred.category(), Passing.category());

    let first = deferred.run().await;
    let second = deferred.run().await;

    assert_eq!(built.load(Ordering::SeqCst), 1);
    assert_eq!(first.len(), 1);
    assert_eq!(first, second);
    assert!(deferred.mender().is_none(), "what was built mends nothing");
}

/// Building is part of the run, so a reading that will not answer spends the check's
/// own budget and becomes its unverified finding rather than holding up the run.
#[tokio::test(start_paused = true)]
async fn a_reading_that_will_not_answer_spends_the_checks_own_budget() {
    let deferred = Deferred::new(Category::Storage, Duration::from_secs(5), || {
        std::future::pending::<Box<dyn Check>>()
    });
    let checks: Vec<Box<dyn Check>> = vec![Box::new(deferred)];

    let began = tokio::time::Instant::now();
    let report = examine(&checks, &Narrowing::Suite).await;

    assert_eq!(began.elapsed(), Duration::from_secs(5));
    assert!(
        matches!(
            report.findings.first().map(|finding| &finding.verdict),
            Some(Verdict::Unverified { .. })
        ),
        "{:?}",
        report.findings
    );
}
