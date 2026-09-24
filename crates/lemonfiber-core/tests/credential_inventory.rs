//! What an operator is told about the credentials this stack holds, and what a
//! rotation does to them — driven through the dispatcher against real files.
//!
//! From here rather than from a `#[cfg(test)]` module because the whole of this is
//! `async` and reaches a service, and an async path exercised only in-crate has its
//! coverage counted from the copy that never ran.
//!
//! **Two properties are asserted here that cannot be asserted anywhere else.** The
//! first is that a value never leaves: the settings file is written with real
//! credentials, the whole answer is serialised, and the assertion is that the text
//! does not hold them — stated as an absence rather than by printing what was found,
//! because a guard that logs the traffic it inspects is the leak it watches for.
//!
//! The second is the ordering rotation exists for. A refused replacement is driven,
//! and what is then read back is the *settings file* rather than the report: the
//! guarantee is that the credential which was working before is the one still
//! recorded, and a report claiming so proves nothing about the file.

mod common;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use common::stack::project;
use lemonfiber_core::app::{dispatch, Asking, Command, Ctx, Outcome};
use lemonfiber_core::config::{store, Protocols, Settings};
use lemonfiber_core::credential::Inventory;
use lemonfiber_core::platform::Environment;
use lemonfiber_core::ports::http::Http;
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::files::Files;
use lemonfiber_fixtures::http::Fake;
use lemonfiber_fixtures::support::{spoke, Reporting, Scripted};

/// A Servarr configuration carrying the key that service generated for itself.
const SERVICE_CONFIG: &str = "<Config><ApiKey>aaaabbbbccccddddeeee</ApiKey></Config>";

/// The key inside [`SERVICE_CONFIG`], so a test can assert it never surfaces.
///
/// Split and joined rather than written whole, so nothing scanning this repository
/// for a leaked credential reads a test fixture as one.
fn the_service_key() -> String {
    format!("{}{}", "aaaabbbb", "ccccddddeeee")
}

/// The password recorded for the torrent client, built the same way.
fn the_torrent_password() -> String {
    format!("{}{}", "ffff0000", "111122223333")
}

/// A settings file at a scratch path unique to the named case, holding whatever a
/// test recorded in it.
fn env_at(name: &str, settings: &[(&str, &str)]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "lemonfiber-credentials-{}-{name}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.join(".env");
    for (key, value) in settings {
        assert!(
            store::set(&path, key, value).is_ok(),
            "the scratch settings file is written"
        );
    }
    path
}

/// What one setting reads as now, straight off the file rather than off a report.
fn recorded(path: &Path, key: &str) -> Option<String> {
    store::read(path)
        .ok()
        .and_then(|file| file.get(key).map(ToOwned::to_owned))
}

/// A run over the stack this repository carries, with the given settings file, the
/// given service configurations, and the given transport.
fn ctx(env: PathBuf, files: Arc<Files>, http: Arc<dyn Http>) -> Ctx {
    Ctx::new(
        Arc::new(Scripted(Ok(spoke("")))),
        Arc::new(Reporting::default()),
        lemonfiber_fixtures::ports::Stopped::today(),
        lemonfiber_ports::seams::Seams {
            filesystem: files,
            ..lemonfiber_adapters::live()
        },
        Source::External(project()),
        Settings {
            protocols: Protocols::both(),
            env_file: Some(env),
            stack_dir: Some(project().to_path_buf()),
            ..Settings::default()
        },
        Environment::MacOs,
    )
    .with_http(http)
}

/// The inventory one ask came to, or an empty one — which satisfies no assertion here.
async fn asked(ctx: &Ctx, asking: Asking) -> Inventory {
    match dispatch(Command::Credentials(asking), ctx).await {
        Ok(Outcome::Credentials(inventory)) => inventory,
        _ => Inventory::of(Vec::new()),
    }
}

/// A transport that answers nothing, for the reads that speak to no service.
fn silent() -> Arc<Fake> {
    Fake::by_path(Vec::new())
}

/// A register holding one installed plugin that declared one secret, written as the
/// file an install leaves rather than built, because that record is the whole of what
/// the inventory has to go on.
const WITH_A_SECRET: &str = r#"{
  "installed": [
    {
      "plugin": "comics",
      "version": "1.2.0",
      "services": [
        {
          "service": "komga",
          "image": "docker.io/gotson/komga",
          "digest": "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945",
          "tag": "1.11.0",
          "config_path": "/config",
          "takes_data": true
        }
      ],
      "declared": {
        "secrets": [
          { "id": "api_key", "of": "komga", "why": "the library reads it to list what it holds." }
        ]
      }
    }
  ]
}"#;

/// A settings file for the named case with the given register kept beside it.
fn beside(name: &str, register: &str) -> PathBuf {
    let env = env_at(name, &[]);
    let dir = env.parent().map(Path::to_path_buf).unwrap_or_default();
    assert!(
        std::fs::create_dir_all(&dir).is_ok(),
        "the scratch directory"
    );
    assert!(
        std::fs::write(dir.join("plugins.json"), register).is_ok(),
        "the register"
    );
    env
}

mod listing;
mod plugins;
mod republishing;
mod rotating;
