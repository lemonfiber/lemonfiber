//! What lemonfiber last wrote, and telling it apart from what the operator changed.

use super::*;

#[tokio::test]
async fn a_wired_root_folder_the_host_cannot_back_is_a_warning() {
    // The *arr files into `/data/media/tv`, but the host directory it resolves to
    // is not there — the operator repointed the data root, or the media directory
    // was never made. The *arr imports into a void, so the folder is raised to a
    // warning naming the missing path.
    let filesystem = SeedFs::keyed(None, None).missing(vec!["media/tv"]);
    let wanted = [root("tv")];
    let mut wirings = vec![Wiring::settled(
        "tv root folder in Sonarr".to_owned(),
        State::Wired,
    )];
    escalate_broken_roots(
        &filesystem,
        Some(std::path::Path::new("/srv/media")),
        &wanted,
        &mut wirings,
    )
    .await;
    assert!(
        broken(&wirings).is_some_and(|breakage| breakage.contains("media/tv")),
        "the warning names the path that resolves to nothing"
    );
}

#[tokio::test]
async fn a_wired_root_folder_backed_on_disk_stays_informational() {
    // The host directory is there, so the folder files where it should — nothing is
    // broken, and it stays the settled connection it is.
    let filesystem = SeedFs::keyed(None, None);
    let wanted = [root("tv")];
    let mut wirings = vec![Wiring::settled(
        "tv root folder in Sonarr".to_owned(),
        State::AlreadyWired,
    )];
    escalate_broken_roots(
        &filesystem,
        Some(std::path::Path::new("/srv/media")),
        &wanted,
        &mut wirings,
    )
    .await;
    assert!(broken(&wirings).is_none());
}

#[tokio::test]
async fn a_root_folder_check_without_a_data_root_escalates_nothing() {
    // Without a data root the host path cannot be resolved, so nothing can be
    // confirmed or denied — the check escalates nothing rather than guessing.
    let filesystem = SeedFs::keyed(None, None).missing(vec!["media/tv"]);
    let wanted = [root("tv")];
    let mut wirings = vec![Wiring::settled(
        "tv root folder in Sonarr".to_owned(),
        State::Wired,
    )];
    escalate_broken_roots(&filesystem, None, &wanted, &mut wirings).await;
    assert!(broken(&wirings).is_none());
}

#[tokio::test]
async fn a_root_folder_not_wired_is_not_warned_even_where_the_path_is_missing() {
    // A skipped folder is not one the *arr files into, so a missing path there is
    // not yet a break — only the folders the *arr actually holds are checked.
    let filesystem = SeedFs::keyed(None, None).missing(vec!["media/tv"]);
    let wanted = [root("tv")];
    let mut wirings = vec![Wiring::settled(
        "tv root folder in Sonarr".to_owned(),
        State::Skipped {
            reason: "later".to_owned(),
        },
    )];
    escalate_broken_roots(
        &filesystem,
        Some(std::path::Path::new("/srv/media")),
        &wanted,
        &mut wirings,
    )
    .await;
    assert!(broken(&wirings).is_none());
}

#[tokio::test]
async fn a_reset_previews_then_reverts_a_drifted_connection() {
    const KEYED: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
    let dir = std::env::temp_dir().join(format!("lemonfiber-reset-conn-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    let env = dir.join(".env");
    // A recorded qBittorrent password makes a qBittorrent download client wanted; a
    // baseline recording lemonfiber's category for it, against the categoryless client
    // the service now reports, reads as the operator's drift to revert.
    let _ = std::fs::write(&env, "QBITTORRENT_PASSWORD=pw\n");
    let _ = crate::config::store::write(
        &dir.join("baseline.json"),
        r#"{"services":{"Sonarr":{"downloadclient:gluetun:8081":{"value":"tv","at":"1"}}}}"#,
    );

    let context = a_context()
        .settings(Settings {
            env_file: Some(env.clone()),
            ..Settings::default()
        })
        .build()
        .with_filesystem(Arc::new(SeedFs::keyed(Some(KEYED), None)))
        .with_http(seeding());

    // Preview: the drifted connection is named, and nothing is written.
    let preview = super::super::reset_connections(&context, false).await;
    assert!(
        preview
            .iter()
            .any(|wiring| wiring.connection.contains("into Sonarr")),
        "the drifted connection is previewed"
    );

    // Confirm: it is reverted — the category written back in place, so it reads wired.
    let confirmed = super::super::reset_connections(&context, true).await;
    assert!(
        confirmed
            .iter()
            .any(|wiring| matches!(wiring.state, crate::seed::State::Wired)),
        "the drifted connection is reverted"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// The Servarr routing, but answering `system/status` with a set version and
/// holding each download client under a drifted category — so a schema change
/// (version bumped, every client moved) can be driven end to end.
/// A seed run whose \*arrs report the given major version, and hold a second
/// download client the ordinary routes do not.
///
/// Three routes over the ordinary table rather than a transport of its own: the
/// first match wins, so stating what differs is enough and the rest stays shared.
fn versioned(version: &'static str) -> Arc<Fake> {
    seeding_with(vec![
        (
            "system/status",
            Answer::reply(
                200,
                format!(r#"{{"appName":"Sonarr","version":"{version}"}}"#),
            ),
        ),
        ("/downloadclient/testall", Answer::reply(200, "[]")),
        (
            "/downloadclient",
            Answer::reply(
                200,
                r#"[{"id":1,"fields":[{"name":"host","value":"sabnzbd"},{"name":"port","value":8080},{"name":"tvCategory","value":"shows"}]},{"id":2,"fields":[{"name":"host","value":"gluetun"},{"name":"port","value":8081},{"name":"tvCategory","value":"shows"}]}]"#,
            ),
        ),
    ])
}

/// A transport answering the client list with `clients`, and success to everything
/// else — the reset previews turn on what that one list says, including its refusal.
fn clients_answering(clients: Answer) -> Arc<Fake> {
    Fake::by_path(vec![
        ("/downloadclient", clients),
        ("", Answer::reply(200, "Ok.")),
    ])
}

/// A context seeding the real stack over the given transport, with a
/// qBittorrent password recorded and the given baseline written beside it.
fn schema_ctx(dir: &std::path::Path, baseline: &str, http: Arc<Fake>) -> Ctx {
    const KEYED: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
    let _ = std::fs::create_dir_all(dir);
    let env = dir.join(".env");
    let _ = std::fs::write(&env, "QBITTORRENT_PASSWORD=pw\n");
    let _ = crate::config::store::write(&dir.join("baseline.json"), baseline);
    a_context()
        .settings(Settings {
            env_file: Some(env),
            ..Settings::default()
        })
        .build()
        .with_filesystem(Arc::new(SeedFs::keyed(Some(KEYED), None)))
        .with_http(http)
}

/// The state of the `qBittorrent into Sonarr` wiring in a seed report.
fn qbittorrent_into_sonarr(report: &crate::seed::Report) -> Option<&crate::seed::State> {
    report
        .wirings
        .iter()
        .find(|wiring| wiring.connection == "qBittorrent into Sonarr")
        .map(|wiring| &wiring.state)
}

#[tokio::test]
async fn a_schema_change_re_baselines_rather_than_reporting_mass_drift() {
    // Sonarr moved from version 4 to 5, and its one managed download client now
    // reads a different category — every managed value moved at once. That is the
    // upgrade renaming fields, not the operator editing each, so the current shape
    // is adopted as the new baseline and the wiring reads adopted, not drifted.
    let dir = std::env::temp_dir().join(format!("lemonfiber-schema-adopt-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let baseline = r#"{"services":{"Sonarr":{"schema:version":{"value":"4","at":"1"},"downloadclient:gluetun:8081":{"value":"tv","at":"1"}}}}"#;
    let ctx = schema_ctx(&dir, baseline, versioned("5"));

    let report = seeded(dispatch(Command::Seed, &ctx).await).unwrap_or_default();
    assert_eq!(
        qbittorrent_into_sonarr(&report),
        Some(&crate::seed::State::Adopted),
        "a schema change adopts the current shape rather than reporting drift"
    );
    // The new version is recorded, so the next run compares against it.
    let saved = std::fs::read_to_string(dir.join("baseline.json")).unwrap_or_default();
    assert!(
        saved.contains(r#""schema:version""#) && saved.contains(r#""value":"5""#),
        "the service's new version is recorded: {saved}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn a_version_change_with_only_some_drift_is_left_as_the_operators_edits() {
    // The version changed, but Sonarr never recorded this client — so it reads as
    // the operator's own, unmanaged, not as drift. Not every managed value moved,
    // so it is not a schema change: it is left as it is rather than re-baselined.
    let dir =
        std::env::temp_dir().join(format!("lemonfiber-schema-partial-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let baseline = r#"{"services":{"Sonarr":{"schema:version":{"value":"4","at":"1"}}}}"#;
    let ctx = schema_ctx(&dir, baseline, versioned("5"));

    let report = seeded(dispatch(Command::Seed, &ctx).await).unwrap_or_default();
    assert_eq!(
        qbittorrent_into_sonarr(&report),
        Some(&crate::seed::State::Unmanaged),
        "a version change alone does not re-baseline a value that did not wholesale-drift"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn an_unchanged_version_leaves_a_drift_as_the_drift_it_is() {
    // Sonarr is on the version lemonfiber last recorded, so nothing upgraded — the
    // client that differs is the operator's edit, reported as drift and preserved,
    // not re-baselined.
    let dir = std::env::temp_dir().join(format!("lemonfiber-schema-same-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let baseline = r#"{"services":{"Sonarr":{"schema:version":{"value":"5","at":"1"},"downloadclient:gluetun:8081":{"value":"tv","at":"1"}}}}"#;
    let ctx = schema_ctx(&dir, baseline, versioned("5"));

    let report = seeded(dispatch(Command::Seed, &ctx).await).unwrap_or_default();
    assert_eq!(
        qbittorrent_into_sonarr(&report),
        Some(&crate::seed::State::Drifted),
        "an unchanged version leaves a drift as drift"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// A transport whose download-client list is set per test — a body to return, or
/// nothing to fail the read — with every other call answered plainly. For the
/// reset-connection edge cases, where what the service holds decides the preview.
/// A context for the reset-connection edge cases: the real stack, a recorded
/// qBittorrent password so a client is wanted, keys per `filesystem`, over `http`.
fn reset_ctx(
    dir: &std::path::Path,
    filesystem: Arc<SeedFs>,
    http: Arc<dyn crate::ports::http::Http>,
) -> Ctx {
    let _ = std::fs::create_dir_all(dir);
    let env = dir.join(".env");
    let _ = std::fs::write(&env, "QBITTORRENT_PASSWORD=pw\n");
    a_context()
        .settings(Settings {
            env_file: Some(env),
            ..Settings::default()
        })
        .build()
        .with_filesystem(filesystem)
        .with_http(http)
}

#[tokio::test]
async fn a_reset_skips_an_arr_that_has_not_written_its_key() {
    // A client is wanted, but the \*arr's key is not readable — it has not finished
    // starting — so there is nothing to open and it is passed over rather than reset.
    let dir = std::env::temp_dir().join(format!("lemonfiber-reset-noopen-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let ctx = reset_ctx(&dir, Arc::new(SeedFs::keyed(None, None)), seeding());
    assert!(super::super::reset_connections(&ctx, false)
        .await
        .is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn a_reset_preview_passes_over_a_client_the_service_does_not_hold() {
    const KEYED: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
    // The service holds none of the wanted clients, so there is nothing whose drift
    // to preview — each wanted one is passed over rather than reported.
    let dir = std::env::temp_dir().join(format!("lemonfiber-reset-absent-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let ctx = reset_ctx(
        &dir,
        Arc::new(SeedFs::keyed(Some(KEYED), None)),
        clients_answering(Answer::reply(200, "[]")),
    );
    assert!(super::super::reset_connections(&ctx, false)
        .await
        .is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn a_reset_preview_names_a_client_whose_category_the_operator_changed() {
    const KEYED: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
    // The service holds the wanted client under a category the operator changed from
    // lemonfiber's recorded one — a drift the preview names as one a reset would
    // revert, reading the category the service now holds to judge it.
    let dir = std::env::temp_dir().join(format!("lemonfiber-reset-drift-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let held = r#"[{"id":2,"fields":[{"name":"host","value":"gluetun"},{"name":"port","value":8081},{"name":"tvCategory","value":"shows"}]}]"#;
    let ctx = reset_ctx(
        &dir,
        Arc::new(SeedFs::keyed(Some(KEYED), None)),
        clients_answering(Answer::reply(200, held)),
    );
    let _ = crate::config::store::write(
        &dir.join("baseline.json"),
        r#"{"services":{"Sonarr":{"downloadclient:gluetun:8081":{"value":"tv","at":"1"}}}}"#,
    );
    let preview = super::super::reset_connections(&ctx, false).await;
    assert!(
        preview
            .iter()
            .any(|wiring| wiring.connection.contains("into Sonarr")),
        "a category the operator changed is previewed as a revert"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn a_reset_preview_reads_nothing_where_the_client_list_cannot_be_read() {
    const KEYED: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
    // The service will not answer its client list, so the preview has nothing to
    // compare against and reports nothing rather than guessing.
    let dir = std::env::temp_dir().join(format!("lemonfiber-reset-unread-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let ctx = reset_ctx(
        &dir,
        Arc::new(SeedFs::keyed(Some(KEYED), None)),
        clients_answering(Answer::Silent),
    );
    assert!(super::super::reset_connections(&ctx, false)
        .await
        .is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn a_lost_baseline_is_reported_and_left_for_a_deliberate_re_baseline() {
    // The record is there but does not parse — lost. An ordinary seed cannot judge
    // drift against it, so it reports that drift could not be assessed and leaves
    // the record untouched rather than silently replacing it: re-baselining is the
    // deliberate act of `adopt`, not a side effect of a plain seed.
    let env = config_scratch("baseline-lost");
    let baseline = env.with_file_name("baseline.json");
    let corrupt = "this is not the baseline you are looking for";
    let _ = crate::config::store::write(&baseline, corrupt);

    let ctx = seed_ctx(None, false, Vec::new(), None, Some(env.clone()));
    let report = seeded(dispatch(Command::Seed, &ctx).await).unwrap_or_default();
    assert_eq!(report.assessment, crate::seed::Assessment::Unassessable);

    let read_back = std::fs::read_to_string(&baseline).unwrap_or_default();
    assert_eq!(
        read_back, corrupt,
        "a lost record is left untouched by a plain seed"
    );
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

#[tokio::test]
async fn an_adopt_pass_re_baselines_over_a_lost_record() {
    // Re-baselining is offered, and `adopt` is how it is taken: over a lost record
    // an adopt pass re-forms the baseline from current state, replacing the
    // unreadable file with a readable one, and assesses cleanly.
    let env = config_scratch("baseline-rebaseline");
    let baseline = env.with_file_name("baseline.json");
    let _ = crate::config::store::write(&baseline, "not parseable");

    let ctx = seed_ctx(None, false, Vec::new(), None, Some(env.clone()));
    let report = seeded(dispatch(Command::Adopt, &ctx).await).unwrap_or_default();
    assert_eq!(report.assessment, crate::seed::Assessment::Assessed);

    let read_back = std::fs::read_to_string(&baseline).unwrap_or_default();
    assert!(
        serde_json::from_str::<crate::baseline::Baseline>(&read_back).is_ok(),
        "an adopt pass re-forms the lost record into a readable one: {read_back}"
    );
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

#[tokio::test]
async fn a_baseline_whose_file_cannot_be_read_is_a_loss() {
    // Not every loss is a parse failure: a record whose file cannot even be opened
    // — here a directory standing where the file should be — is a loss too, told
    // apart from a first seed's genuinely absent file.
    let env = config_scratch("baseline-unreadable");
    let baseline = env.with_file_name("baseline.json");
    let _ = std::fs::create_dir_all(&baseline);

    let ctx = seed_ctx(None, false, Vec::new(), None, Some(env.clone()));
    let report = seeded(dispatch(Command::Seed, &ctx).await).unwrap_or_default();
    assert_eq!(report.assessment, crate::seed::Assessment::Unassessable);
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}
