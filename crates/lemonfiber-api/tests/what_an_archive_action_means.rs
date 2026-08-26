//! What a browser may ask to be captured, bundled and put back — and what it may not.
//!
//! The three actions that reach lemonfiber's own files on disk rather than the
//! container engine, and the two things about them that are this surface's own
//! problem rather than the core's.
//!
//! **A browser names, it does not point.** The server is on the host and runs as the
//! operator, so a path it accepted from a request would be a path it could read or
//! write. A capture takes no path at all, a bundle goes where lemonfiber keeps its
//! own files, and a restore names one of the archives this machine took.
//!
//! **A restore says what it would overwrite before it does.** Unconfirmed it is
//! answered now, with the listing, because a listing that arrives behind a job name
//! arrives after the moment it existed for. Confirmed it is a job like any other
//! wait, because by then the decision has been taken.
//!
//! Driven from outside the crate, because what a caller can reach is the thing
//! worth holding still.

use std::sync::Arc;

use axum::body::to_bytes;
use axum::http::{header, StatusCode};
use lemonfiber_api::actions;
use lemonfiber_api::actions::{answering, named, Answering, Arguments, Refused};
use lemonfiber_api::events::live::Live;
use lemonfiber_api::guard::Token;
use lemonfiber_api::jobs::Jobs;
use lemonfiber_api::router::Serving;
use lemonfiber_core::app::restore::{Consent, Kept};
use lemonfiber_core::app::support::Destination;
use lemonfiber_core::app::{bundle, Command, Ctx};
use lemonfiber_core::bundle::Filenames;
use lemonfiber_core::config::Settings;
use lemonfiber_core::platform::Environment;
use lemonfiber_fixtures::ports::{Chance, Idle, Stopped};

/// The name one of this machine's backups is written under.
const KEPT: &str = "lemonfiber-full-1700000000.tar.gz";

/// A listing, as one names itself.
const LISTING: &str = "5c3a1d20";

/// What an action came to, or nothing where it was refused.
fn command(action: &str, given: Arguments) -> Option<Command> {
    named(action, given).ok()
}

/// Why an action was refused, or nothing where it was not.
fn refusal(action: &str, given: Arguments) -> Option<Refused> {
    named(action, given).err()
}

/// A restore of the archive this machine keeps under [`KEPT`].
fn restoring(repoint: bool, confirm: bool) -> Arguments {
    Arguments {
        archive: Some(KEPT.to_owned()),
        repoint,
        confirm,
        ..Arguments::default()
    }
}

// ── What a browser may ask to be written ──────────────────────────────────────

#[test]
fn a_capture_takes_the_one_service_it_is_narrowed_to_and_no_path_at_all() {
    // Where an archive goes is lemonfiber's own answer on both surfaces, so there
    // is no path in this request to be wrong about.
    assert_eq!(
        command("backup", Arguments::default()),
        Some(Command::Backup { service: None })
    );
    let one = Arguments {
        service: Some("sonarr".to_owned()),
        ..Arguments::default()
    };
    assert_eq!(
        command("backup", one),
        Some(Command::Backup {
            service: Some("sonarr".to_owned())
        })
    );
}

#[test]
fn a_bundle_goes_where_lemonfiber_keeps_its_own_files() {
    // The only web-specific question a bundle has is which path, and it is answered
    // by the server rather than asked of a caller that has no filesystem in front
    // of it. A caller that names one is told there is no such argument.
    let asked = Arguments {
        write: true,
        logs: Some(50),
        filenames: Filenames::Shown,
        reveal: vec!["INDEXER_KEY".to_owned()],
        confirm: true,
        ..Arguments::default()
    };
    assert_eq!(
        command("support", asked),
        Some(Command::Support {
            write: true,
            wanted: bundle::Wanted::asked(
                50,
                Filenames::Shown,
                vec!["INDEXER_KEY".to_owned()],
                true
            ),
            dest: Destination::Kept,
        })
    );
    let named_a_path = serde_json::from_str::<Arguments>(r#"{"out":"/etc/lemonfiber.tar.gz"}"#);
    assert!(named_a_path.is_err(), "there is no such argument");
}

#[test]
fn a_bundle_asked_for_without_a_window_takes_the_one_the_command_line_takes() {
    let bare = Arguments {
        write: true,
        ..Arguments::default()
    };
    assert_eq!(
        command("support", bare),
        Some(Command::Support {
            write: true,
            wanted: bundle::Wanted::default(),
            dest: Destination::Kept,
        })
    );
}

#[test]
fn a_restore_names_one_of_this_machines_backups_rather_than_a_path() {
    // The name is carried as a name and resolved beneath the backups directory by
    // the core, so nothing this surface hands over is a path the server can read.
    assert_eq!(
        command("restore", restoring(true, true)),
        Some(Command::Restore {
            archive: Kept::Named(KEPT.to_owned()),
            repoint: true,
            consent: Consent::Standing,
        })
    );
}

/// The listing a restore was read in travels with the yes, and both halves are
/// carried: the listing reaches the command, and naming one without the yes is
/// refused rather than read as either answer.
///
/// A browser reads the listing in one request and answers in another, and in that
/// gap what the archive would do can move — so the yes says which listing it was
/// given for, the way a repair's says which offer.
#[test]
fn a_restore_carries_the_listing_its_yes_was_read_in() {
    let answered = Arguments {
        offer: Some(LISTING.to_owned()),
        ..restoring(true, true)
    };
    assert_eq!(
        command("restore", answered),
        Some(Command::Restore {
            archive: Kept::Named(KEPT.to_owned()),
            repoint: true,
            consent: Consent::Given {
                listing: LISTING.to_owned()
            },
        })
    );

    // A listing named without the yes is an answer to a question nobody was asked:
    // it would restore or not restore depending on which half was believed.
    let unanswered = Arguments {
        offer: Some(LISTING.to_owned()),
        ..restoring(true, false)
    };
    assert_eq!(
        refusal("restore", unanswered),
        Some(Refused::Missing {
            action: "restore".to_owned(),
            argument: "confirm".to_owned()
        })
    );
}

#[test]
fn a_restore_that_names_no_archive_says_which_argument_it_wanted() {
    assert_eq!(
        refusal("restore", Arguments::default()),
        Some(Refused::Missing {
            action: "restore".to_owned(),
            argument: "archive".to_owned()
        })
    );
}

// ── The listing arrives before the overwrite, not behind a job name ───────────

#[test]
fn an_unconfirmed_restore_is_answered_now_and_a_confirmed_one_is_left_to_run() {
    let Some(listing) = command("restore", restoring(false, false)) else {
        unreachable!("a named archive reaches a command");
    };
    let Some(overwrite) = command("restore", restoring(false, true)) else {
        unreachable!("a named archive reaches a command");
    };
    assert_eq!(answering(&listing), Answering::Now);
    assert_eq!(answering(&overwrite), Answering::Later);
}

#[test]
fn a_capture_and_a_bundle_are_waits_an_operator_can_watch() {
    // Both prove the stack is stopped or read what the services have been saying,
    // so both reach the engine — and a request that waited for either would tie
    // minutes of work to one connection.
    let Some(capture) = command("backup", Arguments::default()) else {
        unreachable!("a capture takes nothing and reaches a command");
    };
    let Some(gathering) = command("support", Arguments::default()) else {
        unreachable!("a bundle takes nothing and reaches a command");
    };
    assert_eq!(answering(&capture), Answering::Later);
    assert_eq!(answering(&gathering), Answering::Later);
}

// ── The route itself, driven without a socket ─────────────────────────────────

/// A context with no archives and nothing running, which is enough to prove which
/// of the two answers a request gets.
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
fn routed() -> axum::Router {
    let Some(token) = Token::mint(&Chance::cycling()) else {
        unreachable!("cycling letters always supply bytes");
    };
    actions::routes().with_state(Serving {
        ctx: Arc::new(ctx()),
        token: Arc::new(token),
        bound: ([127, 0, 0, 1], 8471).into(),
        admitting: Arc::new(lemonfiber_api::admission::Admitting::default()),
        jobs: Jobs::default(),
        live: Arc::new(Live::opening(Stopped::at(0).as_ref())),
    })
}

/// What the route answered, as the status it answered under and what it said.
async fn said(action: &str, body: &str) -> (u16, String) {
    let request = axum::http::Request::builder()
        .method("POST")
        .uri(format!("/api/actions/{action}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(body.to_owned()));
    let Ok(request) = request else {
        unreachable!("a request built from values that are already headers cannot fail");
    };
    let served = tower::ServiceExt::oneshot(routed(), request).await.ok();
    let Some(response) = served else {
        unreachable!("the router is infallible; its handlers answer rather than fail");
    };
    let status = response.status().as_u16();
    let read = to_bytes(response.into_body(), usize::MAX).await;
    let bytes = read.map(|bytes| bytes.to_vec()).unwrap_or_default();
    (status, String::from_utf8(bytes).unwrap_or_default())
}

#[tokio::test]
async fn a_restore_that_has_not_been_agreed_to_answers_with_what_it_would_overwrite() {
    // This run keeps no archives, so what comes back is the refusal rather than a
    // listing — but it comes back *now*, in the envelope, which is the property
    // this holds: the answer to an unconfirmed restore is not a job name.
    let asked = format!(r#"{{"archive":"{KEPT}"}}"#);
    let (status, body) = said("restore", &asked).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR.as_u16());
    assert!(body.contains(r#""kind":"error""#), "{body}");
    assert!(body.contains("RESTORE-9"), "{body}");
}

#[tokio::test]
async fn a_confirmed_restore_is_answered_with_a_name_for_the_work() {
    let asked = format!(r#"{{"archive":"{KEPT}","confirm":true}}"#);
    let (status, body) = said("restore", &asked).await;
    assert_eq!(status, StatusCode::ACCEPTED.as_u16());
    assert!(body.contains(r#""kind":"job""#), "{body}");
    assert!(body.contains(r#""action":"restore""#), "{body}");
}

#[tokio::test]
async fn a_capture_and_a_bundle_are_answered_with_a_name_for_the_work() {
    for action in ["backup", "support"] {
        let (status, body) = said(action, "{}").await;
        assert_eq!(status, StatusCode::ACCEPTED.as_u16(), "{action}: {body}");
        assert!(body.contains(r#""kind":"job""#), "{action}: {body}");
    }
}

#[tokio::test]
async fn a_path_where_a_backup_name_was_expected_is_refused_by_name() {
    // Carried through as a name, and refused by the core because it is not one of
    // the archives this machine kept — which is the whole of what a browser may
    // ask to be read.
    let asked = r#"{"archive":"../../etc/passwd"}"#;
    let (status, body) = said("restore", asked).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR.as_u16());
    assert!(body.contains(r#""kind":"error""#), "{body}");
}
