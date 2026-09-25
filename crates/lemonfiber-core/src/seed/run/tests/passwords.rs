//! The download client's password: set, recorded, and read back.

use super::*;

/// Whether a wiring failed, on one line so it holds no phantom coverage.
fn is_failed(wiring: &crate::seed::Wiring) -> bool {
    matches!(wiring.state, crate::seed::State::Failed { .. })
}

/// Whether a wiring is one a real pass would make, on one line for the same reason.
fn is_would_wire(wiring: &crate::seed::Wiring) -> bool {
    matches!(wiring.state, crate::seed::State::WouldWire { .. })
}

/// The three replies a full password exchange expects: log in, set, confirm.
fn exchange() -> Vec<(u16, &'static str)> {
    vec![(200, "Ok."), (200, ""), (200, "Ok.")]
}

#[test]
fn a_non_seed_outcome_carries_no_seed_report() {
    let version = Outcome::Version(VersionReport {
        binary: env!("CARGO_PKG_VERSION").to_owned(),
        supported_schema: vec![1],
        stack: "0.1.0".to_owned(),
        compose: None,
        changelog: crate::changelog::Notes::unread(),
    });
    assert!(seeded(Ok(version)).is_none());
}

#[tokio::test]
async fn seed_replaces_and_records_the_qbittorrent_password() {
    let env = config_scratch("seed-records");
    if let Some(parent) = env.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&env, "DATA_ROOT=/srv/media\n");
    let ctx = seed_ctx(
        Some(TEMP_LOG),
        true,
        exchange(),
        Some(vec![0x11; 24]),
        Some(env.to_path_buf()),
    );

    let outcome = dispatch(Command::Seed, &ctx).await;

    let json = outcome
        .as_ref()
        .ok()
        .and_then(|outcome| outcome.clone().envelope().to_json());
    assert!(json
        .as_deref()
        .is_some_and(|json| json.contains(r#""kind":"seed""#)));

    let report = seeded(outcome).unwrap_or_default();
    let wired = report
        .wirings
        .iter()
        .any(|wiring| wiring.state == crate::seed::State::Wired);
    assert!(wired, "the password is wired");

    let written = std::fs::read_to_string(&env).unwrap_or_default();
    assert!(written.contains("QBITTORRENT_PASSWORD="));
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// The whole claim, over a whole pass: the same walk against the same log and the
/// same services, with nothing set, nothing recorded and nothing kept for the next
/// run to compare against.
///
/// The counterpart above is the same context without `rehearsing`, and it asserts
/// the password *is* recorded — so these two together say the difference is the
/// flag rather than a fixture that could not have written anyway.
#[tokio::test]
async fn a_rehearsed_seed_names_what_it_would_do_and_records_none_of_it() {
    let env = config_scratch("seed-rehearsed");
    if let Some(parent) = env.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&env, "DATA_ROOT=/srv/media\n");
    let ctx = seed_ctx(
        Some(TEMP_LOG),
        true,
        exchange(),
        Some(vec![0x11; 24]),
        Some(env.to_path_buf()),
    )
    .rehearsing();

    let report = seeded(dispatch(Command::Seed, &ctx).await).unwrap_or_default();

    assert!(report.rehearsed, "{report:?}");
    assert!(
        report.wirings.iter().any(is_would_wire),
        "a rehearsal that named nothing it would do said nothing: {report:?}"
    );
    assert!(
        !report
            .wirings
            .iter()
            .any(|wiring| wiring.state == crate::seed::State::Wired),
        "something was wired: {report:?}"
    );

    let written = std::fs::read_to_string(&env).unwrap_or_default();
    assert!(
        !written.contains("QBITTORRENT_PASSWORD="),
        "a password was recorded: {written}"
    );
    assert!(
        !env.with_file_name("baseline.json").exists(),
        "the record a later run compares against was written by a question"
    );
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

#[tokio::test]
async fn seed_sets_the_password_even_with_nowhere_to_record_it() {
    let ctx = seed_ctx(Some(TEMP_LOG), true, exchange(), Some(vec![0x11; 24]), None);
    let report = seeded(dispatch(Command::Seed, &ctx).await).unwrap_or_default();
    let wired = report
        .wirings
        .iter()
        .any(|wiring| wiring.state == crate::seed::State::Wired);
    assert!(wired, "the password is set even with nowhere to record it");
}

#[tokio::test]
async fn the_baseline_persists_across_runs() {
    // The baseline is the one seed artifact kept between runs. A value an earlier
    // run recorded is pre-seeded here; two more passes — neither reaching a
    // service, so neither changing it — must load it, leave it, and save it back
    // unchanged, the round trip the drift policy is built to read.
    let env = config_scratch("baseline-across-runs");
    let baseline = env.with_file_name("baseline.json");
    let recorded =
        r#"{"services":{"sonarr":{"downloadclient:sabnzbd:8080":{"value":"tv","at":"1"}}}}"#;
    let _ = crate::config::store::write(&baseline, recorded);

    let first = seed_ctx(None, false, Vec::new(), None, Some(env.to_path_buf()));
    let _ = dispatch(Command::Seed, &first).await;
    let second = seed_ctx(None, false, Vec::new(), None, Some(env.to_path_buf()));
    let _ = dispatch(Command::Seed, &second).await;

    let read_back = std::fs::read_to_string(&baseline).unwrap_or_default();
    assert!(
        read_back.contains(r#""value":"tv""#) && read_back.contains(r#""at":"1""#),
        "the recorded value and its timestamp survive both passes unchanged: {read_back}"
    );
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

#[tokio::test]
async fn seed_reports_an_unreadable_stack_rather_than_guessing() {
    let nowhere = Source::External(std::path::Path::new("/lemonfiber/no/such/stack"));
    let ctx = a_context()
        .engine(Arc::new(Reporting::default()))
        .over(nowhere)
        .build();
    let outcome = dispatch(Command::Seed, &ctx).await;
    assert_eq!(
        outcome.err().map(|problem| problem.code),
        Some(crate::error::codes::stack::STACK_UNREADABLE)
    );
}

#[tokio::test]
async fn seed_reports_a_failed_password_change_and_records_nothing() {
    // The temporary password is read but the client rejects it, so the change
    // fails and there is no generated value to record.
    let ctx = seed_ctx(
        Some(TEMP_LOG),
        true,
        vec![(200, "Fails.")],
        Some(vec![0x11; 24]),
        None,
    );
    let report = seeded(dispatch(Command::Seed, &ctx).await).unwrap_or_default();

    // Both words are in this one report, and they send the operator to different
    // places. A skip is a wiring this run could not attempt — the service has not
    // finished starting — and the answer to it is to run seeding again. A failure
    // is a service that was asked and said no, and running again will produce the
    // same no. A run that reported the refusal as a skip would have the operator
    // waiting for a stack that was never going to settle.
    let failed = report
        .wirings
        .iter()
        .filter(|wiring| is_failed(wiring))
        .count();
    let skipped = report
        .wirings
        .iter()
        .filter(|wiring| is_skipped(wiring))
        .count();
    assert!(failed > 0, "a rejected change is reported as failed");
    assert!(
        skipped > 0,
        "and the wirings this run never got to are skips rather than failures"
    );
}

#[tokio::test]
async fn seed_skips_qbittorrent_when_no_password_is_announced() {
    let ctx = seed_ctx(None, true, Vec::new(), Some(vec![0x11; 24]), None);
    let report = seeded(dispatch(Command::Seed, &ctx).await).unwrap_or_default();
    assert!(
        !report.wirings.is_empty(),
        "the run produced wirings to judge, not an empty report from an error"
    );
    let all_skipped = report.wirings.iter().all(is_skipped);
    assert!(
        all_skipped,
        "an unannounced password is skipped, not failed"
    );
}

#[tokio::test]
async fn seed_skips_qbittorrent_when_its_log_cannot_be_read() {
    let ctx = seed_ctx(None, false, Vec::new(), Some(vec![0x11; 24]), None);
    let report = seeded(dispatch(Command::Seed, &ctx).await).unwrap_or_default();
    assert!(
        !report.wirings.is_empty(),
        "the run produced wirings to judge, not an empty report from an error"
    );
    let all_skipped = report.wirings.iter().all(is_skipped);
    assert!(all_skipped, "an unreadable log is skipped, not failed");
}

/// A password already set is reported, not set a second time.
///
/// The temporary one stays in the container's log — a log is not consumed by
/// being read — so a run that reached for it again would authenticate with a
/// credential spent on the first run and call a healthy stack refused. Asserted
/// on the requests that went out: reporting `AlreadyWired` while still writing
/// a new password would look right here and be the same defect.
#[tokio::test]
async fn a_password_already_set_is_reported_rather_than_set_again() {
    const ANNOUNCED: &str = "A temporary password is provided for this session: spent";
    let path = config_scratch("qbt-already-set");
    let _ = store::set(
        &path,
        crate::config::QBITTORRENT_PASSWORD_KEY,
        "minted-earlier",
    );
    let http = Fake::always(Answer::reply(200, "Ok."));
    let ctx = seed_ctx(
        Some(ANNOUNCED),
        true,
        Vec::new(),
        None,
        Some(path.to_path_buf()),
    )
    .with_http(http.clone());

    let (wiring, recorded) = super::super::seed_qbittorrent_password(
        &ctx,
        &("qbittorrent".to_owned(), "http://127.0.0.1:8081".to_owned()),
    )
    .await;

    assert_eq!(wiring.state, crate::seed::State::AlreadyWired);
    // The value is deliberately not in the message: it is a credential, and a
    // failing assertion prints its message into the run's log.
    assert!(
        recorded.is_none(),
        "a password already in force was minted again"
    );
    assert!(
        !http
            .requests()
            .iter()
            .any(|asked| asked.url.contains("setPreferences")),
        "a password already in force was set again"
    );
}

/// A recorded password the client no longer takes falls through to the
/// temporary one — the case of a container rebuilt from nothing.
#[tokio::test]
async fn a_recorded_password_the_client_refuses_falls_through_to_the_temporary() {
    const ANNOUNCED: &str = "A temporary password is provided for this session: fresh";
    let path = config_scratch("qbt-stale-record");
    let _ = store::set(
        &path,
        crate::config::QBITTORRENT_PASSWORD_KEY,
        "from-a-container-that-is-gone",
    );
    // Log in, refused; log in with the temporary, taken; set; confirm.
    let http = Fake::by_path_in_turn(vec![
        (
            "/auth/login",
            vec![
                Answer::reply(200, "Fails."),
                Answer::reply(200, "Ok."),
                Answer::reply(200, "Ok."),
            ],
        ),
        ("/app/setPreferences", vec![Answer::reply(200, "")]),
    ]);
    let ctx = seed_ctx(
        Some(ANNOUNCED),
        true,
        Vec::new(),
        Some(vec![7; 32]),
        Some(path.to_path_buf()),
    )
    .with_http(http.clone());

    let (wiring, recorded) = super::super::seed_qbittorrent_password(
        &ctx,
        &("qbittorrent".to_owned(), "http://127.0.0.1:8081".to_owned()),
    )
    .await;

    assert_eq!(wiring.state, crate::seed::State::Wired);
    assert!(recorded.is_some(), "the minted password is handed back");
    assert!(
        http.requests()
            .iter()
            .any(|asked| asked.url.contains("setPreferences")),
        "the temporary password was never used to set a new one"
    );
}

/// A rehearsal will not sign in to find out whether the recorded password is still
/// the one in force, and says that rather than guessing either way.
///
/// Signing in is how that question is answered and signing in is a `POST` — a
/// session left on somebody else's service by a run that promised to leave nothing.
/// The recorded password usually *is* still the one in force, and on a container
/// rebuilt from nothing it is not; the difference is exactly what the sign-in exists
/// to find out, so neither answer can honestly be reported without asking. The
/// client is scripted to accept everything on purpose, so a run that reached for it
/// would come back `AlreadyWired` and this would catch it.
#[tokio::test]
async fn a_rehearsed_pass_will_not_sign_in_to_test_the_password_it_recorded() {
    let path = config_scratch("qbt-rehearsed");
    let _ = store::set(
        &path,
        crate::config::QBITTORRENT_PASSWORD_KEY,
        "minted-earlier",
    );
    let http = Fake::always(Answer::reply(200, "Ok."));
    let ctx = seed_ctx(
        Some(TEMP_LOG),
        true,
        Vec::new(),
        None,
        Some(path.to_path_buf()),
    )
    .with_http(http.clone())
    .rehearsing();

    let (wiring, recorded) = super::super::seed_qbittorrent_password(
        &ctx,
        &("qbittorrent".to_owned(), "http://127.0.0.1:8081".to_owned()),
    )
    .await;

    // Nothing this call answered with is put in an assertion message. The pair
    // carries a minted password in its second half, and a failing assertion
    // prints its message into the run's log — so each check below says what went
    // wrong rather than showing what came back.
    assert!(
        is_skipped(&wiring),
        "a rehearsal answered the question only a sign-in can answer"
    );
    assert!(
        format!("{wiring:?}").contains("signing in"),
        "the operator was not told why this run could not tell"
    );
    assert!(recorded.is_none(), "a rehearsal minted a password");
    let asked = http.requests();
    assert!(
        asked.is_empty(),
        "a rehearsal opened a session on the torrent client: {asked:?}"
    );
}

#[tokio::test]
async fn a_later_seed_offers_qbittorrent_from_its_recorded_password() {
    // The temporary password is gone, so nothing is minted this run; the
    // password recorded earlier stands in and qBittorrent is offered anyway.
    const SERVARR: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
    const SABNZBD: &str = "[misc]\napi_key = the-sab-key\n";
    let path = config_scratch("qbt-later-seed");
    let _ = store::set(
        &path,
        crate::config::QBITTORRENT_PASSWORD_KEY,
        "minted-earlier",
    );
    let ctx = seed_ctx(None, true, Vec::new(), None, Some(path.to_path_buf()))
        .with_http(seeding())
        .with_filesystem(Arc::new(SeedFs::keyed(Some(SERVARR), Some(SABNZBD))));

    let report = seeded(dispatch(Command::Seed, &ctx).await).unwrap_or_default();
    let clients = download_client_wirings(&report);
    assert_eq!(clients.len(), 6, "both clients into each of three arrs");
    let qbittorrent = clients
        .iter()
        .filter(|wiring| wiring.connection.starts_with("qBittorrent into "))
        .count();
    assert_eq!(
        qbittorrent, 3,
        "qBittorrent is offered to every arr on a later run"
    );
}
