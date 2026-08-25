//! What a name for work stands for, and what became of the work it names.
//!
//! The load-bearing property is that the name is redeemable. An action that runs
//! for minutes is answered with a name and nothing else, so a name that could not
//! afterwards be turned back into an outcome would make that reply an
//! acknowledgement rather than an answer — and a browser that started a repair
//! would have no way of learning it had failed.
//!
//! Driven from outside the crate, because what a caller can reach is the thing
//! worth holding still.

use std::sync::Arc;
use std::time::Duration;

use axum::body::to_bytes;
use axum::http::{header, StatusCode};
use lemonfiber_api::guard::Token;
use lemonfiber_api::jobs::{accepted, routes, Job, Jobs, Standing};
use lemonfiber_api::router::Serving;
use lemonfiber_core::app::{dispatch, Command, Ctx, Outcome, QualityAction};
use lemonfiber_core::config::Settings;
use lemonfiber_core::platform::Environment;
use lemonfiber_fixtures::ports::{Chance, Idle};

/// A context that needs neither a stack on disk nor a daemon to answer.
fn ctx() -> Ctx {
    Ctx::new(
        Arc::new(Idle),
        Arc::new(lemonfiber_core::adapters::Daemon::local()),
        Arc::new(lemonfiber_core::adapters::System),
        Arc::new(lemonfiber_core::adapters::Disk),
        lemonfiber_core::stack::Source::External(std::path::Path::new("/lemonfiber/no/such/stack")),
        Settings::default(),
        Environment::MacOs,
    )
    .with_random(Arc::new(Chance::cycling()))
}

/// A name this run can hand out.
fn minted() -> Job {
    let Some(job) = Job::mint(&Chance::cycling()) else {
        unreachable!("cycling letters always supply bytes");
    };
    job
}

/// The route as a run builds it, over the work a test started.
fn routed(jobs: Jobs) -> axum::Router {
    let Some(token) = Token::mint(&Chance::cycling()) else {
        unreachable!("cycling letters always supply bytes");
    };
    routes().with_state(Serving {
        ctx: Arc::new(ctx()),
        token: Arc::new(token),
        bound: ([127, 0, 0, 1], 8471).into(),
        jobs,
    })
}

/// What asking about one name answered, as its status and what it said.
async fn asked(jobs: Jobs, job: &str) -> (u16, String) {
    let request = axum::http::Request::builder()
        .uri(format!("/api/jobs/{job}"))
        .body(axum::body::Body::empty());
    let Ok(request) = request else {
        unreachable!("a request built from values that are already headers cannot fail");
    };
    let served = tower::ServiceExt::oneshot(routed(jobs), request).await.ok();
    let Some(response) = served else {
        unreachable!("the router is infallible; its handlers answer rather than fail");
    };
    let status = response.status().as_u16();
    let read = to_bytes(response.into_body(), usize::MAX).await;
    let bytes = read.map(|bytes| bytes.to_vec()).unwrap_or_default();
    (status, String::from_utf8(bytes).unwrap_or_default())
}

/// Where a job got to, once it has had the chance to get anywhere.
async fn settled(jobs: &Jobs, job: &str) -> Option<Standing> {
    for _ in 0..200 {
        match jobs.about(job).await.map(|work| work.standing) {
            Some(Standing::Running) | None => {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            settled => return settled,
        }
    }
    jobs.about(job).await.map(|work| work.standing)
}

/// The envelope the command line would have rendered for this command.
async fn as_the_command_renders_it(command: Command) -> Option<String> {
    dispatch(command, &ctx())
        .await
        .ok()
        .map(Outcome::envelope)
        .and_then(|envelope| envelope.to_json())
}

// ── The name itself ───────────────────────────────────────────────────────────

#[test]
fn a_job_is_the_bytes_the_machine_gave_written_as_one_word() {
    let job = Job::mint(&Chance::exactly(Some(vec![0x00, 0x0f, 0xa5, 0xff])));
    assert_eq!(
        job.map(|job| job.as_str().to_owned()),
        Some("000fa5ff".to_owned())
    );
}

#[test]
fn there_is_no_job_when_the_machine_will_not_say() {
    assert!(Job::mint(&Chance::exactly(None)).is_none());
}

#[tokio::test]
async fn an_accepted_job_is_named_so_what_became_of_it_can_be_asked() {
    let job = minted();
    let response = accepted(&job, "up");

    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert_eq!(
        response
            .headers()
            .get(header::CONTENT_TYPE)
            .map(AsRef::as_ref),
        Some(b"application/json".as_slice())
    );

    let body = to_bytes(response.into_body(), usize::MAX).await;
    let said = String::from_utf8(body.map(|bytes| bytes.to_vec()).unwrap_or_default());
    let said = said.unwrap_or_default();
    assert!(said.contains(r#""kind":"job""#), "{said}");
    assert!(
        said.contains(r#""api_version":1"#),
        "the same envelope: {said}"
    );
    assert!(said.contains(job.as_str()), "the name to ask by: {said}");
    assert!(said.contains(r#""action":"up""#), "and what it was: {said}");
}

// ── Work that a closing connection cannot reach ───────────────────────────────

#[tokio::test]
async fn work_started_under_a_name_finishes_without_anything_holding_it() {
    // Nothing awaits the work, and nothing is holding the request that asked for
    // it — which is the point. A browser tab closed here takes nothing with it.
    let jobs = Jobs::default();
    let job = minted();
    jobs.start(
        &job,
        "quality-reapply",
        Command::Quality(QualityAction::Show),
        Arc::new(ctx()),
    )
    .await;

    let standing = settled(&jobs, job.as_str()).await;
    assert!(
        matches!(standing, Some(Standing::Done(_))),
        "it ran to its end on its own: {standing:?}"
    );
}

#[tokio::test]
async fn work_that_could_not_be_done_says_so_under_its_own_name() {
    let jobs = Jobs::default();
    let job = minted();
    // A stack that is not there: the command fails, and the failure is recorded
    // against the job rather than lost with the request that started it.
    jobs.start(&job, "up", Command::Forms, Arc::new(ctx()))
        .await;

    let standing = settled(&jobs, job.as_str()).await;
    assert!(
        matches!(&standing, Some(Standing::Failed(said))
            if said.contains(r#""kind":"error""#)),
        "{standing:?}"
    );
}

#[tokio::test]
async fn the_action_a_name_was_started_under_is_kept_beside_it() {
    // A caller holding several names is answered with what each one was for,
    // rather than being asked to remember which name it gave which request.
    let jobs = Jobs::default();
    let job = minted();
    jobs.start(&job, "pull", Command::Forms, Arc::new(ctx()))
        .await;

    let asked_for = jobs.about(job.as_str()).await.map(|work| work.action);
    assert_eq!(asked_for, Some("pull".to_owned()));
}

// ── Asking what became of it ─────────────────────────────────────────────────

#[tokio::test]
async fn work_still_going_is_answered_with_the_name_it_was_started_under() {
    // A task spawned on this runtime cannot run while the task that spawned it is
    // still running, and nothing between the start and the read below yields — so
    // the work is still going when it is asked about, every time.
    let jobs = Jobs::default();
    let job = minted();
    jobs.start(&job, "up", Command::Forms, Arc::new(ctx()))
        .await;

    let (status, body) = asked(jobs, job.as_str()).await;
    assert_eq!(status, StatusCode::ACCEPTED.as_u16(), "{body}");
    assert!(body.contains(r#""kind":"job""#), "{body}");
    assert!(body.contains(job.as_str()), "{body}");
    assert!(body.contains(r#""action":"up""#), "{body}");
}

#[tokio::test]
async fn work_that_finished_is_answered_with_the_envelope_the_command_renders() {
    // The whole of the contract in one assertion: the bytes a caller redeeming a
    // name reads are the bytes a script would have piped from the same command.
    let expected = as_the_command_renders_it(Command::Quality(QualityAction::Show)).await;
    assert!(expected.is_some(), "the command answered");

    let jobs = Jobs::default();
    let job = minted();
    jobs.start(
        &job,
        "quality-reapply",
        Command::Quality(QualityAction::Show),
        Arc::new(ctx()),
    )
    .await;
    let standing = settled(&jobs, job.as_str()).await;
    assert!(
        matches!(standing, Some(Standing::Done(_))),
        "it finished: {standing:?}"
    );

    let seen = asked(jobs, job.as_str()).await;
    assert_eq!(
        Some(seen),
        expected.map(|body| (StatusCode::OK.as_u16(), body))
    );
}

#[tokio::test]
async fn work_that_stopped_is_answered_with_the_failure_it_stopped_on() {
    let jobs = Jobs::default();
    let job = minted();
    jobs.start(&job, "up", Command::Forms, Arc::new(ctx()))
        .await;
    let standing = settled(&jobs, job.as_str()).await;
    assert!(
        matches!(standing, Some(Standing::Failed(_))),
        "it stopped: {standing:?}"
    );

    let (status, body) = asked(jobs, job.as_str()).await;
    // Not 200 with a failure inside it: a caller's own idea of a successful call
    // should mean what it says, and which failure it was is the envelope's code.
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR.as_u16(), "{body}");
    assert!(
        body.starts_with(r#"{"api_version":1,"kind":"error","data":{"code":"#),
        "{body}"
    );
}

#[tokio::test]
async fn a_name_this_run_never_handed_out_stands_for_nothing() {
    assert_eq!(Jobs::default().about("0badc0de").await, None);
}

#[tokio::test]
async fn a_name_this_run_never_handed_out_is_absent_rather_than_unfinished() {
    // The refusal this endpoint exists to make. Answering "still going" for work
    // nothing is doing would leave a caller waiting on an outcome never coming,
    // which is worse than the four hundred and four it would be hiding.
    let (status, body) = asked(Jobs::default(), "0badc0de").await;
    assert_eq!(status, StatusCode::NOT_FOUND.as_u16(), "{body}");
    assert_eq!(body, "No work in this run goes by that name.");
}

#[tokio::test]
async fn a_name_that_was_handed_out_is_not_repeated_back_to_a_caller_that_guessed() {
    // What was asked for is not quoted into the refusal: a name is a secret this
    // run minted, and a message repeating one carries it wherever the message goes.
    let (_, body) = asked(Jobs::default(), "deadbeef").await;
    assert!(!body.contains("deadbeef"), "{body}");
}
