//! What lemonfiber last wrote, and telling it apart from what the operator changed.

use super::*;

#[tokio::test]
async fn a_wired_root_folder_the_host_cannot_back_is_a_warning() {
    // The curator files into `/data/media/tv`, but the host directory it resolves to
    // is not there — the operator repointed the data root, or the media directory
    // was never made. The curator imports into a void, so the folder is raised to a
    // warning naming the missing path.
    let filesystem = SeedFs::keyed(None).missing(vec!["media/tv"]);
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
    let filesystem = SeedFs::keyed(None);
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
    let filesystem = SeedFs::keyed(None).missing(vec!["media/tv"]);
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
    // A skipped folder is not one the curator files into, so a missing path there is
    // not yet a break — only the folders the curator actually holds are checked.
    let filesystem = SeedFs::keyed(None).missing(vec!["media/tv"]);
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
async fn a_wired_root_folder_outside_the_data_root_has_no_host_directory_to_check() {
    let filesystem = SeedFs::keyed(None).missing(vec!["elsewhere"]);
    let wanted = [RootFolder {
        path: "/elsewhere/tv".to_owned(),
        media_type: "tv".to_owned(),
    }];
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
    assert!(broken(&wirings).is_none());
}

#[tokio::test]
async fn a_reset_previews_then_reverts_a_drifted_connection() {
    let dir = lemonfiber_fixtures::scratch::Scratch::named("reset-conn");
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    let env = dir.join(".env");
    // The torrent client's minted account makes a torrent download client wanted; a
    // baseline recording lemonfiber's category for it, against the categoryless client
    // the service now reports, reads as the operator's drift to revert.
    let _ = std::fs::write(&env, "");
    let _ = crate::config::store::write(
        &dir.join("baseline.json"),
        r#"{"services":{"Sonarr":{"downloadclient:gluetun:8081":{"value":"tv","at":"1"}}}}"#,
    );

    let context = curating(
        minting_torrent(
            a_context()
                .settings(Settings {
                    env_file: Some(env.clone()),
                    ..Settings::default()
                })
                .build()
                .with_http(seeding_curated(Vec::new())),
        ),
        true,
    );

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

/// A download client a curator holds, filing television under the category the operator
/// changed it to.
fn held_as_shows(id: &str, host: &str, port: u16) -> crate::ports::service::RegisteredClient {
    crate::ports::service::RegisteredClient {
        id: id.to_owned(),
        host: host.to_owned(),
        port,
        category: Some(crate::ports::service::Category {
            field: "tvCategory".to_owned(),
            value: "shows".to_owned(),
        }),
    }
}

/// A seed run whose curators report `version` and hold both download clients under a
/// drifted category.
fn versioned(version: &'static str) -> Arc<Fake> {
    use crate::test_support::curators::answered;
    seeding_curated(vec![
        (
            curated("identity"),
            answered(&crate::ports::service::Identity {
                name: "Sonarr".to_owned(),
                version: version.to_owned(),
            }),
        ),
        (
            curated("test_download_clients"),
            answered(&Vec::<crate::ports::service::ClientProbe>::new()),
        ),
        (
            curated("download_clients"),
            answered(&[
                held_as_shows("1", "sabnzbd", 8080),
                held_as_shows("2", "gluetun", 8081),
            ]),
        ),
    ])
}

/// A transport answering the curators' client lists with `clients`, and taking everything
/// else — the reset previews turn on what that one list says, including its refusal.
fn clients_answering(clients: Answer) -> Arc<Fake> {
    seeding_curated(vec![(curated("download_clients"), clients)])
}

/// A context seeding the real stack over the given transport, with the torrent client
/// moved into its plugin and its account minted, and the given baseline written beside
/// it.
fn schema_ctx(dir: &std::path::Path, baseline: &str, http: Arc<Fake>) -> Ctx {
    let _ = std::fs::create_dir_all(dir);
    let env = dir.join(".env");
    let _ = std::fs::write(&env, "");
    let _ = crate::config::store::write(&dir.join("baseline.json"), baseline);
    curating(
        minting_torrent(
            a_context()
                .settings(Settings {
                    env_file: Some(env),
                    ..Settings::default()
                })
                .build()
                .with_http(http),
        ),
        true,
    )
}

/// The state of the torrent client's wiring into the curator in a seed report.
fn torrent_into_curator(report: &crate::seed::Report) -> Option<&crate::seed::State> {
    report
        .wirings
        .iter()
        .find(|wiring| wiring.connection == "qBittorrent into Sonarr")
        .map(|wiring| &wiring.state)
}

#[tokio::test]
async fn a_schema_change_re_baselines_rather_than_reporting_mass_drift() {
    // The curator moved from version 4 to 5, and its one managed download client now
    // reads a different category — every managed value moved at once. That is the
    // upgrade renaming fields, not the operator editing each, so the current shape
    // is adopted as the new baseline and the wiring reads adopted, not drifted.
    let dir = lemonfiber_fixtures::scratch::Scratch::named("schema-adopt");
    let _ = std::fs::remove_dir_all(&dir);
    let baseline = r#"{"services":{"Sonarr":{"schema:version":{"value":"4","at":"1"},"downloadclient:gluetun:8081":{"value":"tv","at":"1"}}}}"#;
    let ctx = schema_ctx(&dir, baseline, versioned("5"));

    let report = seeded(dispatch(Command::Seed, &ctx).await).unwrap_or_default();
    assert_eq!(
        torrent_into_curator(&report),
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
    // The version changed, but the curator never recorded this client — so it reads as
    // the operator's own, unmanaged, not as drift. Not every managed value moved,
    // so it is not a schema change: it is left as it is rather than re-baselined.
    let dir = lemonfiber_fixtures::scratch::Scratch::named("schema-partial").kept();
    let _ = std::fs::remove_dir_all(&dir);
    let baseline = r#"{"services":{"Sonarr":{"schema:version":{"value":"4","at":"1"}}}}"#;
    let ctx = schema_ctx(&dir, baseline, versioned("5"));

    let report = seeded(dispatch(Command::Seed, &ctx).await).unwrap_or_default();
    assert_eq!(
        torrent_into_curator(&report),
        Some(&crate::seed::State::Unmanaged),
        "a version change alone does not re-baseline a value that did not wholesale-drift"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn an_unchanged_version_leaves_a_drift_as_the_drift_it_is() {
    // The curator is on the version lemonfiber last recorded, so nothing upgraded — the
    // client that differs is the operator's edit, reported as drift and preserved,
    // not re-baselined.
    let dir = lemonfiber_fixtures::scratch::Scratch::named("schema-same");
    let _ = std::fs::remove_dir_all(&dir);
    let baseline = r#"{"services":{"Sonarr":{"schema:version":{"value":"5","at":"1"},"downloadclient:gluetun:8081":{"value":"tv","at":"1"}}}}"#;
    let ctx = schema_ctx(&dir, baseline, versioned("5"));

    let report = seeded(dispatch(Command::Seed, &ctx).await).unwrap_or_default();
    assert_eq!(
        torrent_into_curator(&report),
        Some(&crate::seed::State::Drifted),
        "an unchanged version leaves a drift as drift"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// A transport whose download-client list is set per test — a body to return, or
/// nothing to fail the read — with every other call answered plainly. For the
/// reset-connection edge cases, where what the service holds decides the preview.
/// A context for the reset-connection edge cases: the real stack, the torrent client moved
/// into its plugin with its account minted so a client is wanted, every curator moved into
/// its plugin with its adapter keyed where `keyed`, over `http`.
fn reset_ctx(dir: &std::path::Path, keyed: bool, http: Arc<dyn crate::ports::http::Http>) -> Ctx {
    let _ = std::fs::create_dir_all(dir);
    let env = dir.join(".env");
    let _ = std::fs::write(&env, "");
    curating(
        minting_torrent(
            a_context()
                .settings(Settings {
                    env_file: Some(env),
                    ..Settings::default()
                })
                .build()
                .with_http(http),
        ),
        keyed,
    )
}

/// A register of installed plugins that is there and will not read is a machine that
/// cannot say what fills its asks, so a reset puts back nothing rather than putting
/// connections back to a guess.
#[tokio::test]
async fn a_reset_over_an_unreadable_register_puts_back_nothing() {
    let dir = lemonfiber_fixtures::scratch::Scratch::named("reset-unread");
    let _ = std::fs::remove_dir_all(&dir);
    let context = reset_ctx(&dir, true, seeding_curated(Vec::new()));
    let _ = crate::config::store::write(
        &dir.join("baseline.json"),
        r#"{"services":{"Sonarr":{"downloadclient:gluetun:8081":{"value":"tv","at":"1"}}}}"#,
    );
    let readable = super::super::reset_connections(&context, false).await;
    let _ = std::fs::write(dir.join(crate::config::paths::PLUGINS), "not a register");

    let unread = super::super::reset_connections(&context, false).await;

    assert!(
        !readable.is_empty(),
        "the drift is previewed while the register reads"
    );
    assert!(unread.is_empty(), "{unread:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A curator no download client's credential is held for wants no client, so a reset
/// passes it over however its baseline reads.
#[tokio::test]
async fn a_reset_passes_over_a_curator_no_client_is_held_for() {
    let dir = lemonfiber_fixtures::scratch::Scratch::named("reset-unheld");
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    let env = dir.join(".env");
    let _ = std::fs::write(&env, "");
    let _ = crate::config::store::write(
        &dir.join("baseline.json"),
        r#"{"services":{"Sonarr":{"downloadclient:gluetun:8081":{"value":"tv","at":"1"}}}}"#,
    );
    let ctx = curating(
        a_context()
            .settings(Settings {
                env_file: Some(env),
                ..Settings::default()
            })
            .build()
            .with_http(seeding_curated(Vec::new())),
        true,
    );

    let preview = super::super::reset_connections(&ctx, false).await;

    assert!(preview.is_empty(), "{preview:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn a_reset_skips_a_curator_that_has_not_written_its_key() {
    // A client is wanted, but the curator's key is not readable — it has not finished
    // starting — so there is nothing to open and it is passed over rather than reset.
    let dir = lemonfiber_fixtures::scratch::Scratch::named("reset-noopen");
    let _ = std::fs::remove_dir_all(&dir);
    let ctx = reset_ctx(&dir, false, seeding_curated(Vec::new()));
    assert!(super::super::reset_connections(&ctx, false)
        .await
        .is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn a_reset_preview_passes_over_a_client_the_service_does_not_hold() {
    // The service holds none of the wanted clients, so there is nothing whose drift
    // to preview — each wanted one is passed over rather than reported.
    let dir = lemonfiber_fixtures::scratch::Scratch::named("reset-absent");
    let _ = std::fs::remove_dir_all(&dir);
    let ctx = reset_ctx(
        &dir,
        true,
        clients_answering(crate::test_support::curators::answered(&Vec::<
            crate::ports::service::RegisteredClient,
        >::new())),
    );
    assert!(super::super::reset_connections(&ctx, false)
        .await
        .is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn a_reset_preview_names_a_client_whose_category_the_operator_changed() {
    // The service holds the wanted client under a category the operator changed from
    // lemonfiber's recorded one — a drift the preview names as one a reset would
    // revert, reading the category the service now holds to judge it.
    let dir = lemonfiber_fixtures::scratch::Scratch::named("reset-drift");
    let _ = std::fs::remove_dir_all(&dir);
    let held = crate::test_support::curators::answered(&[held_as_shows("2", "gluetun", 8081)]);
    let ctx = reset_ctx(&dir, true, clients_answering(held));
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
    // The service will not answer its client list, so the preview has nothing to
    // compare against and reports nothing rather than guessing.
    let dir = lemonfiber_fixtures::scratch::Scratch::named("reset-silent");
    let _ = std::fs::remove_dir_all(&dir);
    let ctx = reset_ctx(
        &dir,
        true,
        clients_answering(crate::test_support::curators::unavailable()),
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

    let ctx = seed_ctx(false, None, Some(env.to_path_buf()));
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

    let ctx = seed_ctx(false, None, Some(env.to_path_buf()));
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

    let ctx = seed_ctx(false, None, Some(env.to_path_buf()));
    let report = seeded(dispatch(Command::Seed, &ctx).await).unwrap_or_default();
    assert_eq!(report.assessment, crate::seed::Assessment::Unassessable);
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

#[tokio::test]
async fn a_reset_over_a_stack_that_cannot_be_read_puts_back_nothing() {
    let ctx = crate::test_support::a_context()
        .over(crate::test_support::nowhere())
        .build();

    assert!(super::super::reset_connections(&ctx, true).await.is_empty());
}

#[tokio::test]
async fn each_managed_wiring_carries_what_was_last_recorded_for_its_client() {
    let dir = lemonfiber_fixtures::scratch::Scratch::named("managed-recorded");
    let _ = std::fs::remove_dir_all(&dir);
    let ctx = reset_ctx(&dir, true, seeding_curated(Vec::new()));
    let _ = crate::config::store::write(
        &dir.join("baseline.json"),
        r#"{"services":{"Sonarr":{"downloadclient:gluetun:8081":{"value":"tv","at":"1"}}}}"#,
    );
    let installed = crate::app::plugins::read(&ctx)
        .map(|register| register.installed().to_vec())
        .unwrap_or_default();
    let project = project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref());

    let managed = match ctx.stack.checked_manifest(ctx.today()) {
        Ok(manifest) => {
            super::super::managed_wirings(&ctx, &manifest, &installed, project.as_deref()).await
        }
        Err(_) => Vec::new(),
    };

    let recorded: Vec<(String, Option<&str>)> = managed
        .iter()
        .filter(|one| one.name == "Sonarr")
        .flat_map(|one| one.clients.iter())
        .map(|wired| {
            (
                format!("{}:{}", wired.want.host, wired.want.port),
                wired.recorded.as_ref().map(|record| record.value.as_str()),
            )
        })
        .collect();
    assert_eq!(recorded, vec![("gluetun:8081".to_owned(), Some("tv"))]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_media_type_the_vocabulary_names_is_filed_one_folder_beneath_the_media_directory() {
    assert_eq!(
        super::super::baseline::media_folder("tv").as_deref(),
        Some("/data/media/tv")
    );
}

#[test]
fn a_media_type_outside_the_vocabulary_or_leaving_the_media_directory_has_no_folder() {
    for refused in [
        "..",
        "../escaped",
        "tv/../../escaped",
        "/etc",
        "%2e%2e",
        "%2e%2e%2fescaped",
        "tv/extra",
        "",
        "shows",
    ] {
        assert_eq!(
            super::super::baseline::media_folder(refused),
            None,
            "{refused:?}"
        );
    }
}

#[test]
fn a_curator_wants_no_root_folder_for_a_media_type_outside_the_vocabulary() {
    let wanted = super::super::baseline::wanted_roots(&[
        "tv".to_owned(),
        "../escaped".to_owned(),
        "/etc".to_owned(),
    ]);

    assert_eq!(wanted, vec![root("tv")]);
}
