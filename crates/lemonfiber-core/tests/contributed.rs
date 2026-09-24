//! A plugin's contributed rows, driven through the diagnosis they are part of.
//!
//! From here rather than from a `#[cfg(test)]` module because the claim is about the
//! whole path: a row is read off the register an install wrote, built into the same
//! list the bundled checks are built into, narrowed by the same narrowing and summed
//! into the same overall. A test that called the builder directly would prove the
//! builder and leave the wiring — which is the thing that was missing — unproven.
//!
//! The stack is the real one, read from the repository's own embedded copy. What is
//! installed is a register written here, which is the only thing a diagnosis a month
//! after an install has to go on.

mod common;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use common::stack::project;

use lemonfiber_core::app::{diagnose, Ctx};
use lemonfiber_core::config::Settings;
use lemonfiber_core::doctor::{Narrowing, Verdict};
use lemonfiber_core::platform::Environment;
use lemonfiber_core::ports::docker::{Health, Lifecycle};
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::support::Reporting;

/// The register an install writes, holding one plugin with one contributed row.
///
/// Written as the file rather than built from a manifest, which is the point: the
/// author's directory is gone by the time a diagnosis runs, and this record is the
/// whole of what the machine still has.
const INSTALLED: &str = r#"{
  "installed": [
    {
      "plugin": "komga",
      "version": "1.2.0",
      "services": [
        {
          "service": "komga",
          "image": "docker.io/gotson/komga",
          "digest": "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945",
          "tag": "1.11.0",
          "config_path": "/config",
          "takes_data": true,
          "reached": { "tier": "household", "port": 25600, "hostname": "comics", "group": "Library" }
        }
      ],
      "contributions": [
        {
          "at": "doctor.check",
          "id": "komga:libraries",
          "title": "Komga libraries",
          "category": "services",
          "service": "komga",
          "request": { "method": "GET", "path": "/api/v1/libraries" },
          "expect": { "status": 200 },
          "why": "a library server with no library is not serving anything"
        },
        {
          "at": "doctor.remedy",
          "id": "komga:libraries:add",
          "for": "komga:libraries",
          "why": "nothing has been added to it yet",
          "action": "Add a library in Komga's settings"
        }
      ]
    }
  ]
}"#;

/// The same register with nothing in it, which is what a machine that has installed
/// nothing holds — and what one a plugin was removed from holds again.
const NONE: &str = r#"{ "installed": [] }"#;

/// Where this case keeps its register, beside an environment file the way an install
/// does.
fn kept(name: &str, register: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "lemonfiber-contributed-{}-{name}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    let env = dir.join(".env");
    assert!(std::fs::write(&env, "").is_ok(), "the environment file");
    assert!(
        std::fs::write(dir.join("plugins.json"), register).is_ok(),
        "the register"
    );
    env
}

/// A machine with that register, and a transport answering as the plugin's service.
fn machine(env: &Path, http: Arc<dyn lemonfiber_core::ports::Http>) -> Ctx {
    Ctx::new(
        Arc::new(lemonfiber_fixtures::ports::Idle),
        Arc::new(Reporting::holding(&[], Lifecycle::Exited, Health::None)),
        lemonfiber_fixtures::ports::Stopped::today(),
        lemonfiber_adapters::live(),
        Source::External(project()),
        Settings {
            env_file: Some(env.to_path_buf()),
            ..Settings::default()
        },
        Environment::MacOs,
    )
    .with_http(http)
}

/// What a run narrowed to the plugin's own row came to.
async fn asking(ctx: &Ctx, check: &str) -> Option<Verdict> {
    diagnose(ctx, &Narrowing::Check(check.to_owned()), false)
        .await
        .ok()
        .and_then(|report| {
            report
                .findings
                .into_iter()
                .find(|finding| finding.check == check)
                .map(|finding| finding.verdict)
        })
}

/// The whole of what wiring the register in buys: a row an operator's plugin declared
/// is run by the engine the bundled rows are run by, is reached by its own published
/// port, and is narrowed to by the very id its finding carries.
#[tokio::test]
async fn a_row_a_plugin_declared_runs_where_the_bundled_ones_run() {
    let env = kept("holding", INSTALLED);
    let ctx = machine(
        &env,
        Fake::by_path(vec![("/api/v1/libraries", Answer::reply(200, "[]"))]),
    );

    assert!(
        matches!(
            asking(&ctx, "komga:libraries").await,
            Some(Verdict::Pass { .. })
        ),
        "the plugin's own row answered, on the port its install published"
    );
    assert_eq!(
        origin_of(&ctx, "komga:libraries").await,
        Some(lemonfiber_core::origin::Origin::Plugin {
            named: "komga".to_owned()
        }),
        "and it says whose row it is, beside it rather than in its punctuation"
    );
    assert_eq!(
        origin_of(&ctx, "environment.compose").await,
        Some(lemonfiber_core::origin::Origin::Bundled),
        "while one of this build's own says that"
    );
}

/// Whose check the finding with this id says it is.
async fn origin_of(ctx: &Ctx, check: &str) -> Option<lemonfiber_core::origin::Origin> {
    diagnose(ctx, &Narrowing::Suite, false)
        .await
        .ok()
        .and_then(|report| {
            report
                .findings
                .into_iter()
                .find(|finding| finding.check == check)
                .map(|finding| finding.origin)
        })
}

/// A row whose service does not answer is unrun and says so, naming the plugin. It is
/// not passed, and it is not left out — the two ways a register could quietly check
/// less than it says it does.
#[tokio::test]
async fn a_row_whose_service_is_not_answering_is_unrun_and_names_the_plugin() {
    let env = kept("silent", INSTALLED);
    let ctx = machine(&env, Fake::silent());

    let verdict = asking(&ctx, "komga:libraries").await;
    assert!(
        matches!(&verdict, Some(Verdict::Unverified { reason, .. }) if reason.contains("komga")),
        "it says which plugin's row could not be run: {verdict:?}"
    );
}

/// A machine with nothing installed answers exactly as a build that never saw a plugin
/// does: the row is not there to be narrowed to, and the run says so rather than
/// reporting a healthy silence.
#[tokio::test]
async fn a_machine_with_no_plugin_installed_has_no_row_to_ask_for() {
    let env = kept("empty", NONE);
    let ctx = machine(&env, Fake::silent());

    assert!(
        diagnose(&ctx, &Narrowing::Check("komga:libraries".to_owned()), false)
            .await
            .is_err(),
        "a name nothing reports is refused rather than answered as nothing wrong"
    );
}

/// A register that is there and will not parse refuses the diagnosis rather than being
/// read past. Carrying on would report a clean bill of health with a stranger's rows
/// silently missing from it, which is the one answer somebody would act on.
#[tokio::test]
async fn a_register_that_will_not_parse_refuses_the_diagnosis() {
    let env = kept("damaged", "{ not a register");
    let ctx = machine(&env, Fake::silent());

    assert_eq!(
        diagnose(&ctx, &Narrowing::Suite, false)
            .await
            .err()
            .map(|problem| problem.code.to_string()),
        Some("PLUGIN-4".to_owned()),
        "it says the register could not be read, rather than what it holds"
    );
}
