use std::sync::Arc;

use lemonfiber_fixtures::support::Recording;

use super::{BridgingCheck, CHECK, NETWORKS_MIRRORED};
use crate::doctor::{Category, Check, Verdict};
use crate::ports::docker::{Origin, Target};
use crate::ports::process::{Failure, Output};

const SHOWN_ON: &str = "network.https: true\nnetwork.subnet4: 192.168.138.0/23\nnetwork_bridge: true\nnetwork_proxy: auto\n";

fn orbstack() -> Target {
    Target::at(
        "unix:///Users/ana/.orbstack/run/docker.sock",
        Origin::Context("orbstack".to_owned()),
    )
}

fn said(status: i32, stdout: &str) -> Arc<Recording> {
    Arc::new(Recording::answering(Ok(Output {
        status: Some(status),
        stdout: stdout.to_owned(),
        stderr: String::new(),
    })))
}

async fn verdict(runner: &Arc<Recording>, target: Target) -> Option<Verdict> {
    let check = BridgingCheck::new(runner.clone(), target);
    assert_eq!(check.category(), Category::Environment);
    let findings = check.run().await;
    assert_eq!(findings.len(), 1);
    findings.into_iter().next().map(|finding| {
        assert_eq!(finding.check, CHECK);
        finding.verdict
    })
}

#[tokio::test]
async fn orbstack_mirroring_the_stacks_networks_is_a_warning_naming_the_setting_and_its_fix() {
    let runner = said(0, SHOWN_ON);
    let found = verdict(&runner, orbstack()).await;
    let Some(Verdict::Warn(problem)) = found else {
        unreachable!("a bridge that is on is a warning: {found:?}");
    };
    assert_eq!(problem.code, NETWORKS_MIRRORED);
    let actions: Vec<&str> = problem
        .remedies
        .iter()
        .map(|remedy| remedy.action.as_str())
        .collect();
    assert_eq!(
        actions,
        vec!["Run orb config set network_bridge false, then restart OrbStack"]
    );
    assert_eq!(runner.seen(), vec![vec!["orb", "config", "show"]]);
}

#[tokio::test]
async fn orbstack_with_its_bridge_off_passes() {
    let runner = said(
        0,
        &SHOWN_ON.replace("network_bridge: true", "network_bridge: false"),
    );
    assert!(matches!(
        verdict(&runner, orbstack()).await,
        Some(Verdict::Pass { note: Some(_) })
    ));
}

#[tokio::test]
async fn an_orb_that_cannot_be_run_or_read_leaves_it_unverified_never_failed() {
    let absent = Arc::new(Recording::answering(Err(Failure::NotFound {
        program: "orb".to_owned(),
    })));
    let unanswering = said(1, SHOWN_ON);
    let unsaid = said(0, "network.https: true\n");
    let unreadable = said(0, "network_bridge: maybe\n");
    let unlined = said(0, "network_bridge true\n");
    for runner in [&absent, &unanswering, &unsaid, &unreadable, &unlined] {
        let found = verdict(runner, orbstack()).await;
        let Some(Verdict::Unverified { reason, remedy }) = found else {
            unreachable!("what cannot be read is not established: {found:?}");
        };
        assert!(reason.contains("network_bridge"), "{reason}");
        assert!(
            remedy.action.contains("orb config show"),
            "{}",
            remedy.action
        );
    }
}

#[tokio::test]
async fn another_engine_is_skipped_without_asking_orb_anything() {
    let runner = said(0, SHOWN_ON);
    for target in [
        Target::local(),
        Target::socket("/Users/ana/.docker/run/docker.sock"),
    ] {
        assert!(matches!(
            verdict(&runner, target).await,
            Some(Verdict::Skipped { .. })
        ));
    }
    assert!(runner.seen().is_empty());
}
