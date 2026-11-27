//! What letting go of a name does to the work it names: nothing.
//!
//! A browser that stops watching work it did not mean to end forgets the name rather
//! than releasing it. The load-bearing property is the difference between the two:
//! releasing ends the work, forgetting leaves it running and takes the name away.

use std::sync::Arc;

use axum::http::StatusCode;
use lemonfiber_api::admission::Caller;
use lemonfiber_api::jobs::{Jobs, Standing, A_JOB, FORGET};
use lemonfiber_core::app::{Command, QualityAction};
use lemonfiber_core::ports::filesystem::Volume;

use crate::idle::ctx;
use crate::reading::{refused, Refusal};
use crate::what_became_of_a_job::{at, minted, settled, there, watching, LOOKS, WATCH};

/// What a name this run never issued is answered with.
fn absent() -> Option<String> {
    refused(Refusal::NoSuchJob, "No work in this run goes by that name.").map(|(_, body)| body)
}

#[tokio::test(start_paused = true)]
async fn forgetting_a_name_leaves_the_work_running() {
    let jobs = Jobs::default();
    let looked = there();
    let job = watching(&jobs, Arc::clone(&looked) as Arc<dyn Volume>).await;

    let forgotten = jobs.forget(job.as_str(), &Caller::Machine).await;
    assert_eq!(forgotten.map(|work| work.standing), Some(Standing::Running));

    let before = looked.looked();
    for _ in 0..LOOKS {
        tokio::time::advance(WATCH).await;
    }
    assert!(
        looked.looked() > before,
        "the guard went on looking after its name was let go"
    );
}

#[tokio::test]
async fn forgetting_a_name_over_the_route_answers_what_asking_would_have() {
    let jobs = Jobs::default();
    let job = watching(&jobs, there()).await;

    let (status, body) = at("POST", FORGET, job.as_str(), jobs, Caller::Machine).await;
    assert_eq!(status, StatusCode::ACCEPTED.as_u16(), "{body}");
    assert!(body.contains(r#""kind":"job""#), "{body}");
    assert!(body.contains(job.as_str()), "{body}");
    assert!(body.contains(r#""action":"watch""#), "{body}");
}

#[tokio::test]
async fn a_forgotten_name_is_one_this_run_never_issued_to_every_route() {
    let jobs = Jobs::default();
    let job = watching(&jobs, there()).await;
    let (status, _) = at("POST", FORGET, job.as_str(), jobs.clone(), Caller::Machine).await;
    assert_eq!(status, StatusCode::ACCEPTED.as_u16());

    for (method, route) in [("GET", A_JOB), ("DELETE", A_JOB), ("POST", FORGET)] {
        let (status, body) = at(method, route, job.as_str(), jobs.clone(), Caller::Machine).await;
        assert_eq!(status, StatusCode::NOT_FOUND.as_u16(), "{method} {route}");
        assert_eq!(Some(body), absent(), "{method} {route}");
    }
}

#[tokio::test]
async fn forgetting_finished_work_answers_with_what_it_came_to() {
    let jobs = Jobs::default();
    let job = minted();
    jobs.start(
        &job,
        "quality-reapply",
        Command::Quality(QualityAction::Show),
        Arc::new(ctx()),
        &Caller::Machine,
    )
    .await;
    let standing = settled(&jobs, job.as_str()).await;
    assert!(matches!(standing, Some(Standing::Done(_))), "{standing:?}");

    assert_eq!(
        jobs.forget(job.as_str(), &Caller::Machine)
            .await
            .map(|work| work.standing),
        standing
    );
    assert_eq!(jobs.about(job.as_str(), &Caller::Machine).await, None);
}

#[tokio::test]
async fn a_forgotten_guard_is_still_let_go_once_nothing_asks_about_it() {
    let jobs = Jobs::default();
    let job = watching(&jobs, there()).await;
    jobs.forget(job.as_str(), &Caller::Machine).await;

    assert_eq!(
        jobs.sweep().await,
        0,
        "the request that started it was asking"
    );
    assert_eq!(jobs.sweep().await, 1, "then nothing could ask about it");
}

#[tokio::test]
async fn forgetting_a_name_this_run_never_handed_out_stands_for_nothing() {
    assert_eq!(
        Jobs::default().forget("0badc0de", &Caller::Machine).await,
        None
    );
}
