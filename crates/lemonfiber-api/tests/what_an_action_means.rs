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

use std::collections::BTreeSet;
use std::sync::Arc;

use axum::body::to_bytes;
use axum::http::{header, StatusCode};
use lemonfiber_api::actions;
use lemonfiber_api::actions::{answering, declined, named, Answering, Arguments, Refused, OFFERED};
use lemonfiber_api::guard::Token;
use lemonfiber_api::jobs::Jobs;
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
fn a_refusal_says_which_of_the_four_it_was() {
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
        Refused::Unwanted {
            action: "down".to_owned(),
            argument: "confirm".to_owned(),
        },
    ];
    let spoken: BTreeSet<String> = said.iter().map(Refused::said).collect();
    for refusal in &said {
        assert!(!refusal.said().is_empty(), "{refusal:?}");
    }
    assert_eq!(
        spoken.len(),
        said.len(),
        "and each says something different"
    );
}

#[test]
fn a_name_this_surface_does_not_offer_is_absent_before_its_arguments_are_judged() {
    // A name nothing answers to is the first thing wrong with such a request, and
    // saying what its arguments should have been would be answering about an
    // action that does not exist.
    let given = Arguments {
        confirm: true,
        ..Arguments::default()
    };
    assert_eq!(
        refusal("reticulate", given),
        Some(Refused::Unknown {
            name: "reticulate".to_owned()
        })
    );
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
    assert_eq!(
        Refused::Unwanted {
            action: "down".to_owned(),
            argument: "confirm".to_owned()
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
fn starting_named_services_is_refused_rather_than_answered_with_the_whole_form() {
    // `--service` on a start never reaches a `Command`: the command line runs its
    // own streamed start around it, so there is nothing here to hand them to.
    // Dropped, they would start every service the form holds — the answer to a
    // request nobody made — so they are refused instead, and the caller is told.
    let given = Arguments {
        forms: vec!["tv".to_owned()],
        services: vec!["sonarr".to_owned()],
        ..Arguments::default()
    };
    assert_eq!(
        refusal("up", given),
        Some(Refused::Unwanted {
            action: "up".to_owned(),
            argument: "services".to_owned()
        })
    );
    // Naming none, it is the start it says it is.
    assert_eq!(
        command("up", naming("tv")),
        Some(Command::Up {
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

// ── An argument reaches the commands that carry it, and no others ─────────────

/// Whether the command has the operator's agreement in it.
fn carries_agreement(command: &Command) -> bool {
    matches!(
        command,
        Command::Quality(QualityAction::Set { confirm: true, .. })
            | Command::QualityUpgrade { confirm: true }
            | Command::Reset { confirm: true }
    )
}

/// Whether the command has the services it was given in it.
fn carries_services(command: &Command) -> bool {
    match command {
        Command::Halt { services, .. } | Command::Restart { services, .. } => !services.is_empty(),
        _ => false,
    }
}

/// Everything an action wants, so a refusal is about the argument under test
/// rather than about a missing one.
fn wanting() -> Arguments {
    Arguments {
        forms: vec!["tv".to_owned()],
        key: Some("DATA_ROOT".to_owned()),
        value: Some("/srv".to_owned()),
        preset: Some("balanced".to_owned()),
        ..Arguments::default()
    }
}

/// Every offered action swept for one argument, gathering what is wrong.
///
/// The property rather than a second copy of the table: an action may accept an
/// argument only if the command it reaches has somewhere to put it, and must refuse
/// it by that name otherwise. An action that takes one and drops it is what this
/// exists to catch, and dropping it is invisible from outside — the request is
/// carried out, as something else.
fn swept(argument: &str, given: &Arguments, carries: fn(&Command) -> bool) -> Vec<String> {
    let mut wrong: Vec<String> = Vec::new();
    for action in OFFERED {
        match named(action, given.clone()) {
            Ok(command) if carries(&command) => {}
            Ok(command) => wrong.push(format!(
                "{action} took `{argument}` and dropped it: {command:?}"
            )),
            Err(Refused::Unwanted {
                argument: named, ..
            }) if named == argument => {}
            Err(refused) => wrong.push(format!(
                "{action} was refused for something else: {refused:?}"
            )),
        }
    }
    wrong
}

#[test]
fn agreement_is_taken_exactly_where_a_command_carries_it() {
    let agreeing = Arguments {
        confirm: true,
        ..wanting()
    };
    let wrong = swept("confirm", &agreeing, carries_agreement);
    assert!(wrong.is_empty(), "{wrong:?}");
}

#[test]
fn named_services_are_taken_exactly_where_a_command_carries_them() {
    // The sweep that would have caught `up`: it accepted the services it was given
    // and started the whole form, and nothing said so.
    let naming_services = Arguments {
        services: vec!["sonarr".to_owned()],
        ..wanting()
    };
    let wrong = swept("services", &naming_services, carries_services);
    assert!(wrong.is_empty(), "{wrong:?}");
}

#[test]
fn stopping_a_whole_stack_is_not_gated_the_way_a_reset_is() {
    // A teardown removes what a form started and `up` puts it back, so there is no
    // cost to agree to in advance, and the command line declares no such flag on
    // it. The command it reaches carries no agreement at all.
    assert_eq!(
        command("down", naming("tv")),
        Some(Command::Down {
            forms: vec!["tv".to_owned()]
        })
    );
    let agreed = Arguments {
        forms: vec!["tv".to_owned()],
        confirm: true,
        ..Arguments::default()
    };
    assert_eq!(
        refusal("down", agreed),
        Some(Refused::Unwanted {
            action: "down".to_owned(),
            argument: "confirm".to_owned()
        })
    );
}

#[test]
fn stopping_named_services_takes_no_agreement_either() {
    let agreed = Arguments {
        forms: vec!["tv".to_owned()],
        services: vec!["sonarr".to_owned()],
        confirm: true,
        ..Arguments::default()
    };
    assert_eq!(
        refusal("down", agreed),
        Some(Refused::Unwanted {
            action: "down".to_owned(),
            argument: "confirm".to_owned()
        })
    );
}

#[test]
fn choosing_for_music_takes_the_agreement_and_drops_it_as_the_command_line_does() {
    // Picking an audio format is not a choice this host has to transcode for, so
    // the agreement has nowhere to go — and `quality set --for music --confirm`
    // drops it too. Refusing it here would make this surface the stricter of the
    // two, which is the same divergence as offering something the other cannot.
    let given = Arguments {
        preset: Some("lossless".to_owned()),
        media_type: Some("music".to_owned()),
        confirm: true,
        ..Arguments::default()
    };
    assert!(matches!(
        command("quality-set", given),
        Some(Command::QualityMusic { .. })
    ));
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

// ── The route itself, driven without a socket ─────────────────────────────────

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
async fn agreement_an_action_does_not_take_is_refused_by_the_route_too() {
    let asked = r#"{"forms":["tv"],"confirm":true}"#;
    let (status, body) = said(Chance::cycling(), "down", asked).await;
    assert_eq!(status, StatusCode::BAD_REQUEST.as_u16());
    assert!(body.contains("takes no `confirm`"), "{body}");
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
