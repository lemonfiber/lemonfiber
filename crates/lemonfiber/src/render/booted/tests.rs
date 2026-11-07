use lemonfiber_core::model::{BootOutcome, BootReport, BootTunnel, BootVerification, Forwarded};

use super::booted;

fn verification(against: Option<u64>, outcome: BootOutcome) -> BootVerification {
    BootVerification {
        against,
        at: 1_759_400_000,
        outcome,
        not_back: vec!["qbittorrent".to_owned()],
        tunnel: BootTunnel::NotEstablished {
            said: "the stack came back and its tunnel did not: vpn.tunnel".to_owned(),
        },
        port: Some(Forwarded {
            was: Some(51_413),
            now: None,
            pushed: false,
        }),
        said: Some("the stack came back and its tunnel did not: vpn.tunnel".to_owned()),
    }
}

#[test]
fn the_verification_after_the_last_restart_is_said_with_each_part_of_it() {
    let failed = verification(Some(1_759_399_000), BootOutcome::Failed);
    let said = booted(&BootReport {
        booted_at: Some(1_759_399_000),
        verified: Some(failed.clone()),
        last_verified: Some(failed),
        declined: None,
        failing_since: Some(1_759_400_000),
        failed_boots: Some(2),
    })
    .text();

    assert!(
        said.contains("This machine last started at 2025-10-02T09:56:40 UTC."),
        "{said}"
    );
    assert!(
        said.contains("After it, at 2025-10-02T10:13:20 UTC, the stack did not come back."),
        "{said}"
    );
    assert!(said.contains("  not back   qbittorrent"), "{said}");
    assert!(
        said.contains("  tunnel     not established: the stack came back"),
        "{said}"
    );
    assert!(
        said.contains("  port       51413 before, none now, not every client takes it"),
        "{said}"
    );
    assert!(
        said.contains(
            "The stack has not come back since 2025-10-02T10:13:20 UTC, across 2 start(s)."
        ),
        "{said}"
    );
}

#[test]
fn a_restart_with_no_verification_says_so_and_why_beside_the_newest_one() {
    let mut earlier = verification(None, BootOutcome::Passed);
    earlier.tunnel = BootTunnel::Held;
    earlier.port = Some(Forwarded {
        was: None,
        now: Some(51_413),
        pushed: true,
    });
    let said = booted(&BootReport {
        booted_at: None,
        verified: None,
        last_verified: Some(earlier),
        declined: Some("this machine is running on its battery".to_owned()),
        failing_since: None,
        failed_boots: None,
    })
    .text();

    assert!(
        said.contains("This machine does not say when it last started."),
        "{said}"
    );
    assert!(
        said.contains("No verification has run since it started."),
        "{said}"
    );
    assert!(
        said.contains("  Nothing was started: this machine is running on its battery"),
        "{said}"
    );
    assert!(
        said.contains("After an earlier start, at 2025-10-02T10:13:20 UTC, the stack came back."),
        "{said}"
    );
    assert!(said.contains("  tunnel     held"), "{said}");
    assert!(
        said.contains("  port       none before, 51413 now, every client takes it"),
        "{said}"
    );
    assert!(!said.contains("has not come back since"), "{said}");
}

#[test]
fn a_part_waiting_and_an_unchecked_tunnel_are_said_as_they_are() {
    let mut waiting = verification(Some(5), BootOutcome::WaitingForInstall);
    waiting.tunnel = BootTunnel::NotChecked;
    waiting.port = None;
    waiting.not_back = Vec::new();
    waiting.said = None;
    let said = booted(&BootReport {
        booted_at: Some(10),
        last_verified: Some(waiting),
        ..BootReport::default()
    })
    .text();

    assert!(said.contains("After 1970-01-01T00:00:05 UTC,"), "{said}");
    assert!(
        said.contains("came back with a part waiting for its install"),
        "{said}"
    );
    assert!(said.contains("  tunnel     not checked"), "{said}");
    assert!(
        !said.contains("not back") && !said.contains("port "),
        "{said}"
    );
}
