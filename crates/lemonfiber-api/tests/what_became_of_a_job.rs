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

use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use axum::body::to_bytes;
use axum::http::{header, StatusCode};
use lemonfiber_api::events::live::Live;
use lemonfiber_api::guard::Token;
use lemonfiber_api::jobs::{accepted, leased, routes, Job, Jobs, Lease, Standing};
use lemonfiber_api::router::Serving;
use lemonfiber_core::app::{dispatch, Command, Ctx, Outcome, QualityAction};
use lemonfiber_core::config::Settings;
use lemonfiber_core::platform::Environment;
use lemonfiber_core::ports::filesystem::{Presence, Volume};
use lemonfiber_fixtures::ports::{Chance, Idle, Stopped};

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
        live: Arc::new(Live::opening(Stopped::at(0).as_ref())),
    })
}

/// How often a guard looks again, and how many looks a test moves past.
///
/// The interval is the command's own; a test that wrote down its own number would
/// pass on an interval this product does not use.
const WATCH: Duration = lemonfiber_core::app::WATCH;

/// Looks a test moves the clock past, enough that a guard still guarding has
/// plainly looked more than once.
const LOOKS: usize = 4;

/// How often a swept register is looked at, for the test that drives the beat.
const BEAT: Duration = Duration::from_millis(5);

/// A world guarding a directory that is really there, so a guard holds rather
/// than ending before a test can do anything to it.
fn guarding(volume: Arc<dyn Volume>) -> Ctx {
    let dir = std::env::temp_dir().join(format!("lemonfiber-job-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    Ctx::new(
        Arc::new(Idle),
        Arc::new(lemonfiber_core::adapters::Daemon::local()),
        Arc::new(lemonfiber_core::adapters::System),
        Arc::new(lemonfiber_core::adapters::Disk),
        lemonfiber_core::stack::Source::External(std::path::Path::new("/lemonfiber/no/such/stack")),
        Settings {
            data_root: Some(dir),
            ..Settings::default()
        },
        Environment::MacOs,
    )
    .with_random(Arc::new(Chance::cycling()))
    .with_volume(volume)
}

/// A volume that is always there, and counts how often it was asked.
struct Counting(Arc<AtomicUsize>);

#[async_trait]
impl Volume for Counting {
    async fn presence(&self, _path: &Path) -> Presence {
        self.0.fetch_add(1, Ordering::SeqCst);
        Presence::On(9)
    }
}

/// A guard started under a name, over a location that will not go away.
async fn watching(jobs: &Jobs, volume: Arc<dyn Volume>) -> Job {
    let job = minted();
    jobs.start(
        &job,
        "watch",
        Command::Watch {
            forms: vec!["library".to_owned()],
        },
        Arc::new(guarding(volume)),
    )
    .await;
    job
}

/// What asking about one name answered, as its status and what it said.
async fn asked(jobs: Jobs, job: &str) -> (u16, String) {
    over("GET", jobs, job).await
}

/// The same, for a request that releases the name instead of asking about it.
async fn released(jobs: Jobs, job: &str) -> (u16, String) {
    over("DELETE", jobs, job).await
}

/// What one request to the job route answered, as its status and what it said.
async fn over(method: &str, jobs: Jobs, job: &str) -> (u16, String) {
    let request = axum::http::Request::builder()
        .method(method)
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
    let job = Job::mint(&Chance::exactly(Some(vec![
        0x00, 0x0f, 0xa5, 0xff, 0x00, 0x0f, 0xa5, 0xff,
    ])));
    assert_eq!(
        job.map(|job| job.as_str().to_owned()),
        Some("000fa5ff000fa5ff".to_owned())
    );
}

#[test]
fn a_machine_that_answers_short_has_not_named_the_work() {
    // A name is the whole of what a caller redeems work by, so it is a capability
    // and its width is what makes guessing hopeless — the same reasoning the token
    // is minted under. A short answer is invisible in the result: half a name is a
    // name, and looks like one right up until somebody guesses it. This asked for
    // eight bytes and was given four.
    assert!(Job::mint(&Chance::exactly(Some(vec![0x00, 0x0f, 0xa5, 0xff]))).is_none());
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
        matches!(&standing, Some(Standing::Failed(said, _))
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
        matches!(standing, Some(Standing::Failed(..))),
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

// ── Ending work by the name it was answered with ─────────────────────────────

#[tokio::test(start_paused = true)]
async fn releasing_a_name_stops_the_work_rather_than_only_the_record_of_it() {
    // The claim is that the polling stops, not that a register was written in.
    // Time is moved rather than waited on: a guard looks again every few seconds,
    // and a test that sat through two of them is a test somebody deletes.
    let jobs = Jobs::default();
    let looked = Arc::new(AtomicUsize::new(0));
    let job = watching(&jobs, Arc::new(Counting(Arc::clone(&looked)))).await;

    // Long enough that a guard still running would have looked several times over.
    // Advanced a look at a time, because the next one is only scheduled once the
    // last has woken — one long jump would fire one timer, not all of them.
    let looks_ahead = || async {
        for _ in 0..LOOKS {
            tokio::time::advance(WATCH).await;
        }
    };
    looks_ahead().await;
    let by_now = looked.load(Ordering::SeqCst);
    assert!(by_now > 1, "the guard was guarding: {by_now}");

    let released = jobs.stop(job.as_str()).await;
    assert_eq!(released.map(|work| work.standing), Some(Standing::Ended));

    looks_ahead().await;
    assert_eq!(
        looked.load(Ordering::SeqCst),
        by_now,
        "it stopped looking, which is what a browser has no interrupt to say"
    );
}

#[tokio::test]
async fn releasing_a_name_this_run_never_handed_out_stands_for_nothing() {
    assert_eq!(Jobs::default().stop("0badc0de").await, None);
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
    )
    .await;
    let standing = settled(&jobs, job.as_str()).await;
    assert!(matches!(standing, Some(Standing::Done(_))), "{standing:?}");

    assert_eq!(
        jobs.stop(job.as_str()).await.map(|work| work.standing),
        standing
    );
}

#[tokio::test]
async fn work_that_was_ended_is_answered_with_the_name_rather_than_an_outcome() {
    // Under the status finished work answers with, and the kind the start answered
    // with: the work is over, and there is no outcome because it did not reach one.
    let jobs = Jobs::default();
    let job = watching(&jobs, Arc::new(Counting(Arc::new(AtomicUsize::new(0))))).await;
    let released = jobs.stop(job.as_str()).await;
    assert_eq!(released.map(|work| work.standing), Some(Standing::Ended));

    let (status, body) = asked(jobs, job.as_str()).await;
    assert_eq!(status, StatusCode::OK.as_u16(), "{body}");
    assert!(body.contains(r#""kind":"job""#), "{body}");
    assert!(body.contains(r#""action":"watch""#), "{body}");
}

#[tokio::test]
async fn releasing_a_name_over_the_route_says_where_the_work_now_stands() {
    // Releasing and asking are two questions with one answer, so a caller that
    // released a name need not ask again to find out what it released.
    let jobs = Jobs::default();
    let job = watching(&jobs, Arc::new(Counting(Arc::new(AtomicUsize::new(0))))).await;
    let (status, body) = released(jobs.clone(), job.as_str()).await;
    assert_eq!(status, StatusCode::OK.as_u16(), "{body}");
    assert!(body.contains(r#""kind":"job""#), "{body}");
    assert_eq!(
        jobs.about(job.as_str()).await.map(|work| work.standing),
        Some(Standing::Ended)
    );
}

#[tokio::test]
async fn releasing_a_name_the_route_never_handed_out_is_absent_there_too() {
    let (status, body) = released(Jobs::default(), "0badc0de").await;
    assert_eq!(status, StatusCode::NOT_FOUND.as_u16(), "{body}");
    assert_eq!(body, "No work in this run goes by that name.");
}

// ── The lease on work with no ending of its own ──────────────────────────────

#[test]
fn a_guard_is_the_one_command_held_only_while_somebody_asks() {
    // Everything else finishes — a fetch takes an hour at worst — and letting one
    // go because nobody happened to ask about it would end work that was going to
    // succeed.
    assert_eq!(
        leased(&Command::Watch {
            forms: vec!["library".to_owned()]
        }),
        Lease::WhileAsked
    );
    for command in [
        Command::Up { forms: Vec::new() },
        Command::Pull {
            forms: vec!["library".to_owned()],
        },
        Command::Walkthrough { item: None },
        Command::Seed,
    ] {
        assert_eq!(leased(&command), Lease::Held, "{command:?}");
    }
}

#[tokio::test]
async fn a_guard_nobody_asks_about_is_let_go_on_the_second_look() {
    // One sweep that finds it untouched is not enough: a name is handed out
    // between sweeps, and ending it on the first look would be a race with the
    // sweep's timing rather than a bound on how long nobody was interested.
    let jobs = Jobs::default();
    let job = watching(&jobs, Arc::new(Counting(Arc::new(AtomicUsize::new(0))))).await;

    assert_eq!(jobs.sweep().await, 0, "it survives the first look");
    assert_eq!(
        jobs.about(job.as_str()).await.map(|work| work.standing),
        Some(Standing::Running)
    );
    // Asking is what renews it, so the ask above buys it another look.
    assert_eq!(jobs.sweep().await, 0, "and the ask renewed it");
    assert_eq!(jobs.sweep().await, 1, "then nothing was asking");
    assert_eq!(
        jobs.about(job.as_str()).await.map(|work| work.standing),
        Some(Standing::Ended)
    );
}

#[tokio::test]
async fn work_that_has_already_finished_is_not_let_go_by_a_sweep() {
    let jobs = Jobs::default();
    let job = minted();
    jobs.start(&job, "up", Command::Forms, Arc::new(ctx()))
        .await;
    let standing = settled(&jobs, job.as_str()).await;
    assert!(
        matches!(standing, Some(Standing::Failed(..))),
        "{standing:?}"
    );

    assert_eq!(jobs.sweep().await, 0);
    assert_eq!(jobs.sweep().await, 0);
    assert_eq!(
        jobs.about(job.as_str()).await.map(|work| work.standing),
        standing,
        "what it came to is what it stands for"
    );
}

#[tokio::test]
async fn sweeping_on_the_beat_lets_go_of_what_a_sweep_asked_for_would() {
    // Waited out rather than polled: asking what became of it is what renews the
    // lease, so a test that watched for the ending by asking would renew the very
    // thing it was waiting to see let go.
    let jobs = Jobs::default();
    let job = watching(&jobs, Arc::new(Counting(Arc::new(AtomicUsize::new(0))))).await;
    let beating = tokio::spawn(jobs.clone().sweeping(BEAT));

    tokio::time::sleep(BEAT * 20).await;
    beating.abort();
    assert_eq!(
        jobs.about(job.as_str()).await.map(|work| work.standing),
        Some(Standing::Ended)
    );
}

/// Where a refusal lies decides the status, whichever door it arrives through.
///
/// The same mistake reaches this surface two ways: named to a read, which answers
/// it directly, and named to an action, which hands back a name and is asked later
/// what became of it. A form nothing declares is the caller's mistake on both, and
/// answering the second `500` sends a browser on retrying what cannot succeed.
///
/// Asserted twice: on the standing, which is the only place a problem and a status
/// are both in hand, and through the door, which is what a caller actually meets. The
/// first alone passes on a door that ignores what it carries — which is the shape of
/// the defect, so a test that stopped there would be testing the wrong half.
#[test]
fn where_a_refusal_lies_decides_the_status_a_job_is_answered_with() {
    use lemonfiber_core::error::{Amiss, Code, Problem, Remedy, Severity};

    let refusing = |amiss| {
        let problem = Problem::new(
            Code::new("TEST-1"),
            Severity::Error,
            "the form was not declared",
            "nothing by that name is in this stack",
            Remedy::new("name a form the stack declares"),
        )
        .lies_in(amiss);
        match Standing::failed(&problem) {
            Standing::Failed(_, status) => Some(status.as_u16()),
            _ => None,
        }
    };

    assert_eq!(
        refusing(Amiss::Naming),
        Some(StatusCode::NOT_FOUND.as_u16()),
        "a thing there is no such thing as"
    );
    assert_eq!(
        refusing(Amiss::Asking),
        Some(StatusCode::BAD_REQUEST.as_u16()),
        "a request that could not be answered as it stands"
    );
    assert_eq!(
        refusing(Amiss::Answering),
        Some(StatusCode::INTERNAL_SERVER_ERROR.as_u16()),
        "and this product's own failure, which is what every one of them said before"
    );
}

/// And the door answers with the status the standing carries, not a constant.
///
/// A word the glossary has no entry for is the caller naming a thing there is no
/// such thing as, and the glossary is a table compiled into the binary — so this
/// reaches a `Naming` refusal without a stack to read. Asked for as a job, it is
/// answered `404`, the same as asking `/api/explain` for that word directly.
#[tokio::test]
async fn a_name_nothing_answers_to_is_absent_through_the_job_door_too() {
    let jobs = Jobs::default();
    let job = minted();
    jobs.start(
        &job,
        "explain",
        Command::Explain {
            word: "nothing-explains-this".to_owned(),
        },
        Arc::new(ctx()),
    )
    .await;

    let standing = settled(&jobs, job.as_str()).await;
    assert!(
        matches!(standing, Some(Standing::Failed(..))),
        "the word has no entry: {standing:?}"
    );

    let (status, body) = asked(jobs, job.as_str()).await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND.as_u16(),
        "a word with no entry is absent, not this product failing: {body}"
    );
}
