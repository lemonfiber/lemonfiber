//! Reading and writing the settings.

use super::*;

/// A context whose settings live in a scratch file.
fn with_config(path: &std::path::Path) -> Ctx {
    let settings = Settings {
        env_file: Some(path.to_path_buf()),
        ..Settings::default()
    };
    a_context()
        .engine(Arc::new(Reporting::default()))
        .settings(settings)
        .build()
}

fn settings_of(
    outcome: Result<Outcome, Box<super::super::Problem>>,
) -> Option<Vec<(String, String)>> {
    match outcome {
        Ok(Outcome::Config(report)) => Some(
            report
                .settings
                .into_iter()
                .map(|setting| (setting.key, setting.value))
                .collect(),
        ),
        Ok(
            Outcome::Version(_)
            | Outcome::Alerts(_)
            | Outcome::Migration(_)
            | Outcome::History(_)
            | Outcome::Adoption(_)
            | Outcome::Beside(_)
            | Outcome::Replacement(_)
            | Outcome::Import(_)
            | Outcome::Forms(_)
            | Outcome::Preview(_)
            | Outcome::Lifecycle(_)
            | Outcome::Quality(_)
            | Outcome::Upgrade(_)
            | Outcome::Music(_)
            | Outcome::Trace(_)
            | Outcome::Hosting(_)
            | Outcome::Household(_)
            | Outcome::Held(_)
            | Outcome::FrontDoor(_)
            | Outcome::Stuck(_)
            | Outcome::Word(_)
            | Outcome::Glossary(_)
            | Outcome::Clients(_)
            | Outcome::Invited(_)
            | Outcome::Removed(_)
            | Outcome::Catalogue(_)
            | Outcome::Wiring(_)
            | Outcome::Substituted(_)
            | Outcome::Outbound(_)
            | Outcome::Plugins(_)
            | Outcome::Provenance(_)
            | Outcome::Credentials(_)
            | Outcome::Stored(_)
            | Outcome::SelfUpdate(_)
            | Outcome::Space(_)
            | Outcome::Letting(_)
            | Outcome::Bandwidth(_)
            | Outcome::Status(_)
            | Outcome::Doctor(_)
            | Outcome::Repair(_)
            | Outcome::Undo(_)
            | Outcome::Seed(_)
            | Outcome::Reset(_)
            | Outcome::Uninstall(_)
            | Outcome::Wizard(_)
            | Outcome::Update(_)
            | Outcome::Backup(_)
            | Outcome::Support(_)
            | Outcome::Archives(_)
            | Outcome::Restore(_)
            | Outcome::Watch(_)
            | Outcome::Walkthrough(_),
        )
        | Err(_) => None,
    }
}

#[tokio::test]
async fn a_setting_can_be_written_and_read_back() {
    let path = config_scratch("round-trip");
    let ctx = with_config(&path);

    let written = dispatch(
        Command::ConfigSet(Setting::to("LEMONFIBER_USENET", "on").agreed(true)),
        &ctx,
    )
    .await;
    assert_eq!(
        settings_of(written),
        Some(vec![("LEMONFIBER_USENET".to_owned(), "on".to_owned())])
    );

    let read = dispatch(
        Command::ConfigGet {
            key: "LEMONFIBER_USENET".to_owned(),
        },
        &ctx,
    )
    .await;
    assert_eq!(
        settings_of(read),
        Some(vec![("LEMONFIBER_USENET".to_owned(), "on".to_owned())])
    );

    let _ = std::fs::remove_dir_all(path.parent().unwrap_or(std::path::Path::new("/")));
}

#[tokio::test]
async fn a_rehearsed_change_reports_itself_and_writes_nothing() {
    let path = config_scratch("rehearsed");
    let ctx = with_config(&path).rehearsing();

    let outcome = dispatch(
        Command::ConfigSet(Setting::to("LEMONFIBER_TORRENT", "on").agreed(true)),
        &ctx,
    )
    .await;
    assert!(
        matches!(&outcome, Ok(Outcome::Config(report)) if report.changed && report.rehearsed),
        "a rehearsal reports the change it would make, and that it was a rehearsal"
    );
    assert!(!path.exists(), "a rehearsal writes nothing");
}

#[tokio::test]
async fn a_lifecycle_command_with_a_config_file_reports_no_edits_for_an_external_stack() {
    // With an environment file in hand, a lifecycle command derives where it would
    // keep the materialised-stack record beside it. The stack here is external —
    // the operator's own on disk — so nothing is written and no edit is reported.
    let path = config_scratch("lifecycle-config");
    let ctx = with_config(&path).rehearsing();
    let outcome = dispatch(
        Command::Up {
            forms: vec!["tv".to_owned()],
        },
        &ctx,
    )
    .await;
    let edits = report(outcome)
        .map(|report| report.stack_edits)
        .unwrap_or_default();
    assert!(
        edits.is_empty(),
        "an external stack is left as it is, so nothing is reported"
    );
    let _ = std::fs::remove_dir_all(path.parent().unwrap_or(std::path::Path::new("/")));
}

#[tokio::test]
async fn showing_settings_withholds_credentials() {
    let path = config_scratch("secrets");
    let ctx = with_config(&path);
    for (key, value) in [("DATA_ROOT", "/media"), ("WIREGUARD_PRIVATE_KEY", "abc123")] {
        let _ = dispatch(
            Command::ConfigSet(Setting::to(key, value).agreed(true)),
            &ctx,
        )
        .await;
    }

    let shown = settings_of(dispatch(Command::ConfigShow, &ctx).await).unwrap_or_default();
    assert_eq!(
        shown,
        vec![
            ("DATA_ROOT".to_owned(), "/media".to_owned()),
            (
                "WIREGUARD_PRIVATE_KEY".to_owned(),
                crate::config::store::REDACTED.to_owned()
            ),
        ]
    );

    let _ = std::fs::remove_dir_all(path.parent().unwrap_or(std::path::Path::new("/")));
}

#[tokio::test]
async fn a_configuration_answer_serialises_under_its_own_kind() {
    let path = config_scratch("envelope");
    let ctx = with_config(&path);
    let _ = dispatch(
        Command::ConfigSet(Setting::to("DATA_ROOT", "/media").agreed(true)),
        &ctx,
    )
    .await;

    let rendered = dispatch(Command::ConfigShow, &ctx)
        .await
        .ok()
        .map(Outcome::envelope)
        .and_then(|envelope| envelope.to_json().map(|json| (envelope.kind, json)));

    assert_eq!(
        rendered.map(|(kind, json)| (
            kind,
            json.starts_with(r#"{"api_version":1,"kind":"config","data":{"settings":["#),
            json.contains(r#""changed":false"#)
        )),
        Some((crate::model::kind::CONFIG, true, true))
    );

    let _ = std::fs::remove_dir_all(path.parent().unwrap_or(std::path::Path::new("/")));
}

#[tokio::test]
async fn asking_a_non_configuration_outcome_for_settings_gets_none() {
    let ctx = ctx(Ok(spoke("v2.32.1")));
    assert_eq!(settings_of(dispatch(Command::Version, &ctx).await), None);
}

#[cfg(unix)]
#[tokio::test]
async fn settings_that_cannot_be_saved_reach_the_operator() {
    use std::os::unix::fs::PermissionsExt as _;

    let dir = lemonfiber_fixtures::scratch::Scratch::named("app-ro");
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o500));

    let ctx = with_config(&dir.join(".env"));
    let refusal = dispatch(Command::ConfigSet(Setting::to("A", "1")), &ctx)
        .await
        .err()
        .map(|problem| problem.code);
    assert_eq!(
        refusal,
        Some(crate::error::codes::config::CONFIG_NOT_WRITTEN)
    );

    let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn settings_that_cannot_be_read_reach_the_operator() {
    // A file where the directory holding settings should be.
    let blocker = lemonfiber_fixtures::scratch::Scratch::named("app-blocked").kept();
    let _ = std::fs::remove_dir_all(&blocker);
    let _ = std::fs::write(&blocker, "in the way");

    let ctx = with_config(&blocker.join(".env"));
    let refusal = dispatch(Command::ConfigShow, &ctx)
        .await
        .err()
        .map(|problem| problem.code);
    assert_eq!(
        refusal,
        Some(crate::error::codes::config::CONFIG_UNREADABLE)
    );

    let _ = std::fs::remove_file(&blocker);
}

#[tokio::test]
async fn settings_with_nowhere_to_live_say_setup_has_not_run() {
    let ctx = a_context().engine(Arc::new(Reporting::default())).build();
    assert_eq!(
        dispatch(Command::ConfigShow, &ctx)
            .await
            .err()
            .map(|p| p.code),
        Some(crate::error::codes::config::CONFIG_NOWHERE)
    );
}

#[tokio::test]
async fn a_lifecycle_outcome_serialises_under_its_own_kind() {
    let ctx = rehearsing(crate::config::Protocols::both());
    let command = Command::Up {
        forms: vec!["library".to_owned()],
    };
    let rendered = dispatch(command, &ctx)
        .await
        .ok()
        .map(Outcome::envelope)
        .and_then(|envelope| envelope.to_json().map(|json| (envelope.kind, json)));

    assert_eq!(
        rendered.map(|(kind, json)| (
            kind,
            json.starts_with(r#"{"api_version":1,"kind":"lifecycle","data":{"action":"up""#),
            json.contains(r#""rehearsed":true"#)
        )),
        Some((crate::model::kind::LIFECYCLE, true, true))
    );
}
