//! Offering somebody an account, driven from outside the crate.
//!
//! The command's own tests live beside it. This drives the same request through the
//! library as every surface links it, because the app layer is compiled twice — once
//! with its in-crate tests and once as the library these binaries link — and a path
//! exercised from only one of those leaves the other copy counted as never run.
//!
//! It also asserts the shape a browser is handed, which is the promise the core
//! makes rather than the terminal's: an invitation that serialised under the wrong
//! name would still print correctly at a prompt.

use std::sync::Arc;

use crate::common::household::recorded_admin;
use lemonfiber_core::app::{dispatch, Allowance, Command, Ctx, Inviting, Outcome};
use lemonfiber_core::config::Settings;
use lemonfiber_core::model::Linked;
use lemonfiber_core::ports::http::Request;
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::support::Reporting;
use lemonfiber_testing::media;
use lemonfiber_testing::requests::{self, REQUESTS};

/// A media server that signs in and takes the account it is given, beside a request
/// service that takes lemonfiber's key and whose adapter answers being told about the
/// household as `linking` does.
fn answering_with(linking: Answer) -> Arc<Fake> {
    let mut routes = vec![
        (requests::intake("answers"), vec![requests::taken()]),
        (requests::intake("link_members"), vec![linking]),
    ];
    // The owner is the one account the media server already holds, before this
    // invitation.
    routes.extend(media::household(
        &[media::member("1", "owner", true)],
        &media::member("9", "ana", false),
        &[],
    ));
    Fake::by_path_in_turn(routes)
}

/// Whether an account was asked for.
fn made_an_account(sent: &[Request]) -> bool {
    !media::asked(sent, "invite").is_empty()
}

/// Everybody the request service was told about, each time it was told.
fn told_about(sent: &[Request]) -> Vec<Vec<String>> {
    requests::told(sent, "link_members", "members")
}

/// A media server and a request service that both answer.
fn answering() -> Arc<Fake> {
    answering_with(requests::taken())
}

/// A context over the shipped stack, with the media server and the request service moved
/// into their plugins and up, and a password recorded.
fn context(env: &std::path::Path) -> Ctx {
    context_over(env, answering())
}

/// The same, over a transport the caller chose.
fn context_over(env: &std::path::Path, http: Arc<Fake>) -> Ctx {
    let stack = crate::common::household::set_up_beside(env);
    REQUESTS.keyed_in(context_on(env, http, stack, REQUESTS.running()))
}

/// The same again, over a stack the caller chose and with the engine also holding what
/// `beside` holds, the request service moved in only where the caller moves it.
fn context_on(env: &std::path::Path, http: Arc<Fake>, stack: Source, beside: Reporting) -> Ctx {
    media::keyed_in(
        lemonfiber_testing::a_context()
            .engine(Arc::new(media::running().alongside(beside)))
            .clock(Arc::new(lemonfiber_adapters::System))
            .over(stack)
            .settings(Settings {
                env_file: Some(env.to_path_buf()),
                household_host: Some("192.168.1.20".to_owned()),
                ..Settings::default()
            })
            .build()
            .with_http(http),
    )
}

#[tokio::test]
async fn an_invitation_carries_one_address_and_the_name_to_sign_in_as() {
    let env = recorded_admin("outside");
    let ctx = context(&env);

    let made = dispatch(
        Command::Invite(Inviting {
            name: "ana".to_owned(),
            allowance: Allowance::default(),
            confirm: true,
        }),
        &ctx,
    )
    .await;

    let Some(Outcome::Invitation(report)) = made.ok() else {
        unreachable!("the invite command answers with an invitation")
    };
    assert_eq!(report.name, "ana");
    assert!(
        report.address.starts_with("http"),
        "an invitation was made with nowhere to send anybody: {}",
        report.address
    );
    assert!(report.hours > 0, "an invitation that stands for no time");
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// The request service is told about the whole household, not only the new account.
///
/// It skips anybody it already holds, so naming everybody costs nothing and is what
/// completes a link an earlier run could not make — without anything having been
/// written down in between.
#[tokio::test]
async fn the_request_service_is_told_about_everybody_and_not_only_the_new_account() {
    let env = recorded_admin("linked");
    let http = answering();
    let ctx = context_over(&env, http.clone());

    let made = dispatch(
        Command::Invite(Inviting {
            name: "ana".to_owned(),
            allowance: Allowance::default(),
            confirm: true,
        }),
        &ctx,
    )
    .await;

    assert_eq!(
        told_about(&http.requests()),
        [["1".to_owned(), "9".to_owned()]],
        "the household already here was left out, or told about more than once"
    );
    assert!(
        made.is_ok_and(
            |outcome| matches!(outcome, Outcome::Invitation(report) if report.linked == Linked::Made)
        ),
        "the link was made but not reported as made"
    );
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// An invitation stands when the request service will not answer.
///
/// The account somebody watches with is the media server's, and it is made whether or
/// not a second service is up. What they cannot do yet is reported; it does not cost
/// them the account, which would be the operator's work thrown away for an outage.
#[tokio::test]
async fn an_invitation_stands_when_the_request_service_will_not_answer() {
    let env = recorded_admin("unlinked");
    let http = answering_with(requests::unavailable());
    let ctx = context_over(&env, http.clone());

    let made = dispatch(
        Command::Invite(Inviting {
            name: "ana".to_owned(),
            allowance: Allowance::default(),
            confirm: true,
        }),
        &ctx,
    )
    .await;

    let Some(Outcome::Invitation(report)) = made.ok() else {
        unreachable!("a request service that is down is not a reason to refuse an invitation")
    };
    assert_eq!(report.name, "ana", "the account was not made");
    assert_eq!(
        report.linked,
        Linked::NotYet,
        "a link that was not made was reported as though it had been"
    );
    assert!(
        made_an_account(&http.requests()),
        "the media-server account was never even attempted"
    );
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// Somebody already in the house is named to the request service once, not twice.
///
/// No account is made for them — the one they have is theirs — so the identifier that
/// goes is the one already in the household rather than a second copy of it.
#[tokio::test]
async fn somebody_already_here_is_named_once() {
    let env = recorded_admin("already-here");
    let http = answering();
    let ctx = context_over(&env, http.clone());

    let made = dispatch(
        Command::Invite(Inviting {
            name: "owner".to_owned(),
            allowance: Allowance::default(),
            confirm: true,
        }),
        &ctx,
    )
    .await;

    assert_eq!(
        told_about(&http.requests()),
        [["1".to_owned()]],
        "the household already here was named more than once, or not at all"
    );
    assert!(
        !made_an_account(&http.requests()),
        "a second account was made for somebody who already has one"
    );
    assert!(made.is_ok(), "{made:?}");
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// A stack with no request service tells nothing, and calls it nothing tried.
///
/// Not `NotYet`: there is no service here that a later run could reach, so saying "not
/// yet" would promise something that is never coming. The household on such a stack
/// watches and does not ask, which is a shape of stack rather than a fault in one.
///
/// **Driven from out here as well as in-crate**, because the app layer is compiled
/// twice — once with its `#[cfg(test)]` modules and once as the library these binaries
/// link — and this path exists only on a stack the shipped manifest is not, so the copy
/// without the in-crate test would otherwise never run it.
#[tokio::test]
async fn a_stack_with_no_request_service_calls_it_nothing_tried() {
    static WITHOUT: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
    let dir = WITHOUT.get_or_init(|| {
        let to = lemonfiber_fixtures::scratch::Scratch::named("no-asking").kept();
        lemonfiber_fixtures::stack::manifest_without(&to, "seerr");
        to
    });

    let env = recorded_admin("no-asking");
    let http = answering();
    let ctx = context_on(
        &env,
        http.clone(),
        Source::External(Box::leak(dir.clone().into_boxed_path())),
        Reporting::default(),
    );

    let made = dispatch(
        Command::Invite(Inviting {
            name: "ana".to_owned(),
            allowance: Allowance::default(),
            confirm: true,
        }),
        &ctx,
    )
    .await;

    let Some(Outcome::Invitation(report)) = made.ok() else {
        unreachable!("a stack with no request service still makes the account")
    };
    assert_eq!(report.name, "ana", "the account was not made");
    assert_eq!(
        report.linked,
        Linked::NotTried,
        "a stack with no request service was told one might answer later"
    );
    assert!(
        !http.asked_for(requests::intake("link_members")),
        "a stack with no request service was asked to link somebody"
    );
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// A rehearsal tells the request service nothing, as it makes no account.
#[tokio::test]
async fn a_rehearsal_tells_the_request_service_nothing() {
    let env = recorded_admin("rehearsed-link");
    let http = answering();
    let ctx = context_over(&env, http.clone()).rehearsing();

    let made = dispatch(
        Command::Invite(Inviting {
            name: "ana".to_owned(),
            allowance: Allowance::default(),
            confirm: true,
        }),
        &ctx,
    )
    .await;

    assert!(
        made.is_ok_and(|outcome| matches!(outcome, Outcome::Invitation(report) if report.linked == Linked::NotTried)),
        "a rehearsal reported a link it did not make"
    );
    assert!(
        !http.asked_for(requests::intake("link_members")),
        "a rehearsal told the request service about somebody"
    );
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// What a browser is handed, which is the core's promise rather than the terminal's.
#[tokio::test]
async fn an_invitation_serialises_under_its_own_name() {
    let env = recorded_admin("json-outside");
    let ctx = context(&env);

    let json = dispatch(
        Command::Invite(Inviting {
            name: "ana".to_owned(),
            allowance: Allowance::default(),
            confirm: true,
        }),
        &ctx,
    )
    .await
    .ok()
    .and_then(|outcome| outcome.envelope().to_json())
    .unwrap_or_default();

    assert!(json.contains("\"invitation\""), "{json}");
    assert!(json.contains("ana"), "{json}");
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}
