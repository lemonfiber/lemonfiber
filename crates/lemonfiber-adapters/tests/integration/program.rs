//! Running an image's own program once, driven against an engine written for it: the
//! container created, attached, started, read, waited on and removed, whatever it came to.

use std::time::Duration;

use crate::fake;
#[cfg(unix)]
use lemonfiber_adapters::Daemon;
use lemonfiber_ports::program::{Ended, Program as _, Run, Unran};

fn run(within: Duration) -> Run {
    Run {
        image: "ghcr.io/recyclarr/recyclarr@sha256:9c6c".to_owned(),
        args: vec!["--version".to_owned()],
        env: vec!["TZ=UTC".to_owned()],
        user: Some("1000:1000".to_owned()),
        security_opt: vec!["no-new-privileges:true".to_owned()],
        cap_drop: vec!["ALL".to_owned()],
        cap_add: Vec::new(),
        pids_limit: 1024,
        read_only: true,
        memory: None,
        configuration: None,
        within,
        read_at_most: 64 * 1024,
    }
}

/// Every route asked for that removes the proof's container.
fn removed(asked: &[String]) -> usize {
    asked
        .iter()
        .filter(|path| path.starts_with("/containers/p1?") && path.contains("force=true"))
        .count()
}

#[cfg(unix)]
#[tokio::test]
async fn a_run_reports_what_it_wrote_and_how_it_ended_and_leaves_nothing_behind() {
    let mut engine = fake::engine(
        "program",
        vec![
            (
                "/containers/json",
                fake::Reply::Body(
                    200,
                    r#"[{"Id":"stale","Created":1,"Labels":{"app.lemonfiber.proof":"run"}}]"#
                        .to_owned(),
                ),
            ),
            (
                "/containers/p1/attach",
                fake::Reply::Upgraded(vec![
                    (1, "v8.7.3 (Branch.master)\n".to_owned()),
                    (2, "a warning nobody judges\n".to_owned()),
                ]),
            ),
            (
                "/containers/p1/start",
                fake::Reply::Body(204, String::new()),
            ),
            (
                "/containers/p1/wait",
                fake::Reply::Body(200, r#"{"StatusCode":3}"#.to_owned()),
            ),
            (
                "/containers/create",
                fake::Reply::Body(201, r#"{"Id":"p1","Warnings":[]}"#.to_owned()),
            ),
            ("/containers/p1?", fake::Reply::Body(204, String::new())),
            ("/containers/stale?", fake::Reply::Body(204, String::new())),
        ],
    );

    let ran = Daemon::at(&engine.socket)
        .run(&run(Duration::from_secs(5)))
        .await;

    assert_eq!(
        ran.map(|ran| (ran.ended, String::from_utf8_lossy(&ran.stdout).into_owned())),
        Ok((Ended::Exited(3), "v8.7.3 (Branch.master)\n".to_owned()))
    );
    let asked = engine.asked_for();
    assert_eq!(removed(&asked), 1, "{asked:?}");
    assert!(
        asked
            .iter()
            .any(|path| path.starts_with("/containers/stale?")),
        "a container an earlier run left is removed: {asked:?}"
    );
    assert!(
        !asked.iter().any(|path| path.contains("/images/create")),
        "nothing is pulled: {asked:?}"
    );
    engine.stop().await;
}

#[cfg(unix)]
#[tokio::test]
async fn a_run_still_going_at_its_limit_is_stopped_and_removed() {
    let mut engine = fake::engine(
        "program-overran",
        vec![
            ("/containers/json", fake::Reply::Body(200, "[]".to_owned())),
            (
                "/containers/p1/attach",
                fake::Reply::Upgraded(vec![(1, "starting\n".to_owned())]),
            ),
            (
                "/containers/p1/start",
                fake::Reply::Body(204, String::new()),
            ),
            ("/containers/p1/wait", fake::Reply::Held),
            (
                "/containers/create",
                fake::Reply::Body(201, r#"{"Id":"p1","Warnings":[]}"#.to_owned()),
            ),
            ("/containers/p1?", fake::Reply::Body(204, String::new())),
        ],
    );

    let ran = Daemon::at(&engine.socket)
        .run(&run(Duration::from_millis(300)))
        .await;

    assert_eq!(
        ran.map(|ran| (ran.ended, ran.stdout)),
        Ok((Ended::Overran, b"starting\n".to_vec()))
    );
    assert_eq!(removed(&engine.asked_for()), 1);
    engine.stop().await;
}

#[cfg(unix)]
#[tokio::test]
async fn an_image_the_engine_does_not_hold_is_named_and_nothing_is_made() {
    let mut engine = fake::engine(
        "program-no-image",
        vec![
            ("/containers/json", fake::Reply::Body(200, "[]".to_owned())),
            (
                "/containers/create",
                fake::Reply::Body(
                    404,
                    r#"{"message":"No such image: ghcr.io/recyclarr/recyclarr@sha256:9c6c"}"#
                        .to_owned(),
                ),
            ),
        ],
    );

    let ran = Daemon::at(&engine.socket)
        .run(&run(Duration::from_secs(5)))
        .await;

    assert_eq!(
        ran,
        Err(Unran::NoImage {
            image: "ghcr.io/recyclarr/recyclarr@sha256:9c6c".to_owned()
        })
    );
    assert_eq!(removed(&engine.asked_for()), 0);
    engine.stop().await;
}

#[cfg(unix)]
#[tokio::test]
async fn a_container_that_will_not_start_is_refused_and_still_removed() {
    let mut engine = fake::engine(
        "program-unstarted",
        vec![
            ("/containers/json", fake::Reply::Body(200, "[]".to_owned())),
            ("/containers/p1/attach", fake::Reply::Upgraded(Vec::new())),
            (
                "/containers/p1/start",
                fake::Reply::Body(
                    500,
                    r#"{"message":"bind source path does not exist"}"#.to_owned(),
                ),
            ),
            (
                "/containers/create",
                fake::Reply::Body(201, r#"{"Id":"p1","Warnings":[]}"#.to_owned()),
            ),
            ("/containers/p1?", fake::Reply::Body(204, String::new())),
        ],
    );

    let ran = Daemon::at(&engine.socket)
        .run(&run(Duration::from_secs(5)))
        .await;

    assert!(
        matches!(&ran, Err(Unran::Refused { reason }) if reason.contains("bind source path")),
        "{ran:?}"
    );
    assert_eq!(removed(&engine.asked_for()), 1);
    engine.stop().await;
}

#[cfg(unix)]
#[tokio::test]
async fn an_absent_engine_runs_nothing() {
    let nowhere = std::path::PathBuf::from("/lemonfiber/no/such/engine.sock");
    let ran = Daemon::at(&nowhere).run(&run(Duration::from_secs(5))).await;
    assert!(matches!(ran, Err(Unran::Unreachable { .. })), "{ran:?}");
}

#[cfg(unix)]
#[tokio::test]
async fn an_engine_that_will_not_attach_or_say_how_a_run_ended_is_refused_and_still_removed() {
    let mut unattached = fake::engine(
        "program-unattached",
        vec![
            ("/containers/json", fake::Reply::Body(500, "{}".to_owned())),
            (
                "/containers/p1/attach",
                fake::Reply::Body(500, r#"{"message":"cannot attach"}"#.to_owned()),
            ),
            (
                "/containers/create",
                fake::Reply::Body(201, r#"{"Id":"p1","Warnings":[]}"#.to_owned()),
            ),
            ("/containers/p1?", fake::Reply::Body(204, String::new())),
        ],
    );
    let ran = Daemon::at(&unattached.socket)
        .run(&run(Duration::from_secs(5)))
        .await;
    assert!(
        matches!(&ran, Err(Unran::Refused { reason }) if reason.contains("cannot attach")),
        "{ran:?}"
    );
    assert_eq!(removed(&unattached.asked_for()), 1);
    unattached.stop().await;

    let mut unsaid = fake::engine(
        "program-unsaid",
        vec![
            ("/containers/json", fake::Reply::Body(200, "[]".to_owned())),
            ("/containers/p1/attach", fake::Reply::Upgraded(Vec::new())),
            (
                "/containers/p1/start",
                fake::Reply::Body(204, String::new()),
            ),
            ("/containers/p1/wait", fake::Reply::Body(200, String::new())),
            (
                "/containers/create",
                fake::Reply::Body(201, r#"{"Id":"p1","Warnings":[]}"#.to_owned()),
            ),
            ("/containers/p1?", fake::Reply::Body(204, String::new())),
        ],
    );
    let ran = Daemon::at(&unsaid.socket)
        .run(&run(Duration::from_secs(5)))
        .await;
    assert!(matches!(&ran, Err(Unran::Refused { .. })), "{ran:?}");
    assert_eq!(removed(&unsaid.asked_for()), 1);
    unsaid.stop().await;
}
