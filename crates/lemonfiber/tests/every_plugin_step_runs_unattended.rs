//! Every step of taking a plugin on, run with nobody at the terminal.
//!
//! Fetching a manifest from where it lives, validating it, proving what it claims and
//! rehearsing its install are each a plain subcommand, and the answer each gives is an
//! exit status a script can branch on. Asked of the command rather than of a function:
//! whether something waits for a person is decided by the process, and only a real
//! process with nothing on its input can show that it does not.
//!
//! Every run has its input closed, its environment emptied and a machine of its own, so
//! a step that stopped to ask would fail here on end-of-input rather than pass on
//! somebody's terminal, and no answer depends on whose machine it ran on.

use std::path::{Path, PathBuf};
use std::process::Stdio;

/// The binary Cargo built for this test.
const BINARY: &str = env!("CARGO_BIN_EXE_lemonfiber");

/// A plugin that claims a core capability, with the two recordings that answer it.
const MANIFEST: &str = r#"
schema_version = 1

[plugin]
id          = "kavita"
name        = "Kavita"
version     = "1.0.0"
description = "Reads comics in a browser"
without_it  = "Comics stay folders of images"
upstream    = "https://example.invalid"
license     = "MIT"
forms       = ["library"]

[[service]]
id          = "kavita"
name        = "Kavita"
image       = "example.invalid/kavita"
digest      = "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"
tag         = "1.0.0"
port        = 5000
bind        = "lan"
criticality = "enhancing"
provides    = ["media.serve"]

[[claim]]
capability = "media.serve"

[[claim.probe]]
id      = "guarded"
request = { method = "GET", path = "/api/series" }
expect  = { status = 401 }
fixture = "fixtures/guarded.json"

[[claim.probe]]
id      = "catalogue"
request = { method = "GET", path = "/api/series" }
expect  = { status = 200, json_has_keys = ["content"] }
fixture = "fixtures/catalogue.json"
"#;

/// One recorded answer, with whatever status and body it recorded.
fn recording(status: u16, body: &str) -> String {
    format!(
        r#"{{"recorded_from": "example.invalid/kavita@sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945",
            "note": "recorded for this test",
            "request": {{"method": "GET", "path": "/api/series"}},
            "response": {{"status": {status}, "json": {body}}}}}"#
    )
}

/// A machine of its own, with a plugin source on it holding this manifest, and the
/// catalogue recorded as answering with this status.
fn machine(named: &str, manifest: &str, recorded: u16) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "lemonfiber-unattended-{}-{named}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    for dir in ["source/fixtures", "stack", "config", "data", "empty"] {
        let _ = std::fs::create_dir_all(root.join(dir));
    }
    let _ = std::fs::write(root.join("source/plugin.toml"), manifest);
    let _ = std::fs::write(
        root.join("source/fixtures/guarded.json"),
        recording(401, "null"),
    );
    let _ = std::fs::write(
        root.join("source/fixtures/catalogue.json"),
        recording(recorded, r#"{"content": []}"#),
    );
    root
}

/// Run one step on that machine with nobody there, and say how it ended.
///
/// `None` is a run that did not end with a status at all — killed, or crashed — which
/// is never a meaningful answer and is failed as one.
fn unattended(root: &Path, argv: &[&str]) -> Option<i32> {
    std::process::Command::new(BINARY)
        .args(
            ["--stack-dir", "--config-dir", "--data-dir"]
                .iter()
                .zip(["stack", "config", "data"])
                .flat_map(|(flag, dir)| [(*flag).to_owned(), root.join(dir).display().to_string()]),
        )
        .args(argv)
        .env_clear()
        .env("PATH", "")
        .env("HOME", root)
        .current_dir(root)
        .stdin(Stdio::null())
        .output()
        .ok()
        .and_then(|ran| ran.status.code())
}

/// Every file on the machine outside the plugin's own source, which is what a step
/// that promises to write nothing has to leave empty.
fn written(root: &Path) -> Vec<PathBuf> {
    ["stack", "config", "data"]
        .iter()
        .flat_map(|dir| {
            std::fs::read_dir(root.join(dir))
                .into_iter()
                .flatten()
                .flatten()
        })
        .map(|entry| entry.path())
        .collect()
}

/// Each step answers, with a status that says which way it went, and none of them
/// waits for anybody.
///
/// Both answers for each: a status that could only ever be zero, or only ever not,
/// would be a status nothing could branch on.
#[test]
fn each_step_answers_with_a_status_and_nobody_at_the_terminal() {
    let refused = format!("{MANIFEST}\n[requires]\ncapabilities = [\"service.add\"]\n");
    // One run of `claims` fetches, validates and proves at once, so it holds for all
    // three; what tells the three apart is which way each one fails.
    let steps: [(&str, &str, u16, &[&str], bool); 6] = [
        (
            "fetching, validating and proving",
            MANIFEST,
            200,
            &["plugin", "claims", "source"],
            true,
        ),
        (
            "fetching from where nothing is",
            MANIFEST,
            200,
            &["plugin", "claims", "empty"],
            false,
        ),
        (
            "validating what the build refuses",
            &refused,
            200,
            &["plugin", "claims", "source"],
            false,
        ),
        (
            "proving what its recording refutes",
            MANIFEST,
            404,
            &["plugin", "claims", "source"],
            false,
        ),
        (
            "rehearsing",
            MANIFEST,
            200,
            &["plugin", "install", "--dry-run", "source"],
            true,
        ),
        (
            "rehearsing what the build refuses",
            &refused,
            200,
            &["plugin", "install", "--dry-run", "source"],
            false,
        ),
    ];

    for (step, manifest, recorded, argv, holds) in steps {
        let root = machine(&step.replace(' ', "-"), manifest, recorded);
        let ended = unattended(&root, argv);
        assert!(
            ended.is_some(),
            "{step} did not end with a status: {ended:?}"
        );
        assert_eq!(
            ended == Some(0),
            holds,
            "{step} ended with {ended:?}, where a script would branch the other way"
        );
    }
}

/// A rehearsal is the one step here that describes a write, so it is the one worth
/// checking wrote nothing: an install rehearsed from a script is a script that was
/// promised it could look first.
#[test]
fn a_rehearsed_install_leaves_the_machine_as_it_found_it() {
    let root = machine("rehearsed", MANIFEST, 200);

    assert_eq!(
        unattended(&root, &["plugin", "install", "--dry-run", "source"]),
        Some(0)
    );
    assert_eq!(written(&root), Vec::<PathBuf>::new(), "nothing was written");
}

/// A plugin with nothing to prove and nothing to claim, which is what an install can
/// finish against with no instance of the software anywhere: its proofs hold because it
/// declares none, and the stack's own checks are held against themselves.
fn plain() -> String {
    MANIFEST
        .split("[[claim]]")
        .next()
        .unwrap_or_default()
        .replace("provides    = [\"media.serve\"]\n", "")
}

/// A machine with the stack this binary carries and a container engine that agrees to
/// everything, so the verbs that act can be seen acting from start to finish.
///
/// The engine is a script rather than a real one, and that is enough for the question
/// here — whether each verb reaches its end with nobody present and says how it went —
/// because what it would have started is proved elsewhere, by the verbs' own tests.
fn engine_agreeing(named: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt as _;

    let root = std::env::temp_dir().join(format!(
        "lemonfiber-unattended-verbs-{}-{named}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    for dir in ["bin", "source", "next", "config", "data"] {
        let _ = std::fs::create_dir_all(root.join(dir));
    }
    let _ = std::fs::write(root.join("bin/docker"), "#!/bin/sh\nexit 0\n");
    let _ = std::fs::set_permissions(
        root.join("bin/docker"),
        std::fs::Permissions::from_mode(0o755),
    );
    let _ = std::fs::write(root.join("source/plugin.toml"), plain());
    let _ = std::fs::write(
        root.join("next/plugin.toml"),
        plain().replace("version     = \"1.0.0\"", "version     = \"1.1.0\""),
    );
    root
}

/// One verb on that machine, with nobody there, and how it ended.
fn acting(root: &Path, argv: &[&str]) -> Option<i32> {
    std::process::Command::new(BINARY)
        .arg("--config-dir")
        .arg(root.join("config"))
        .arg("--data-dir")
        .arg(root.join("data"))
        .args(argv)
        .env_clear()
        .env("PATH", root.join("bin"))
        .env("HOME", root)
        .current_dir(root)
        .stdin(Stdio::null())
        .output()
        .ok()
        .and_then(|ran| ran.status.code())
}

/// Installing, rehearsing, updating and removing each run to their end with nobody at
/// the terminal, and each ends with a status a script can branch on — shown both ways,
/// in the order an operator would meet them.
#[test]
fn each_verb_runs_to_its_end_with_nobody_at_the_terminal() {
    let root = engine_agreeing("lifecycle");
    let steps: [(&str, &[&str], bool); 9] = [
        (
            "removing what is not installed",
            &["plugin", "remove", "kavita"],
            false,
        ),
        (
            "updating what is not installed",
            &["plugin", "update", "next"],
            false,
        ),
        (
            "rehearsing the install",
            &["plugin", "install", "--dry-run", "source"],
            true,
        ),
        ("installing", &["plugin", "install", "source"], true),
        (
            "installing it again",
            &["plugin", "install", "source"],
            false,
        ),
        (
            "rehearsing the update",
            &["plugin", "update", "--dry-run", "next"],
            true,
        ),
        ("updating", &["plugin", "update", "next"], true),
        (
            "rehearsing the removal",
            &["plugin", "remove", "--dry-run", "kavita"],
            true,
        ),
        ("removing", &["plugin", "remove", "kavita"], true),
    ];
    for (step, argv, holds) in steps {
        let ended = acting(&root, argv);
        assert!(
            ended.is_some(),
            "{step} did not end with a status: {ended:?}"
        );
        assert_eq!(
            ended == Some(0),
            holds,
            "{step} ended with {ended:?}, where a script would branch the other way"
        );
    }
}
