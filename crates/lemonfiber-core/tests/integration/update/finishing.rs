//! What a finished run records, and what it waits for first.

use super::{asking, behind, recording, reported, Coming, Kept, Machine, SONARR};
use lemonfiber_core::app::{dispatch, Ctx, Waiting};
use lemonfiber_core::archive::{Archiving, Vault};
use lemonfiber_core::config::paths::Paths;
use lemonfiber_core::config::{store, Protocols, Settings, QBITTORRENT_PASSWORD_KEY};
use lemonfiber_core::ports::docker::Engine;
use lemonfiber_core::ports::http::Http;
use lemonfiber_core::ports::process::Runner;
use lemonfiber_core::update::State;
use lemonfiber_fixtures::downloads::{QBIT_FINISHED, QBIT_TORRENTS, SAB_EMPTY};
use lemonfiber_fixtures::files::Files;
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::pulled::Pulled;
use lemonfiber_fixtures::support::a_password;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

/// Moving a service from one pinned version to another is the largest change this
/// product makes to a machine, and it was the one the record said nothing about.
#[tokio::test]
async fn a_confirmed_run_records_what_it_moved_and_where_the_capture_went() {
    let machine = Machine::coming(Coming::Answering);
    let archive = Kept::writing(true);
    let (context, journal) = recording(&machine, &archive, "moved");

    let report = reported(dispatch(asking(true, Waiting::Never), &context).await);
    assert_eq!(
        report.map(|report| report.state),
        Some(State::Updated),
        "the run has to have worked for its record to be worth reading"
    );

    let written = std::fs::read_to_string(&journal).unwrap_or_default();
    for held in [
        r#""operation":"update""#.to_owned(),
        r#""target":"sonarr""#.to_owned(),
        r#""action":"pinned""#.to_owned(),
        format!(r#""previous":"{}""#, SONARR.0),
        format!(r#""current":"{}""#, SONARR.1),
    ] {
        assert!(written.contains(&held), "{held} is missing from {written}");
    }
    // Where to go instead, for the one reversal this cannot perform — named by path,
    // since the run that took the capture is the only thing that knows which it is.
    assert!(
        !written.contains(r#""backup":null"#),
        "the capture is not named: {written}"
    );
}

/// A private environment file recording qBittorrent's password, at a scratch path
/// unique to this case so concurrent tests do not share one.
fn env_at(name: &str) -> PathBuf {
    let dir = lemonfiber_fixtures::scratch::Scratch::named(&format!("update-{name}")).kept();
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.join(".env");
    assert!(
        store::set(&path, QBITTORRENT_PASSWORD_KEY, &a_password()).is_ok(),
        "the scratch environment file is written"
    );
    path
}

/// The same context, reaching the download clients over `http` and holding a key for
/// the one of them that needs one.
fn transferring(
    machine: &Arc<Machine>,
    archive: &Arc<Kept>,
    http: Arc<dyn Http>,
    name: &str,
) -> Ctx {
    lemonfiber_testing::a_context()
        .runner(Arc::clone(machine) as Arc<dyn Runner>)
        .engine(Arc::clone(machine) as Arc<dyn Engine>)
        .filesystem(Files::empty())
        .settings(Settings {
            protocols: Protocols::both(),
            env_file: Some(env_at(name)),
            ..Settings::default()
        })
        .build()
        .with_images(Pulled::holding(behind(&[("sonarr", SONARR.0)])))
        .with_http(http)
        .with_archives(Archiving {
            paths: Paths::rooted(Path::new("/cfg"), Path::new("/data")),
            vault: Arc::clone(archive) as Arc<dyn Vault>,
        })
        .with_patience(Duration::ZERO)
}

/// A qBittorrent still working on something, answering the same way every time.
fn still_coming_down() -> Arc<Fake> {
    Fake::by_path_in_turn(vec![
        ("/auth/login", vec![Answer::reply(200, "Ok.")]),
        ("/torrents/info", vec![Answer::reply(200, QBIT_TORRENTS)]),
        ("", vec![Answer::reply(200, SAB_EMPTY)]),
    ])
}

#[tokio::test]
async fn a_run_that_would_interrupt_a_transfer_is_refused_rather_than_carried_out() {
    let machine = Machine::coming(Coming::Answering);
    let archive = Kept::writing(true);
    let context = transferring(&machine, &archive, still_coming_down(), "refused");

    let refused = dispatch(asking(true, Waiting::Never), &context).await;

    let code = refused.err().map(|problem| problem.code.to_string());
    assert_eq!(code.as_deref(), Some("UPDATE-3"));
    assert_eq!(archive.written(), 0, "the stack was captured anyway");
    assert!(machine.started().is_empty(), "a service was moved anyway");
}

#[tokio::test(start_paused = true)]
async fn a_run_asked_to_wait_lets_what_is_coming_down_finish_first() {
    // Coming down twice and finished on the third look: something to wait for, a look
    // with no news, and an end.
    let http = Fake::by_path_in_turn(vec![
        ("/auth/login", vec![Answer::reply(200, "Ok.")]),
        (
            "/torrents/info",
            vec![
                Answer::reply(200, QBIT_TORRENTS),
                Answer::reply(200, QBIT_TORRENTS),
                Answer::reply(200, QBIT_FINISHED),
            ],
        ),
        ("", vec![Answer::reply(200, SAB_EMPTY)]),
    ]);
    let machine = Machine::coming(Coming::Answering);
    let archive = Kept::writing(true);
    let context = transferring(&machine, &archive, http, "waited");

    let report = reported(dispatch(asking(true, Waiting::ForTheDownloads), &context).await);

    let read = report.map(|report| (report.state, report.in_flight.len()));
    assert_eq!(
        read,
        Some((State::Updated, 1)),
        "the wait was taken and what it was waiting on is still named"
    );
    assert_eq!(machine.started(), vec!["sonarr".to_owned()]);
}

/// An update that moved a service and could not record it is said as that: it stands,
/// and it cannot be put back, which is what a run the record does not have is.
#[tokio::test]
async fn a_run_that_cannot_be_recorded_says_it_stands_unrecorded() {
    let machine = Machine::coming(Coming::Answering);
    let archive = Kept::writing(true);
    let (context, journal) = recording(&machine, &archive, "unrecorded");
    let mut staging = journal.file_name().unwrap_or_default().to_os_string();
    staging.push(".writing");
    assert!(std::fs::create_dir_all(journal.with_file_name(staging).join("held")).is_ok());

    let refused = dispatch(asking(true, Waiting::Never), &context).await.err();

    assert_eq!(
        refused.as_ref().map(|problem| problem.code),
        Some(lemonfiber_core::error::codes::undo::CANNOT_SUCCEED),
        "{refused:?}"
    );
    assert!(
        refused.is_some_and(|problem| problem.summary.contains("The update was done")),
        "the run was not said to stand"
    );
}
