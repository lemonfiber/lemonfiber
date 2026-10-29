use std::sync::Arc;
use std::time::Duration;

use lemonfiber_fixtures::proving::Proving;
use lemonfiber_fixtures::scratch::Scratch;

use super::{declared, Running};
use crate::doctor::{Category, Verdict};
use crate::plugin::program::{starting_digest, Operator, Starting};
use crate::plugin::Installed;
use crate::ports::program::Unran;

/// The plugin, installed from a source whose starting files are kept under `kept`.
fn installed(kept: &Scratch) -> Option<Installed> {
    let _ = std::fs::create_dir_all(kept.join("quality/config/qualitysync"));
    let _ = std::fs::write(kept.join("quality/config/qualitysync/recyclarr.yml"), "x");
    let manifest =
        lemonfiber_plugin::Manifest::from_toml(crate::plugin::program::tests::RUNNING).ok()?;
    let root = kept.join("quality");
    Some(Installed::of(&manifest).starting(&|service| {
        starting_digest(Some(Starting {
            root: &root,
            service,
        }))
        .ok()
    }))
}

fn running(program: Arc<Proving>, kept: &Scratch, now: &str, uid: &str) -> Running {
    Running {
        program,
        operator: Operator {
            zone: "Europe/Amsterdam".to_owned(),
            ids: (uid.to_owned(), "1000".to_owned()),
        },
        starting: Some(kept.path().to_path_buf()),
        kept: Some(kept.join("run-proofs.json")),
        now: now.to_owned(),
        random: Arc::new(lemonfiber_fixtures::ports::Chance::cycling()),
        confined: Arc::new(lemonfiber_adapters::Disk),
    }
}

async fn verdicts(installed: &Installed, running: &Running) -> Vec<Verdict> {
    let mut found = Vec::new();
    for check in declared(installed, running) {
        found.extend(check.run().await.into_iter().map(|finding| finding.verdict));
    }
    found
}

fn note(verdict: Option<&Verdict>) -> String {
    match verdict {
        Some(Verdict::Pass { note }) => note.clone().unwrap_or_default(),
        Some(Verdict::Fail(problem)) => problem.summary.clone(),
        Some(Verdict::Unverified { reason, .. }) => reason.clone(),
        _ => String::new(),
    }
}

#[tokio::test]
async fn a_run_proof_is_run_once_and_its_verdict_kept_with_when_it_was_reached() {
    let kept = Scratch::new("proved-once");
    let Some(installed) = installed(&kept) else {
        return;
    };
    let first = Proving::exiting(0, "v8.7.3 (Branch)");
    let said = verdicts(
        &installed,
        &running(Arc::clone(&first), &kept, "100", "1000"),
    )
    .await;
    assert_eq!(first.asked().len(), 1);
    assert!(note(said.first()).contains("reached at 100"), "{said:?}");

    let again = Proving::exiting(1, "broken");
    let said = verdicts(
        &installed,
        &running(Arc::clone(&again), &kept, "200", "1000"),
    )
    .await;
    assert!(again.asked().is_empty(), "a kept verdict is not run again");
    assert!(
        matches!(said.first(), Some(Verdict::Pass { note: Some(note) }) if note.contains("reached at 100")),
        "{said:?}"
    );
}

/// Whether a run proof runs again once its baseline verdict is kept and `moved` changes
/// one thing it depends on.
async fn runs_again(named: &str, moved: impl Fn(&mut Installed, &mut String)) -> bool {
    let kept = Scratch::new(named);
    let Some(installed) = installed(&kept) else {
        return false;
    };
    let _ = verdicts(
        &installed,
        &running(Proving::exiting(0, "v8.7.3 ("), &kept, "100", "1000"),
    )
    .await;
    let mut changed = installed.clone();
    let mut uid = "1000".to_owned();
    moved(&mut changed, &mut uid);
    let again = Proving::exiting(0, "v8.7.3 (");
    let _ = verdicts(&changed, &running(Arc::clone(&again), &kept, "200", &uid)).await;
    again.asked().len() == 1
}

#[tokio::test]
async fn a_run_proof_runs_again_once_what_it_depends_on_moves() {
    assert!(
        !runs_again("proved-still", |_, _| {}).await,
        "nothing moved"
    );
    assert!(
        runs_again("proved-user", |_, uid| *uid = "1001".to_owned()).await,
        "another user is another run"
    );
    assert!(
        runs_again("proved-image", |installed, _| {
            if let Some(placed) = installed.services.first_mut() {
                placed.digest = format!("sha256:{}", "1".repeat(64));
            }
        })
        .await,
        "another image is another run"
    );
    assert!(
        runs_again("proved-posture", |installed, _| {
            if let Some(placed) = installed.services.first_mut() {
                placed.fields.posture = lemonfiber_manifest::container::Posture::Standard;
            }
        })
        .await,
        "another posture is another run"
    );
    assert!(
        runs_again("proved-args", |installed, _| {
            if let Some(proof) = installed.run_proofs.first_mut() {
                proof.run.args = vec!["--help".to_owned()];
            }
        })
        .await,
        "other arguments are another run"
    );
}

#[tokio::test]
async fn a_kept_verdict_is_kept_against_everything_its_run_depended_on() {
    let kept = Scratch::new("proved-key");
    let Some(installed) = installed(&kept) else {
        return;
    };
    let _ = verdicts(
        &installed,
        &running(Proving::exiting(0, "v8.7.3 ("), &kept, "100", "1000"),
    )
    .await;
    let held = super::kept::read(Some(&kept.join("run-proofs.json")));
    let digest = installed
        .services
        .first()
        .and_then(|placed| placed.starting.clone())
        .unwrap_or_default();
    assert!(digest.starts_with("sha256:"));
    assert_eq!(
        held.iter().map(|one| one.key.clone()).collect::<Vec<_>>(),
        vec![super::Key {
            image: "ghcr.io/recyclarr/recyclarr@sha256:9c6c74d52cfc6f740a5a3624aa83867e171bb70b90453ddb7a4048b8c80445f9".to_owned(),
            args: vec!["--version".to_owned()],
            posture: lemonfiber_manifest::container::Posture::Confined,
            user: "Uid 1000:1000".to_owned(),
            starting: digest,
            release: crate::stack::RUNNING.to_owned(),
        }]
    );
}

#[tokio::test]
async fn starting_files_changed_since_the_install_leave_it_unverified_and_unrun() {
    let kept = Scratch::new("proved-changed");
    let Some(installed) = installed(&kept) else {
        return;
    };
    let _ = std::fs::write(
        kept.join("quality/config/qualitysync/recyclarr.yml"),
        "edited",
    );
    let program = Proving::exiting(0, "v8.7.3 (");
    let said = verdicts(
        &installed,
        &running(Arc::clone(&program), &kept, "100", "1000"),
    )
    .await;
    assert!(program.asked().is_empty());
    assert!(
        matches!(said.first(), Some(Verdict::Unverified { reason, .. }) if reason.contains("not as they were")),
        "{said:?}"
    );
}

#[tokio::test]
async fn a_failed_run_is_kept_and_reported_as_the_plugins_failure() {
    let kept = Scratch::new("proved-failed");
    let Some(installed) = installed(&kept) else {
        return;
    };
    let said = verdicts(
        &installed,
        &running(Proving::exiting(2, "nope"), &kept, "100", "1000"),
    )
    .await;
    assert!(
        matches!(said.first(), Some(Verdict::Fail(problem))
            if problem.code == crate::error::codes::plugin::PROOF_RUN_FAILED
                && problem.detail.as_deref().is_some_and(|detail| detail.contains("exited 2"))),
        "{said:?}"
    );
    let again = Proving::exiting(0, "v8.7.3 (");
    let said = verdicts(
        &installed,
        &running(Arc::clone(&again), &kept, "200", "1000"),
    )
    .await;
    assert!(again.asked().is_empty());
    assert!(
        note(said.first()).contains("did not hold, reached at 100"),
        "{said:?}"
    );
}

#[tokio::test]
async fn a_run_that_could_not_be_made_is_unverified_and_not_kept() {
    let kept = Scratch::new("proved-unmade");
    let Some(installed) = installed(&kept) else {
        return;
    };
    let unmade = Proving::answering(Err(Unran::NoImage {
        image: "x".to_owned(),
    }));
    let said = verdicts(&installed, &running(unmade, &kept, "100", "1000")).await;
    assert!(
        matches!(said.first(), Some(Verdict::Unverified { reason, .. }) if reason.contains("pulls no image")),
        "{said:?}"
    );
    let later = Proving::exiting(0, "v8.7.3 (");
    let _ = verdicts(
        &installed,
        &running(Arc::clone(&later), &kept, "200", "1000"),
    )
    .await;
    assert_eq!(later.asked().len(), 1, "nothing was kept, so it runs");
}

#[test]
fn a_run_proofs_check_is_the_plugins_and_bounded_by_its_own_limit() {
    let kept = Scratch::new("proved-shape");
    let Some(installed) = installed(&kept) else {
        return;
    };
    let checks = declared(
        &installed,
        &running(Proving::unreachable(), &kept, "1", "1"),
    );
    assert_eq!(checks.len(), 1);
    let Some(check) = checks.first() else {
        return;
    };
    assert_eq!(check.category(), Category::Services);
    assert_eq!(check.budget(), Duration::from_secs(45 + 30));
    let reported = check.reports();
    assert_eq!(
        reported
            .as_ref()
            .map(|one| (one.check.as_str(), one.service.as_deref())),
        Some(("quality:recyclarr.runs", Some("qualitysync")))
    );
    assert_eq!(
        reported.map(|one| one.origin),
        Some(crate::origin::Origin::Plugin {
            named: "quality".to_owned()
        })
    );
    let mut lost = installed;
    lost.services.clear();
    assert!(declared(&lost, &running(Proving::unreachable(), &kept, "1", "1")).is_empty());
}

#[tokio::test]
async fn a_run_proof_runs_again_for_new_starting_files_and_for_another_release() {
    let kept = Scratch::new("proved-reinstalled");
    let Some(installed) = installed(&kept) else {
        return;
    };
    let _ = verdicts(
        &installed,
        &running(Proving::exiting(0, "v8.7.3 ("), &kept, "100", "1000"),
    )
    .await;

    let _ = std::fs::write(
        kept.join("quality/config/qualitysync/recyclarr.yml"),
        "updated",
    );
    let root = kept.join("quality");
    let updated = installed.clone().starting(&|service| {
        starting_digest(Some(Starting {
            root: &root,
            service,
        }))
        .ok()
    });
    let new_files = Proving::exiting(0, "v8.7.3 (");
    let _ = verdicts(
        &updated,
        &running(Arc::clone(&new_files), &kept, "200", "1000"),
    )
    .await;
    assert_eq!(
        new_files.asked().len(),
        1,
        "other starting files are another run"
    );

    let file = kept.join("run-proofs.json");
    let held: Vec<super::Kept> = super::kept::read(Some(&file))
        .into_iter()
        .map(|mut one| {
            one.key.release = "0.0.1".to_owned();
            one
        })
        .collect();
    let _ = std::fs::write(&file, serde_json::to_string(&held).unwrap_or_default());
    let new_release = Proving::exiting(0, "v8.7.3 (");
    let _ = verdicts(
        &updated,
        &running(Arc::clone(&new_release), &kept, "300", "1000"),
    )
    .await;
    assert_eq!(
        new_release.asked().len(),
        1,
        "another release is another run"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn starting_files_that_cannot_be_read_leave_it_unverified_and_unrun() {
    let kept = Scratch::new("proved-linked");
    let Some(installed) = installed(&kept) else {
        return;
    };
    let elsewhere = Scratch::new("proved-linked-elsewhere");
    let _ = std::fs::remove_dir_all(kept.join("quality/config/qualitysync"));
    let _ = std::os::unix::fs::symlink(elsewhere.path(), kept.join("quality/config/qualitysync"));
    let program = Proving::exiting(0, "v8.7.3 (");
    let said = verdicts(
        &installed,
        &running(Arc::clone(&program), &kept, "100", "1000"),
    )
    .await;
    assert!(program.asked().is_empty());
    assert!(
        matches!(said.first(), Some(Verdict::Unverified { reason, .. }) if reason.contains("is a link")),
        "{said:?}"
    );
}
