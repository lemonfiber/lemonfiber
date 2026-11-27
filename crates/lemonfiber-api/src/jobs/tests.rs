use std::time::Duration;

use tokio::sync::oneshot;

use super::{Jobs, Lease, Standing};
use crate::admission::Caller;
use crate::jobs::Job;

/// A name a test chose.
fn named(name: &str) -> Job {
    Job(name.to_owned())
}

/// How many pieces of work the register still holds.
async fn held(jobs: &Jobs) -> usize {
    jobs.0.lock().await.held.len()
}

#[tokio::test]
async fn forgotten_work_runs_to_its_end_and_is_then_held_for_nobody() {
    let jobs = Jobs::default();
    let (go, going) = oneshot::channel::<()>();
    let (finished, done) = oneshot::channel::<()>();
    jobs.begin(
        &named("0a0b0c0d"),
        "up",
        Lease::Held,
        &Caller::Machine,
        async move {
            drop(going.await);
            assert!(finished.send(()).is_ok());
            Standing::Done("{}".to_owned())
        },
    )
    .await;

    let forgotten = jobs.forget("0a0b0c0d", &Caller::Machine).await;
    assert_eq!(forgotten.map(|work| work.standing), Some(Standing::Running));
    assert_eq!(jobs.about("0a0b0c0d", &Caller::Machine).await, None);

    assert_eq!(go.send(()), Ok(()));
    assert_eq!(done.await, Ok(()), "the work ran to its end");
    for _ in 0..200 {
        if held(&jobs).await == 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert_eq!(held(&jobs).await, 0, "what it came to is kept for nobody");
}

#[tokio::test]
async fn a_forgotten_guard_let_go_by_the_sweep_is_held_for_nobody() {
    let jobs = Jobs::default();
    jobs.begin(
        &named("0e0f1011"),
        "watch",
        Lease::WhileAsked,
        &Caller::Machine,
        std::future::pending(),
    )
    .await;
    jobs.forget("0e0f1011", &Caller::Machine).await;

    jobs.sweep().await;
    assert_eq!(jobs.sweep().await, 1);
    assert_eq!(held(&jobs).await, 0);
}

#[tokio::test]
async fn forgetting_finished_work_lets_go_of_it_at_once() {
    let jobs = Jobs::default();
    jobs.begin(
        &named("12131415"),
        "up",
        Lease::Held,
        &Caller::Machine,
        std::future::ready(Standing::Done("{}".to_owned())),
    )
    .await;
    for _ in 0..200 {
        let standing = jobs
            .about("12131415", &Caller::Machine)
            .await
            .map(|work| work.standing);
        if standing != Some(Standing::Running) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }

    let forgotten = jobs.forget("12131415", &Caller::Machine).await;
    assert_eq!(
        forgotten.map(|work| work.standing),
        Some(Standing::Done("{}".to_owned()))
    );
    assert_eq!(held(&jobs).await, 0);
}
