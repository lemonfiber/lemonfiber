use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;

use lemonfiber_fixtures::scratch::Scratch;

use super::{spoken, Preparing, PREPARE, PREPARED_WITHIN};
use crate::plugin::program::Throwaway;

fn script(root: &Path, body: &str, mode: u32) {
    let at = root.join(PREPARE);
    let _ = at.parent().map(std::fs::create_dir_all);
    let _ = std::fs::write(&at, format!("#!/bin/sh\nset -eu\n{body}\n"));
    let _ = std::fs::set_permissions(&at, std::fs::Permissions::from_mode(mode));
}

fn copy() -> Option<Throwaway> {
    Throwaway::of(None, &lemonfiber_fixtures::ports::Chance::cycling()).ok()
}

async fn prepared(root: &Path, copy: &Throwaway) -> Result<(), String> {
    Preparing {
        runner: &lemonfiber_adapters::Local,
        root,
    }
    .prepared(copy)
    .await
}

#[tokio::test]
async fn a_plugin_that_ships_no_script_has_its_copy_left_as_it_was() {
    let root = Scratch::new("prepare-none");
    let Some(copy) = copy() else {
        unreachable!("a throwaway copy is made");
    };
    let ran = Preparing {
        runner: &lemonfiber_fixtures::ports::Idle,
        root: &root,
    }
    .prepared(&copy)
    .await;
    assert_eq!(ran, Ok(()));
    assert_eq!(
        std::fs::read_dir(copy.path()).map(Iterator::count).ok(),
        Some(0)
    );
}

#[tokio::test]
async fn the_script_is_run_from_the_plugins_root_with_the_copy_as_its_one_argument() {
    let root = Scratch::new("prepare-runs");
    script(
        &root,
        "pwd -P > \"$1/where\"\necho \"$#\" > \"$1/count\"",
        0o755,
    );
    let Some(copy) = copy() else {
        unreachable!("a throwaway copy is made");
    };
    assert_eq!(prepared(&root, &copy).await, Ok(()));
    let read = |named: &str| {
        std::fs::read_to_string(copy.path().join(named))
            .ok()
            .map(|text| text.trim_end().to_owned())
    };
    let here = root.canonicalize().ok().map(|at| at.display().to_string());
    assert!(here.is_some());
    assert_eq!(read("where"), here);
    assert_eq!(read("count").as_deref(), Some("1"));
}

#[tokio::test]
async fn a_script_that_exits_non_zero_says_its_exit() {
    let root = Scratch::new("prepare-exits");
    script(&root, "exit 3", 0o755);
    let Some(copy) = copy() else {
        unreachable!("a throwaway copy is made");
    };
    assert_eq!(
        prepared(&root, &copy).await,
        Err("posture/prepare exited 3".to_owned())
    );
}

#[tokio::test]
async fn a_script_ended_by_a_signal_says_so() {
    let root = Scratch::new("prepare-killed");
    script(&root, "kill -9 $$", 0o755);
    let Some(copy) = copy() else {
        unreachable!("a throwaway copy is made");
    };
    assert_eq!(
        prepared(&root, &copy).await,
        Err("posture/prepare was ended by a signal".to_owned())
    );
}

#[tokio::test]
async fn a_script_that_cannot_be_run_says_why() {
    let root = Scratch::new("prepare-unrunnable");
    script(&root, "exit 0", 0o644);
    let Some(copy) = copy() else {
        unreachable!("a throwaway copy is made");
    };
    let ran = prepared(&root, &copy).await;
    assert!(
        ran.as_ref()
            .is_err_and(|why| why.starts_with("posture/prepare could not be run: ")),
        "{ran:?}"
    );
}

/// A runner whose program never ends.
struct Hung;

#[async_trait::async_trait]
impl crate::ports::process::Runner for Hung {
    async fn run(
        &self,
        _argv: &[String],
    ) -> Result<crate::ports::process::Output, crate::ports::process::Failure> {
        std::future::pending().await
    }
}

#[tokio::test(start_paused = true)]
async fn a_script_still_running_at_its_limit_is_stopped() {
    let root = Scratch::new("prepare-overruns");
    script(&root, "exit 0", 0o755);
    let Some(copy) = copy() else {
        unreachable!("a throwaway copy is made");
    };
    let ran = Preparing {
        runner: &Hung,
        root: &root,
    }
    .prepared(&copy)
    .await;
    assert_eq!(
        ran,
        Err(format!(
            "posture/prepare was still running at its limit of {} seconds, and was stopped",
            PREPARED_WITHIN.as_secs()
        ))
    );
}

#[tokio::test]
async fn a_script_reached_through_a_link_is_not_run() {
    let root = Scratch::new("prepare-linked");
    let elsewhere = Scratch::new("prepare-linked-target");
    script(&elsewhere, "touch \"$1/ran\"", 0o755);
    let _ = std::fs::create_dir_all(root.join("posture"));
    let _ = std::os::unix::fs::symlink(elsewhere.join(PREPARE), root.join(PREPARE));
    let Some(copy) = copy() else {
        unreachable!("a throwaway copy is made");
    };
    let ran = prepared(&root, &copy).await;
    assert!(
        ran.as_ref()
            .is_err_and(|why| why.ends_with("is a link or a device, so it was not run")),
        "{ran:?}"
    );
    assert!(!copy.path().join("ran").exists());
}

#[tokio::test]
async fn a_script_beneath_a_linked_directory_is_not_run() {
    let root = Scratch::new("prepare-linked-dir");
    let elsewhere = Scratch::new("prepare-linked-dir-target");
    script(&elsewhere, "touch \"$1/ran\"", 0o755);
    let _ = std::os::unix::fs::symlink(elsewhere.join("posture"), root.join("posture"));
    let Some(copy) = copy() else {
        unreachable!("a throwaway copy is made");
    };
    let ran = prepared(&root, &copy).await;
    assert!(
        ran.as_ref()
            .is_err_and(|why| why.ends_with("is a link or a device, so it was not run")),
        "{ran:?}"
    );
    assert!(!copy.path().join("ran").exists());
}

#[tokio::test]
async fn a_script_that_cannot_be_looked_at_is_not_run() {
    let root = Scratch::new("prepare-unseen");
    script(&root, "exit 0", 0o755);
    let posture = root.join("posture");
    let _ = std::fs::set_permissions(&posture, std::fs::Permissions::from_mode(0o000));
    let Some(copy) = copy() else {
        unreachable!("a throwaway copy is made");
    };
    let ran = prepared(&root, &copy).await;
    let _ = std::fs::set_permissions(&posture, std::fs::Permissions::from_mode(0o755));
    assert!(
        ran.as_ref()
            .is_err_and(|why| why.contains("could not be looked at, so it was not run: ")),
        "{ran:?}"
    );
}

#[test]
fn a_path_that_is_not_utf8_is_not_handed_on() {
    use std::os::unix::ffi::OsStrExt as _;
    let named = Path::new(std::ffi::OsStr::from_bytes(b"/plugin/\xff"));
    assert_eq!(
        spoken(named),
        Err("/plugin/\u{fffd} is not UTF-8, so posture/prepare was not run".to_owned())
    );
}
