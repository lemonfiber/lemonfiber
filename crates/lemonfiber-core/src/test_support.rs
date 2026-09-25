//! The fixtures that name a type from this crate.
//!
//! Everything a test stands in for lives in `lemonfiber-fixtures`, and the context a test
//! drives a command through lives in `lemonfiber-testing`; both are reached from here.
//! What is written here names a type of this crate that neither of those can.

pub(crate) use lemonfiber_fixtures::support::*;

// The same file the testing crate is built from, compiled here as a module of this
// crate: the core's own tests cannot depend on a crate that depends on the core
// without the core being built twice, and a context from the other copy would not be
// a context this copy's `dispatch` accepts.
#[path = "../../lemonfiber-testing/src/context.rs"]
pub mod context;

pub(crate) use context::{a_context, nowhere, repository_stack as stack};

/// A journal line for a setting written over nothing — a fresh file, so the prior value
/// is absent.
///
/// Three test modules each built this, because the journal's shape is what they assert
/// against and stating it independently is the point: a test that echoed how apply
/// computes a change would pass whatever apply did. Stating it once does not weaken that
/// — what matters is that the expectation is written out rather than derived, not that
/// it is written out three times.
pub(crate) fn a_fresh_write(key: &str, value: &str) -> crate::journal::Change {
    crate::journal::Change {
        at: "t".to_owned(),
        operation: "apply".to_owned(),
        target: ".env".to_owned(),
        kind: crate::journal::Kind::Set {
            key: key.to_owned(),
            previous: None,
            current: value.to_owned(),
        },
    }
}

/// The same file with nothing recorded in it, for a test about anything but the
/// password.
///
/// Built rather than written at the call site: an empty string literal beside a password
/// key reads to a secret scanner as a hard-coded credential, and a fixture that has to
/// be argued about every time it is scanned is worse than one that says what it means.
pub(crate) fn env_without_password(name: &str) -> std::path::PathBuf {
    recorded(name, None)
}

/// A private env file recording qBittorrent's password, at a scratch path unique to
/// the test so concurrent tests do not share one.
///
/// Here rather than beside any one test because more than one needs a client that can
/// be logged into: the dashboard, to fill its transfers panel, and a teardown, to find
/// out what stopping would interrupt. It names `config::store`, which is why it is on
/// this side of the fixtures boundary.
pub(crate) fn env_at(name: &str, password: &str) -> std::path::PathBuf {
    recorded(name, Some(password))
}

/// The file itself, with a password where there is one.
///
/// The absence is `None` rather than an empty string, so a test that wants no password
/// says so instead of writing a blank one — which reads to a secret scanner as a
/// hard-coded credential and to a reader as a password that happens to be empty.
fn recorded(name: &str, password: Option<&str>) -> std::path::PathBuf {
    let dir = lemonfiber_fixtures::scratch::Scratch::named(&format!("env-{name}")).kept();
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.join(".env");
    assert!(
        crate::config::store::set(
            &path,
            crate::config::QBITTORRENT_PASSWORD_KEY,
            password.unwrap_or_default(),
        )
        .is_ok(),
        "the scratch env file is written"
    );
    path
}

mod tests;
