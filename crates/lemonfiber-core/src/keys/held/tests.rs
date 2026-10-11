use std::path::PathBuf;
use std::time::Duration;

use lemonfiber_fixtures::scratch::Scratch;

use super::{beside, Held};
use crate::keys::Unkept;

fn record(scratch: &Scratch) -> PathBuf {
    scratch.path().join("config").join("keys.json")
}

#[tokio::test]
async fn a_second_hold_waits_until_the_first_is_let_go() {
    let scratch = Scratch::new("held-waits");
    let path = record(&scratch);
    let first = Held::taken(&path).await;
    assert!(first.is_ok(), "{first:?}");
    let again = path.clone();
    let second = tokio::spawn(async move { Held::taken(&again).await.is_ok() });
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(!second.is_finished());
    drop(first);
    assert!(second.await.unwrap_or_default());
}

#[cfg(unix)]
#[tokio::test]
async fn the_hold_is_its_owners_alone() {
    use std::os::unix::fs::PermissionsExt as _;
    let scratch = Scratch::new("held-owned");
    let path = record(&scratch);
    let held = Held::taken(&path).await;
    assert!(held.is_ok(), "{held:?}");
    let mode = std::fs::metadata(beside(&path)).map(|meta| meta.permissions().mode() & 0o777);
    assert_eq!(mode.ok(), Some(0o600));
}

#[tokio::test]
async fn a_hold_that_cannot_be_opened_is_refused_as_unwritten() {
    let scratch = Scratch::new("held-blocked");
    let path = record(&scratch);
    assert!(std::fs::create_dir_all(beside(&path)).is_ok());
    let refused = Held::taken(&path).await;
    assert!(matches!(refused, Err(Unkept::NotWritten(_))), "{refused:?}");
}

#[cfg(unix)]
#[tokio::test]
async fn a_link_at_the_holds_name_is_not_followed() {
    let scratch = Scratch::new("held-linked");
    let path = record(&scratch);
    let elsewhere = path.with_file_name("elsewhere");
    assert!(std::fs::create_dir_all(scratch.path().join("config")).is_ok());
    assert!(std::fs::write(&elsewhere, "untouched").is_ok());
    assert!(std::os::unix::fs::symlink(&elsewhere, beside(&path)).is_ok());
    let refused = Held::taken(&path).await;
    assert!(refused.is_err(), "{refused:?}");
    assert_eq!(
        std::fs::read_to_string(&elsewhere).ok().as_deref(),
        Some("untouched")
    );
}
