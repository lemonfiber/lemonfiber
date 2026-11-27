//! What releasing a name does to the work it names: ends it.
//!
//! A browser has no interrupt to send, so the name it was answered with is the
//! handle. The load-bearing property is that releasing stops the work itself rather
//! than only the record of it, and that an outcome already reached is never
//! overwritten with an ending.

use std::sync::Arc;

use axum::http::StatusCode;
use lemonfiber_api::admission::Caller;
use lemonfiber_api::jobs::ended::Ending;
use lemonfiber_api::jobs::{Jobs, Standing};
use lemonfiber_core::app::{Command, QualityAction};
use lemonfiber_core::ports::filesystem::Volume;

use crate::idle::ctx;
use crate::reading::{refused, Refusal};
use crate::what_became_of_a_job::{asked, minted, over, settled, there, watching, LOOKS, WATCH};

/// What releasing one name over the job route answered, as its status and what it
/// said.
async fn released(jobs: Jobs, job: &str) -> (u16, String) {
    over("DELETE", jobs, job).await
}

#[tokio::test(start_paused = true)]
async fn releasing_a_name_stops_the_work_rather_than_only_the_record_of_it() {
    // The claim is that the polling stops, not that a register was written in.
    // Time is moved rather than waited on: a guard looks again every few seconds,
    // and a test that sat through two of them is a test somebody deletes.
    let jobs = Jobs::default();
    let looked = there();
    let job = watching(&jobs, Arc::clone(&looked) as Arc<dyn Volume>).await;

    // Long enough that a guard still running would have looked several times over.
    // Advanced a look at a time, because the next one is only scheduled once the
    // last has woken — one long jump would fire one timer, not all of them.
    let looks_ahead = || async {
        for _ in 0..LOOKS {
            tokio::time::advance(WATCH).await;
        }
    };
    looks_ahead().await;
    let by_now = looked.looked();
    assert!(by_now > 1, "the guard was guarding: {by_now}");

    let released = jobs.stop(job.as_str(), &Caller::Machine).await;
    assert_eq!(
        released.map(|work| work.standing),
        Some(Standing::Ended(Ending::Released))
    );

    looks_ahead().await;
    assert_eq!(
        looked.looked(),
        by_now,
        "it stopped looking, which is what a browser has no interrupt to say"
    );
}

#[tokio::test]
async fn releasing_a_name_this_run_never_handed_out_stands_for_nothing() {
    assert_eq!(
        Jobs::default().stop("0badc0de", &Caller::Machine).await,
        None
    );
}

#[tokio::test]
async fn releasing_work_that_already_finished_answers_with_what_it_came_to() {
    // Releasing a name twice, or releasing one whose work landed in the moment
    // before, must not overwrite an outcome with an ending.
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
        jobs.stop(job.as_str(), &Caller::Machine)
            .await
            .map(|work| work.standing),
        standing
    );
}

#[tokio::test]
async fn work_that_was_released_is_answered_as_ended_and_released() {
    let jobs = Jobs::default();
    let job = watching(&jobs, there()).await;
    let released = jobs.stop(job.as_str(), &Caller::Machine).await;
    assert_eq!(
        released.map(|work| work.standing),
        Some(Standing::Ended(Ending::Released))
    );

    let (status, body) = asked(jobs, job.as_str()).await;
    assert_eq!(status, StatusCode::OK.as_u16(), "{body}");
    assert!(body.contains(r#""kind":"ended""#), "{body}");
    assert!(body.contains(r#""action":"watch""#), "{body}");
    assert!(body.contains(r#""ended":"released""#), "{body}");
}

#[tokio::test]
async fn work_nobody_asked_about_is_answered_as_ended_and_let_go() {
    let jobs = Jobs::default();
    let job = watching(&jobs, there()).await;
    jobs.sweep().await;
    assert_eq!(jobs.sweep().await, 1);

    let (status, body) = asked(jobs, job.as_str()).await;
    assert_eq!(status, StatusCode::OK.as_u16(), "{body}");
    assert!(body.contains(r#""kind":"ended""#), "{body}");
    assert!(
        body.contains(&format!(r#""job":"{}""#, job.as_str())),
        "{body}"
    );
    assert!(body.contains(r#""ended":"let-go""#), "{body}");
}

#[tokio::test]
async fn releasing_a_name_over_the_route_says_where_the_work_now_stands() {
    // Releasing and asking are two questions with one answer, so a caller that
    // released a name need not ask again to find out what it released.
    let jobs = Jobs::default();
    let job = watching(&jobs, there()).await;
    let (status, body) = released(jobs.clone(), job.as_str()).await;
    assert_eq!(status, StatusCode::OK.as_u16(), "{body}");
    assert!(body.contains(r#""kind":"ended""#), "{body}");
    assert!(body.contains(r#""ended":"released""#), "{body}");
    assert_eq!(
        jobs.about(job.as_str(), &Caller::Machine)
            .await
            .map(|work| work.standing),
        Some(Standing::Ended(Ending::Released))
    );
}

#[tokio::test]
async fn releasing_a_name_the_route_never_handed_out_is_absent_there_too() {
    let (status, body) = released(Jobs::default(), "0badc0de").await;
    assert_eq!(status, StatusCode::NOT_FOUND.as_u16(), "{body}");
    assert_eq!(
        Some(body),
        refused(Refusal::NoSuchJob, "No work in this run goes by that name.").map(|(_, body)| body)
    );
}
