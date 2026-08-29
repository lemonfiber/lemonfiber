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

use lemonfiber_core::app::{dispatch, Command, Ctx, Outcome};
use lemonfiber_core::config::Settings;
use lemonfiber_core::platform::Environment;
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::support::{spoke, Reporting, Scripted};
use lemonfiber_ports::docker::{Health, Lifecycle};

/// The stack this repository ships.
fn stack() -> Source {
    Source::External(std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/media-stack"
    )))
}

/// A scratch environment file holding the media server's recorded password.
fn recorded_admin(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "lemonfiber-invitation-{}-{name}",
        std::process::id()
    ));
    let _ = std::fs::create_dir_all(&dir);
    let env = dir.join(".env");
    let _ = lemonfiber_core::config::store::set(
        &env,
        lemonfiber_core::config::JELLYFIN_ADMIN_PASSWORD_KEY,
        "minted-earlier",
    );
    env
}

/// A media server that signs in, holds nobody, and takes the account it is given.
fn answering() -> Arc<Fake> {
    let signed_in = Answer::reply(200, r#"{"AccessToken":"token"}"#);
    Fake::by_path_in_turn(vec![
        (
            "/Users/AuthenticateByName",
            vec![signed_in.clone(), signed_in.clone(), signed_in],
        ),
        (
            "/System/ActivityLog",
            vec![Answer::reply(200, r#"{"Items":[]}"#)],
        ),
        (
            "/Users/New",
            vec![Answer::reply(
                200,
                r#"{"Id":"9","Name":"ana","HasPassword":false}"#,
            )],
        ),
        ("/Users", vec![Answer::reply(200, "[]")]),
    ])
}

/// A context over the shipped stack, with the media server up and a password recorded.
fn context(env: &std::path::Path) -> Ctx {
    Ctx::new(
        Arc::new(Scripted(Ok(spoke("")))),
        Arc::new(Reporting::holding(
            &["jellyfin"],
            Lifecycle::Running,
            Health::Healthy,
        )),
        Arc::new(lemonfiber_core::adapters::System),
        Arc::new(lemonfiber_core::adapters::Disk),
        stack(),
        Settings {
            env_file: Some(env.to_path_buf()),
            // Where the household reaches this machine. Without one there is no
            // address to send anybody, which is a refusal rather than an invitation.
            household_host: Some("192.168.1.20".to_owned()),
            ..Settings::default()
        },
        Environment::MacOs,
    )
    .with_http(answering())
}

#[tokio::test]
async fn an_invitation_carries_one_address_and_the_name_to_sign_in_as() {
    let env = recorded_admin("outside");
    let ctx = context(&env);

    let made = dispatch(
        Command::Invite {
            name: "ana".to_owned(),
        },
        &ctx,
    )
    .await;

    let Some(Outcome::Invited(report)) = made.ok() else {
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

/// What a browser is handed, which is the core's promise rather than the terminal's.
#[tokio::test]
async fn an_invitation_serialises_under_its_own_name() {
    let env = recorded_admin("json-outside");
    let ctx = context(&env);

    let json = dispatch(
        Command::Invite {
            name: "ana".to_owned(),
        },
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
