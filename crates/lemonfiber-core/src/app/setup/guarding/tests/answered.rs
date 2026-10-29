//! The VPN step answered over a request, applied, and taken back where what it gives the
//! guard no longer stands.

use std::sync::Arc;

use super::{given, GUARD};
use crate::app::setup::{setup, SetupAction};
use crate::app::Ctx;
use crate::config::paths::{Paths, SECRETS, SETTINGS};
use crate::config::{store, Protocols, Settings};
use crate::error::codes::setup::GUARD_UNANSWERED;
use crate::error::Code;
use crate::journal::Kind;
use crate::validate::{Credential, Validation, Validator};
use crate::wizard::{Answer, Guarded, Library, Step, Vpn, Wizard};

/// What the guard is answered with to dial its tunnel, assembled so no credential sits
/// here.
fn dialling() -> String {
    ["wireguard", "not", "real"].join("-")
}

/// A validator that proves whatever it is asked.
struct Proving;

#[async_trait::async_trait]
impl Validator for Proving {
    async fn validate(&self, _credential: &Credential) -> Validation {
        Validation::Valid {
            observed: "answered".to_owned(),
        }
    }
}

/// A stack of no services of its own, so the guard is the one its plugin brings.
const ROOT: &str = r#"schema_version  = 1
stack_version   = "1.0.0"
min_cli_version = "0.1.0"

[[profile]]
id          = "torrent"
name        = "Torrents"
description = "What carries the torrents."

[[form]]
id          = "dl"
name        = "Download"
description = "Torrents, through the tunnel."
profiles    = ["torrent"]
"#;

/// A machine part-way through setup, torrents chosen, over [`ROOT`] with [`GUARD`]
/// installed.
fn machine(name: &str) -> (lemonfiber_fixtures::scratch::Scratch, Paths, Ctx) {
    let dir = lemonfiber_fixtures::scratch::Scratch::unmade(name);
    let paths = Paths::rooted(&dir.join("config"), &dir.join("data"));
    let at = dir.join("stack-source");
    let _ = std::fs::create_dir_all(&at);
    let _ = std::fs::write(at.join("stack.toml"), ROOT);
    let ctx = crate::test_support::a_context()
        .over(crate::stack::Source::External(Box::leak(
            at.into_boxed_path(),
        )))
        .settings(Settings {
            env_file: Some(paths.env_file()),
            stack_dir: Some(paths.stack()),
            ..Settings::default()
        })
        .build()
        .with_validator(Arc::new(Proving));
    let mut register = crate::plugin::Register::empty();
    if let Ok(manifest) = lemonfiber_plugin::Manifest::from_toml(GUARD) {
        let _ = register.record(crate::plugin::Installed::of(&manifest));
    }
    if let Some(kept) = crate::app::plugins::kept_at(&ctx) {
        if let Some(parent) = kept.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(kept, register.to_json().unwrap_or_default());
    }
    let mut wizard = Wizard::new(ctx.environment);
    let _ = wizard.answer(Answer::Protocols(Protocols {
        usenet: false,
        torrent: true,
    }));
    progressed(&paths, &wizard);
    (dir, paths, ctx)
}

/// `wizard`'s progress written where setup keeps it.
fn progressed(paths: &Paths, wizard: &Wizard) {
    let _ = store::write(
        &paths.setup_progress(),
        &serde_json::to_string(wizard.progress()).unwrap_or_default(),
    );
}

/// The progress file's text, or nothing where there is none.
fn progress(paths: &Paths) -> String {
    std::fs::read_to_string(paths.setup_progress()).unwrap_or_default()
}

/// The VPN answer given as `given`, over a request.
fn carrying(given: Guarded) -> SetupAction {
    SetupAction::Answer(Answer::Vpn(Vpn::Carrying(given)))
}

/// Which refusal `action` met, or nothing where it met none.
async fn refused(ctx: &Ctx, action: SetupAction) -> Option<Code> {
    setup(ctx, action).await.err().map(|problem| problem.code)
}

/// Every answer after the VPN step a torrent run on this machine asks.
async fn the_rest(ctx: &Ctx, paths: &Paths) {
    for answer in [
        Answer::DataLocation(paths.data_dir().join("media")),
        Answer::Credentials(None),
        Answer::ServiceUser(None),
        Answer::Library(Library::None),
        Answer::Household(false),
        Answer::Notifications(crate::alert::Appetite::default_appetite()),
        Answer::Autostart(false),
    ] {
        let _ = setup(ctx, SetupAction::Answer(answer)).await;
    }
}

#[tokio::test]
async fn the_step_says_what_it_asks_of_the_guard() {
    let (_dir, _paths, ctx) = machine("guard-asked");

    let report = setup(&ctx, SetupAction::Where).await;

    assert!(
        report.as_ref().is_ok_and(|report| report.guard.len() == 5),
        "{report:?}"
    );
}

#[tokio::test]
async fn an_answer_the_guard_does_not_take_is_refused_and_writes_none_of_its_secrets() {
    let (_dir, paths, ctx) = machine("guard-refused");
    let before = progress(&paths);
    let dialled = dialling();

    let missing = refused(
        &ctx,
        carrying(given(
            &[("guard:vpn-type", "openvpn")],
            &[("guard:wireguard-key", &dialled)],
        )),
    )
    .await;
    let foreign = refused(
        &ctx,
        carrying(given(
            &[("guard:vpn-type", "wireguard")],
            &[("guard:wireguard-key", &dialled), ("indexer:key", &dialled)],
        )),
    )
    .await;

    assert_eq!(missing, Some(GUARD_UNANSWERED));
    assert_eq!(foreign, Some(GUARD_UNANSWERED));
    assert_eq!(progress(&paths), before);
    assert!(!progress(&paths).contains(&dialled));
}

#[tokio::test]
async fn an_answer_it_takes_is_applied_journalled_and_its_secrets_leave_no_copy_behind() {
    let (_dir, paths, ctx) = machine("guard-applied");
    let dialled = dialling();

    let answered = setup(
        &ctx,
        carrying(given(
            &[("guard:vpn-type", "wireguard"), ("guard:provider", "ivpn")],
            &[("guard:wireguard-key", &dialled)],
        )),
    )
    .await;
    the_rest(&ctx, &paths).await;
    let reviewed = setup(&ctx, SetupAction::Where).await;
    let applied = setup(&ctx, SetupAction::Apply).await;

    assert!(answered.is_ok(), "{answered:?}");
    assert!(
        reviewed.as_ref().is_ok_and(|report| report
            .plan
            .iter()
            .any(|one| one.key == "guard:wireguard-key" && one.secret && one.value != dialled)),
        "{reviewed:?}"
    );
    assert!(applied.is_ok(), "{applied:?}");
    let settings =
        std::fs::read_to_string(paths.env_file().with_file_name(SETTINGS)).unwrap_or_default();
    assert!(settings.contains("guard:provider") && settings.contains("ivpn"));
    let secrets = crate::settings::Secrets::at(&paths.env_file().with_file_name(SECRETS));
    assert!(secrets.is_ok_and(|kept| kept.value("guard:wireguard-key") == Some(dialled.as_str())));
    assert!(!paths.setup_progress().exists());
    let env = std::fs::read_to_string(paths.env_file()).unwrap_or_default();
    let journal = std::fs::read_to_string(paths.journal()).unwrap_or_default();
    assert!(!env.contains(&dialled) && !journal.contains(&dialled));
    let kinds: Vec<Kind> = crate::app::recover::journal_at(&paths.journal())
        .map(|journal| {
            journal
                .changes()
                .iter()
                .map(|one| one.kind.clone())
                .collect()
        })
        .unwrap_or_default();
    assert!(kinds
        .iter()
        .any(|kind| matches!(kind, Kind::Secret { name, .. } if name == "guard:wireguard-key")));
    assert!(kinds.iter().any(
        |kind| matches!(kind, Kind::Setting { name, current, .. } if name == "guard:provider" && current.as_deref() == Some("ivpn"))
    ));
}

#[tokio::test]
async fn a_recorded_answer_the_guard_does_not_take_is_refused_at_apply_and_asked_again() {
    let dialled = dialling();
    for (case, recorded) in [
        ("bare", serde_json::json!("carrying")),
        (
            "tampered",
            serde_json::json!({"carrying": {
                "settings": {"guard:vpn-type": "wireguard"},
                "secrets": {"guard:wireguard-key": dialled, "indexer:key": dialled},
            }}),
        ),
    ] {
        let (_dir, paths, ctx) = machine(&format!("guard-recorded-{case}"));
        the_rest(&ctx, &paths).await;
        let mut edited: serde_json::Value =
            serde_json::from_str(&progress(&paths)).unwrap_or_default();
        if let Some(answers) = edited.get_mut("answers") {
            answers["vpn"] = recorded;
        }
        let _ = std::fs::write(paths.setup_progress(), edited.to_string());

        let applied = refused(&ctx, SetupAction::Apply).await;
        let after = setup(&ctx, SetupAction::Where).await;

        assert_eq!(applied, Some(GUARD_UNANSWERED), "{case}");
        assert!(
            after.as_ref().is_ok_and(
                |report| report.at == Step::Vpn && report.unanswered.contains(&Step::Vpn)
            ),
            "{case}: {after:?}"
        );
        assert!(!paths.env_file().with_file_name(SECRETS).exists(), "{case}");
        assert!(
            !paths.env_file().with_file_name(SETTINGS).exists(),
            "{case}"
        );
        assert!(!progress(&paths).contains(&dialled), "{case}");
    }
}

#[tokio::test]
async fn with_no_guard_an_answer_that_gives_it_anything_is_refused() {
    let (_dir, paths, mut ctx) = machine("guard-none");
    ctx.stack = crate::stack::Source::External(std::path::Path::new("/lemonfiber-not-a-stack"));

    let given_anything = refused(&ctx, carrying(given(&[], &[("indexer:key", &dialling())]))).await;

    assert_eq!(given_anything, Some(GUARD_UNANSWERED));
    assert!(!progress(&paths).contains(&dialling()));
}
