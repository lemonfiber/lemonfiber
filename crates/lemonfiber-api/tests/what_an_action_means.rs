//! What a write request asks for, and what it is refused for.
//!
//! The load-bearing property is not that any one action works. It is that the set
//! of them is the set the command line already has: every action reaches one of
//! the core's own commands, and a name that reaches none is refused rather than
//! invented. A surface that could grow an action of its own is a surface that has
//! started implementing behaviour, and this is where that is stopped.
//!
//! Driven from outside the crate, because what a caller can reach is the thing
//! worth holding still.

use std::sync::Arc;
use std::time::Duration;

use axum::body::to_bytes;
use axum::http::{header, StatusCode};
use lemonfiber_api::actions;
use lemonfiber_api::actions::{
    accepted, answering, declined, named, Answering, Arguments, Job, Jobs, Refused, OFFERED,
};
use lemonfiber_api::guard::Token;
use lemonfiber_api::router::Serving;
use lemonfiber_core::app::{Command, Ctx, QualityAction};
use lemonfiber_core::config::Settings;
use lemonfiber_core::platform::Environment;
use lemonfiber_core::quality::Preset;
use lemonfiber_fixtures::ports::{Chance, Idle};

/// Nothing named, which is what most actions are asked with.
fn nothing() -> Arguments {
    Arguments::default()
}

/// One form named, which is what most of the rest are asked with.
fn naming(form: &str) -> Arguments {
    Arguments {
        forms: vec![form.to_owned()],
        ..Arguments::default()
    }
}

/// What an action came to, or nothing where it was refused.
fn command(action: &str, given: Arguments) -> Option<Command> {
    named(action, given).ok()
}

/// Why an action was refused, or nothing where it was not.
fn refusal(action: &str, given: Arguments) -> Option<Refused> {
    named(action, given).err()
}

// ── Every action is one of the command line's own ─────────────────────────────

#[test]
fn every_action_this_surface_offers_reaches_a_command() {
    // The whole guarantee, in one sweep: a name on the list that reached no
    // command would be an action only this surface has, and a command is what
    // the command line produces too.
    let unreachable: Vec<&str> = OFFERED
        .iter()
        .copied()
        .filter(|action| {
            // Named with everything any of them could need, so a refusal here is
            // about the name rather than about a missing argument.
            let given = Arguments {
                forms: vec!["tv".to_owned()],
                key: Some("DATA_ROOT".to_owned()),
                value: Some("/srv".to_owned()),
                preset: Some("balanced".to_owned()),
                ..Arguments::default()
            };
            named(action, given).is_err()
        })
        .collect();
    assert!(
        unreachable.is_empty(),
        "these are offered and reach nothing: {unreachable:?}"
    );
}

#[test]
fn a_name_this_surface_does_not_offer_is_refused_rather_than_invented() {
    assert_eq!(
        refusal("reticulate", nothing()),
        Some(Refused::Unknown {
            name: "reticulate".to_owned()
        })
    );
}

#[test]
fn a_read_is_not_an_action() {
    // Asking what the stack is doing is a read with an endpoint of its own, and
    // a write surface that also answered it would be two ways to ask one thing.
    for read in ["status", "ps", "trace", "household", "stuck", "config-get"] {
        assert!(
            matches!(refusal(read, nothing()), Some(Refused::Unknown { .. })),
            "{read} is a read"
        );
    }
}

#[test]
fn a_refusal_says_which_of_the_three_it_was() {
    let said = [
        Refused::Unknown {
            name: "reticulate".to_owned(),
        },
        Refused::Missing {
            action: "pull".to_owned(),
            argument: "forms".to_owned(),
        },
        Refused::Unrecognised {
            argument: "preset".to_owned(),
            offered: "try balanced".to_owned(),
        },
    ];
    for refusal in &said {
        assert!(!refusal.said().is_empty(), "{refusal:?}");
    }
    assert_eq!(said.len(), 3, "and each says something different");
    assert_ne!(said[0].said(), said[1].said());
    assert_ne!(said[1].said(), said[2].said());
}

#[test]
fn a_name_that_is_not_an_action_is_absent_and_a_bad_argument_is_a_mistake() {
    // Two different statuses because they are two different faults: one is a
    // path nobody wrote, the other a request that has one.
    assert_eq!(
        Refused::Unknown {
            name: "reticulate".to_owned()
        }
        .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        Refused::Missing {
            action: "pull".to_owned(),
            argument: "forms".to_owned()
        }
        .status(),
        StatusCode::BAD_REQUEST
    );
}

// ── Arguments mirror the flags the command takes ──────────────────────────────

#[test]
fn starting_and_stopping_take_the_forms_they_are_given() {
    assert_eq!(
        command("up", naming("tv")),
        Some(Command::Up {
            forms: vec!["tv".to_owned()]
        })
    );
    assert_eq!(
        command("up", nothing()),
        Some(Command::Up { forms: vec![] })
    );
    assert_eq!(
        command("down", naming("tv")),
        Some(Command::Down {
            forms: vec!["tv".to_owned()]
        })
    );
}

#[test]
fn stopping_named_services_is_a_different_request_from_tearing_a_form_down() {
    let given = Arguments {
        forms: vec!["tv".to_owned()],
        services: vec!["sonarr".to_owned()],
        ..Arguments::default()
    };
    assert_eq!(
        command("down", given),
        Some(Command::Halt {
            forms: vec!["tv".to_owned()],
            services: vec!["sonarr".to_owned()]
        })
    );
}

#[test]
fn the_three_actions_that_must_be_told_what_to_act_on_say_so() {
    for action in ["switch", "restart", "pull"] {
        assert_eq!(
            refusal(action, nothing()),
            Some(Refused::Missing {
                action: action.to_owned(),
                argument: "forms".to_owned()
            }),
            "{action}"
        );
    }
}

#[test]
fn changing_a_setting_needs_both_halves_of_it() {
    let key_only = Arguments {
        key: Some("DATA_ROOT".to_owned()),
        ..Arguments::default()
    };
    let value_only = Arguments {
        value: Some("/srv".to_owned()),
        ..Arguments::default()
    };
    assert_eq!(
        refusal("config-set", value_only),
        Some(Refused::Missing {
            action: "config-set".to_owned(),
            argument: "key".to_owned()
        })
    );
    assert_eq!(
        refusal("config-set", key_only),
        Some(Refused::Missing {
            action: "config-set".to_owned(),
            argument: "value".to_owned()
        })
    );
}

#[test]
fn a_quality_choice_reaches_the_preset_it_names() {
    let given = Arguments {
        preset: Some("balanced".to_owned()),
        ..Arguments::default()
    };
    assert_eq!(
        command("quality-set", given),
        Some(Command::Quality(QualityAction::Set {
            preset: Preset::Balanced,
            media_type: None,
            confirm: false
        }))
    );
}

#[test]
fn choosing_for_music_chooses_a_format_rather_than_a_resolution() {
    // Music has no resolution, so the same request means a different command —
    // the fork the command line takes, taken the same way here.
    let given = Arguments {
        preset: Some("lossless".to_owned()),
        media_type: Some("music".to_owned()),
        ..Arguments::default()
    };
    assert!(matches!(
        command("quality-set", given),
        Some(Command::QualityMusic { .. })
    ));
}

#[test]
fn a_quality_choice_with_no_preset_at_all_says_which_argument_it_wanted() {
    assert_eq!(
        refusal("quality-set", nothing()),
        Some(Refused::Missing {
            action: "quality-set".to_owned(),
            argument: "preset".to_owned()
        })
    );
}

#[test]
fn a_preset_that_names_nothing_is_refused_with_what_it_could_have_said() {
    let bad_preset = Arguments {
        preset: Some("cinematic".to_owned()),
        ..Arguments::default()
    };
    let bad_music = Arguments {
        preset: Some("cinematic".to_owned()),
        media_type: Some("music".to_owned()),
        ..Arguments::default()
    };
    let bad_type = Arguments {
        preset: Some("balanced".to_owned()),
        media_type: Some("podcasts".to_owned()),
        ..Arguments::default()
    };
    for (given, argument) in [
        (bad_preset, "preset"),
        (bad_music, "preset"),
        (bad_type, "media_type"),
    ] {
        let refused = refusal("quality-set", given);
        assert!(
            matches!(&refused, Some(Refused::Unrecognised { argument: named, offered })
                if named == argument && !offered.is_empty()),
            "{argument}: {refused:?}"
        );
    }
}

#[test]
fn an_argument_no_action_takes_is_refused_rather_than_ignored() {
    // A caller who spelled `service` where `services` was meant has been told,
    // instead of watching a whole form stop.
    let mistyped = serde_json::from_str::<Arguments>(r#"{"service":["sonarr"]}"#);
    assert!(mistyped.is_err());
    assert!(serde_json::from_str::<Arguments>("{}").is_ok());
}

// ── Which answers wait, and which are named and left to run ───────────────────

#[test]
fn an_action_reaching_the_engine_is_answered_with_a_name_for_the_work() {
    for action in ["up", "down", "switch", "restart", "pull", "seed", "adopt"] {
        let Some(command) = command(action, naming("tv")) else {
            unreachable!("every offered action reaches a command");
        };
        assert_eq!(answering(&command), Answering::Later, "{action}");
    }
}

#[test]
fn an_action_confined_to_our_own_files_is_answered_with_its_outcome() {
    let setting = Arguments {
        key: Some("DATA_ROOT".to_owned()),
        value: Some("/srv".to_owned()),
        ..Arguments::default()
    };
    let Some(config) = command("config-set", setting) else {
        unreachable!("a setting with both halves reaches a command");
    };
    let Some(reapply) = command("quality-reapply", nothing()) else {
        unreachable!("reapply takes nothing and reaches a command");
    };
    assert_eq!(answering(&config), Answering::Now);
    assert_eq!(answering(&reapply), Answering::Now);
}

// ── A name for work that outlives the request ─────────────────────────────────

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
async fn an_accepted_job_is_named_so_the_stream_can_be_followed_for_it() {
    let Some(job) = Job::mint(&Chance::cycling()) else {
        unreachable!("cycling letters always supply bytes");
    };
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
    assert!(said.contains(job.as_str()), "the name to follow: {said}");
    assert!(said.contains(r#""action":"up""#), "and what it was: {said}");
}

#[tokio::test]
async fn a_declined_action_says_why_rather_than_answering_with_a_status_alone() {
    let refusal = Refused::Unknown {
        name: "reticulate".to_owned(),
    };
    let response = declined(&refusal);
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let body = to_bytes(response.into_body(), usize::MAX).await;
    assert_eq!(body.ok().as_deref(), Some(refusal.said().as_bytes()));
}

// ── Work that a closing connection cannot reach ───────────────────────────────

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

/// Where a job got to, once it has had the chance to get anywhere.
async fn settled(jobs: &Jobs, job: &str) -> Option<lemonfiber_api::actions::Standing> {
    for _ in 0..200 {
        match jobs.standing(job).await {
            Some(lemonfiber_api::actions::Standing::Running) | None => {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            settled => return settled,
        }
    }
    jobs.standing(job).await
}

#[tokio::test]
async fn work_started_under_a_name_finishes_without_anything_holding_it() {
    // Nothing awaits the work, and nothing is holding the request that asked for
    // it — which is the point. A browser tab closed here takes nothing with it.
    let jobs = Jobs::default();
    let Some(job) = Job::mint(&Chance::cycling()) else {
        unreachable!("cycling letters always supply bytes");
    };
    jobs.start(&job, Command::Quality(QualityAction::Show), Arc::new(ctx()))
        .await;

    let standing = settled(&jobs, job.as_str()).await;
    assert!(
        matches!(standing, Some(lemonfiber_api::actions::Standing::Done(_))),
        "it ran to its end on its own: {standing:?}"
    );
}

#[tokio::test]
async fn work_that_could_not_be_done_says_so_under_its_own_name() {
    let jobs = Jobs::default();
    let Some(job) = Job::mint(&Chance::cycling()) else {
        unreachable!("cycling letters always supply bytes");
    };
    // A stack that is not there: the command fails, and the failure is recorded
    // against the job rather than lost with the request that started it.
    jobs.start(&job, Command::Forms, Arc::new(ctx())).await;

    let standing = settled(&jobs, job.as_str()).await;
    assert!(
        matches!(&standing, Some(lemonfiber_api::actions::Standing::Failed(said))
            if said.contains(r#""kind":"error""#)),
        "{standing:?}"
    );
}

#[tokio::test]
async fn a_name_this_run_never_handed_out_stands_for_nothing() {
    assert_eq!(Jobs::default().standing("0badc0de").await, None);
}

// ── The route itself, driven without a socket ─────────────────────────────────

/// The action routes as a run builds them, over a context a test chose.
///
/// Stated here rather than assembled with the rest of the surface: what an
/// action *means* is this file's business, and that a request reaches it only
/// with a token is the router's, proven where the router is.
fn routed(random: Chance) -> axum::Router {
    let Some(token) = Token::mint(&Chance::cycling()) else {
        unreachable!("cycling letters always supply bytes");
    };
    actions::routes().with_state(Serving {
        ctx: Arc::new(ctx().with_random(Arc::new(random))),
        token: Arc::new(token),
        bound: ([127, 0, 0, 1], 8471).into(),
        jobs: Jobs::default(),
    })
}

/// What the route answered, as the status it answered under and what it said.
async fn said(random: Chance, action: &str, body: &str) -> (u16, String) {
    let request = axum::http::Request::builder()
        .method("POST")
        .uri(format!("/api/actions/{action}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(body.to_owned()));
    let Ok(request) = request else {
        unreachable!("a request built from values that are already headers cannot fail");
    };
    let served = tower::ServiceExt::oneshot(routed(random), request)
        .await
        .ok();
    let Some(response) = served else {
        unreachable!("the router is infallible; its handlers answer rather than fail");
    };
    let status = response.status().as_u16();
    let read = to_bytes(response.into_body(), usize::MAX).await;
    let bytes = read.map(|bytes| bytes.to_vec()).unwrap_or_default();
    (status, String::from_utf8(bytes).unwrap_or_default())
}

#[tokio::test]
async fn a_long_running_action_is_answered_with_a_name_and_left_to_run() {
    let (status, body) = said(Chance::cycling(), "up", r#"{"forms":["tv"]}"#).await;
    assert_eq!(status, StatusCode::ACCEPTED.as_u16());
    assert!(body.contains(r#""kind":"job""#), "{body}");
}

#[tokio::test]
async fn an_immediate_action_is_answered_with_the_outcome_itself() {
    let (status, body) = said(Chance::cycling(), "quality-reapply", "{}").await;
    assert_eq!(status, StatusCode::OK.as_u16());
    // The identical envelope the equivalent command emits, not a shape of its own.
    assert!(body.contains(r#""api_version":1"#), "{body}");
    assert!(body.contains(r#""kind":"quality""#), "{body}");
}

#[tokio::test]
async fn an_immediate_action_that_could_not_be_done_answers_with_the_failure() {
    // Nowhere to record a setting, so the command fails — and it fails in the
    // same envelope every other answer arrives in.
    let asked = r#"{"key":"DATA_ROOT","value":"/srv"}"#;
    let (status, body) = said(Chance::cycling(), "config-set", asked).await;
    // Not 200 with a failure inside it: a client's own idea of a successful call
    // should mean what it says, and which failure it was is the envelope's code.
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR.as_u16());
    assert!(body.contains(r#""kind":"error""#), "{body}");
}

#[tokio::test]
async fn a_name_the_route_does_not_offer_is_refused_by_the_route_too() {
    let (status, body) = said(Chance::cycling(), "reticulate", "{}").await;
    assert_eq!(status, StatusCode::NOT_FOUND.as_u16());
    assert!(body.contains("no action named"), "{body}");
}

#[tokio::test]
async fn an_action_missing_an_argument_is_refused_by_the_route_too() {
    let (status, body) = said(Chance::cycling(), "pull", "{}").await;
    assert_eq!(status, StatusCode::BAD_REQUEST.as_u16());
    assert!(body.contains("needs `forms`"), "{body}");
}

#[tokio::test]
async fn work_that_cannot_be_named_is_not_started() {
    // A job with no name is work nothing could ever be told about, so it is
    // refused rather than begun and lost.
    let asked = r#"{"forms":["tv"]}"#;
    let (status, body) = said(Chance::exactly(None), "up", asked).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR.as_u16());
    assert!(body.contains("randomness"), "{body}");
}
