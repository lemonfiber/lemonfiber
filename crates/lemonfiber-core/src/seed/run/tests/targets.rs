//! Which services a pass reaches, and where each keeps its configuration.

use super::*;

#[test]
fn the_project_directory_is_the_external_path_or_the_materialise_target() {
    // Any directory does: what is under test is which path is chosen, not what is
    // in it. `adapters` is named because the crate cannot compile without it — the
    // previous choice was a directory that later moved to its own crate, which broke
    // this at a distance with an error naming neither.
    static EMBEDDED: include_dir::Dir<'_> =
        include_dir::include_dir!("$CARGO_MANIFEST_DIR/src/config");
    assert_eq!(
        project_directory(&Source::External(std::path::Path::new("/srv/stack")), None).as_deref(),
        Some(std::path::Path::new("/srv/stack")),
        "an external stack is its own project root"
    );
    assert_eq!(
        project_directory(
            &Source::Embedded(&EMBEDDED),
            Some(std::path::Path::new("/opt/lemonfiber/stack"))
        )
        .as_deref(),
        Some(std::path::Path::new("/opt/lemonfiber/stack")),
        "an embedded stack's root is wherever it was materialised"
    );
    assert_eq!(
        project_directory(&Source::Embedded(&EMBEDDED), None),
        None,
        "an embedded stack materialised nowhere has no root to read from"
    );
}

#[test]
fn only_reachable_servarr_services_with_a_config_path_become_targets() {
    let project = std::path::Path::new("/opt/lemonfiber/stack");
    let services = vec![
        manifest_service(
            "sonarr",
            Some(servarr_api(Some("/config/config.xml"))),
            Some(8989),
        ),
        manifest_service(
            "sabnzbd",
            Some(lemonfiber_manifest::Api {
                kind: lemonfiber_manifest::ApiKind::Sabnzbd,
                key_source: lemonfiber_manifest::KeySource::ConfigIni,
                path: Some("/config/sabnzbd.ini".to_owned()),
                version: None,
            }),
            Some(8080),
        ),
        manifest_service("jellyfin", None, Some(8096)),
        manifest_service(
            "radarr",
            Some(servarr_api(Some("/config/config.xml"))),
            None,
        ),
        manifest_service(
            "lidarr",
            Some(servarr_api(Some("/data/elsewhere.xml"))),
            Some(8686),
        ),
        manifest_service("prowlarr", Some(servarr_api(None)), Some(9696)),
    ];

    let targets = servarr_targets(&services, Some(project));

    assert_eq!(
        targets.len(),
        1,
        "only the reachable Servarr service qualifies"
    );
    let target = targets.first();
    assert!(
        target.is_some_and(|target| target.id == "sonarr"
            && target.base == "http://127.0.0.1:8989"
            && target.config == project.join("config/sonarr/config.xml")),
        "the key is read from where Compose mounts the service's config"
    );
}

/// What the shipped stack's asks come to on this machine, with nothing installed.
fn shipped_fillers(project: Option<&std::path::Path>) -> crate::wiring::Fillers {
    crate::test_support::stack()
        .manifest()
        .map(|manifest| {
            crate::wiring::Fillers::of(&manifest, &[], &crate::wiring::Chosen::default(), project)
        })
        .unwrap_or_default()
}

/// A Usenet client's key is read from the file its own declaration names, and held
/// against that client rather than against its kind.
#[tokio::test]
async fn a_usenet_clients_key_is_held_against_the_client_it_was_read_from() {
    const SABNZBD: &str = "[misc]\napi_key = the-sab-key\n";
    let ctx = seed_ctx(None, true, Vec::new(), None, None)
        .with_filesystem(Arc::new(SeedFs::keyed(None, Some(SABNZBD))));

    let held = super::super::clients::held(
        &ctx,
        &shipped_fillers(Some(stack_root())),
        &std::collections::BTreeMap::new(),
    )
    .await;

    assert_eq!(
        held.of(&holder(None, "sabnzbd")),
        Some(&Credential::ApiKey("the-sab-key".to_owned()))
    );
}

/// Without a project there is no file beneath it to read a key from, so no Usenet
/// client's key is in hand — and one that has written nothing yet holds none either.
#[tokio::test]
async fn a_usenet_key_needs_a_project_and_a_file_that_holds_one() {
    const SABNZBD: &str = "[misc]\napi_key = the-sab-key\n";
    let ctx = seed_ctx(None, true, Vec::new(), None, None)
        .with_filesystem(Arc::new(SeedFs::keyed(None, Some(SABNZBD))));
    let unwritten = seed_ctx(None, true, Vec::new(), None, None)
        .with_filesystem(Arc::new(SeedFs::keyed(None, None)));

    let nowhere = super::super::clients::held(
        &ctx,
        &shipped_fillers(None),
        &std::collections::BTreeMap::new(),
    )
    .await;
    let nothing_yet = super::super::clients::held(
        &unwritten,
        &shipped_fillers(Some(stack_root())),
        &std::collections::BTreeMap::new(),
    )
    .await;

    assert!(
        nowhere.of(&holder(None, "sabnzbd")).is_none(),
        "read from no project"
    );
    assert!(
        nothing_yet.of(&holder(None, "sabnzbd")).is_none(),
        "read from a file never written"
    );
}

/// A torrent client's password is the one minted for it this run where there is one,
/// and otherwise the one recorded under its own setting.
#[tokio::test]
async fn a_torrent_clients_password_is_the_one_minted_or_recorded_for_it() {
    let path = config_scratch("held-torrent");
    let _ = store::set(
        &path,
        crate::config::QBITTORRENT_PASSWORD_KEY,
        "minted-earlier",
    );
    let ctx = seed_ctx(None, true, Vec::new(), None, Some(path.to_path_buf()));
    let fillers = shipped_fillers(None);

    let recorded =
        super::super::clients::held(&ctx, &fillers, &std::collections::BTreeMap::new()).await;
    let minted = super::super::clients::held(
        &ctx,
        &fillers,
        &std::collections::BTreeMap::from([(holder(None, "qbittorrent"), "minted-now".to_owned())]),
    )
    .await;

    // Compared rather than printed: the values are credentials, and a failing
    // assertion prints its message into the run's log.
    assert!(password_of(&recorded, &holder(None, "qbittorrent"))
        .is_some_and(|held| held == "minted-earlier"));
    assert!(
        password_of(&minted, &holder(None, "qbittorrent")).is_some_and(|held| held == "minted-now")
    );
    assert!(password_of(&minted, &holder(None, "sabnzbd")).is_none());
}

/// The password held for a service, where what is held for it is one.
fn password_of(held: &Held, service: &super::super::clients::Holder) -> Option<String> {
    match held.of(service) {
        Some(Credential::UserPass { password, .. }) => Some(password.clone()),
        Some(Credential::ApiKey(_)) | None => None,
    }
}

/// The bundled torrent client's password is recorded where the tunnel's forwarded-port
/// push reads it, as it always was.
#[test]
fn the_bundled_torrent_clients_password_setting_is_the_one_the_tunnel_reads() {
    let fillers = shipped_fillers(None);
    let setting = fillers
        .service("qbittorrent")
        .and_then(|client| fillers.setting(client, crate::config::PASSWORD_SUFFIX));
    assert_eq!(
        setting.as_deref(),
        Some(crate::config::QBITTORRENT_PASSWORD_KEY)
    );
}

/// A torrent client a plugin brought, whose id would spell a setting lemonfiber keeps
/// for something else, is never handed that setting's value: its own is named apart.
#[tokio::test]
async fn a_plugin_named_after_a_setting_lemonfiber_keeps_is_never_handed_it() {
    let path = config_scratch("held-namesake");
    let _ = store::set(
        &path,
        crate::config::JELLYFIN_ADMIN_PASSWORD_KEY,
        "the-administrator",
    );
    let ctx = seed_ctx(None, true, Vec::new(), None, Some(path.to_path_buf()));
    let namesake = crate::test_support::a_placed(
        "jellyfin-admin",
        &["download.torrent"],
        Some(torrent_api()),
        Some(8082),
    );
    let fillers = crate::test_support::stack()
        .manifest()
        .map(|manifest| {
            crate::wiring::Fillers::of(
                &manifest,
                &[crate::test_support::an_installed(
                    "namesake",
                    vec![namesake],
                )],
                &crate::wiring::Chosen::default(),
                None,
            )
        })
        .unwrap_or_default();

    let held =
        super::super::clients::held(&ctx, &fillers, &std::collections::BTreeMap::new()).await;

    assert!(password_of(&held, &holder(Some("namesake"), "jellyfin-admin")).is_none());
    assert_eq!(
        fillers
            .service("jellyfin-admin")
            .and_then(|client| fillers.setting(client, crate::config::PASSWORD_SUFFIX))
            .as_deref(),
        Some("PLUGIN_NAMESAKE_JELLYFIN__ADMIN_PASSWORD")
    );
}

/// Where even a plugin's own namespace lands on a setting one of the stack's services
/// holds, the setting is refused: nothing is read for the plugin's client, and the pass
/// says so rather than setting a password it would then keep on top of another.
#[tokio::test]
async fn a_plugin_setting_that_lands_on_one_the_stack_holds_is_refused() {
    let path = config_scratch("held-landed");
    let _ = store::set(&path, "PLUGIN_NZBGET_NZBGET_PASSWORD", "the-stacks-own");
    let ctx = seed_ctx(None, true, Vec::new(), None, Some(path.to_path_buf()));
    let fillers = crate::test_support::stack()
        .manifest()
        .map(|mut manifest| {
            // An operator's own stack may name a service anything, including what a
            // plugin's namespace would spell.
            manifest
                .services
                .push(manifest_service("plugin-nzbget-nzbget", None, None));
            crate::wiring::Fillers::of(
                &manifest,
                &[crate::test_support::an_installed(
                    "nzbget",
                    vec![crate::test_support::a_placed(
                        "nzbget",
                        &["download.torrent"],
                        Some(torrent_api()),
                        Some(8082),
                    )],
                )],
                &crate::wiring::Chosen::default(),
                None,
            )
        })
        .unwrap_or_default();

    let held =
        super::super::clients::held(&ctx, &fillers, &std::collections::BTreeMap::new()).await;
    let (wirings, minted) = super::super::clients::seed_passwords(&ctx, &fillers).await;

    assert!(password_of(&held, &holder(Some("nzbget"), "nzbget")).is_none());
    assert!(minted.is_empty());
    let refused: Vec<&crate::seed::Wiring> = wirings
        .iter()
        .filter(|wiring| matches!(wiring.state, crate::seed::State::Refused { .. }))
        .collect();
    // Counted rather than printed: what came back sits beside minted passwords, and a
    // failing assertion prints its message into the run's log.
    assert_eq!(refused.len(), 1);
    assert!(refused
        .iter()
        .all(|wiring| wiring.connection == "nzbget the stand-in web UI password"));
}

/// A torrent client that publishes no port is one this machine cannot reach to set a
/// password on, so it is passed over rather than reported as tried.
#[tokio::test]
async fn a_torrent_client_publishing_no_port_has_no_password_set() {
    let ctx = seed_ctx(None, true, Vec::new(), None, None);
    let fillers = crate::test_support::stack()
        .manifest()
        .map(|mut manifest| {
            manifest
                .services
                .retain(|service| service.id != "qbittorrent");
            crate::wiring::Fillers::of(
                &manifest,
                &[crate::test_support::an_installed(
                    "unpublished",
                    vec![crate::test_support::a_placed(
                        "unpublished",
                        &["download.torrent"],
                        Some(torrent_api()),
                        None,
                    )],
                )],
                &crate::wiring::Chosen::default(),
                None,
            )
        })
        .unwrap_or_default();

    let (wirings, minted) = super::super::clients::seed_passwords(&ctx, &fillers).await;

    assert!(wirings.is_empty());
    assert!(minted.is_empty());
}

/// qBittorrent's adapter, as a plugin's torrent client names it.
fn torrent_api() -> lemonfiber_manifest::Api {
    lemonfiber_manifest::Api {
        kind: lemonfiber_manifest::ApiKind::Qbittorrent,
        key_source: lemonfiber_manifest::KeySource::Generated,
        path: None,
        version: None,
    }
}

#[test]
fn nothing_can_be_proven_without_a_project_directory() {
    let services = vec![manifest_service(
        "sonarr",
        Some(servarr_api(Some("/config/config.xml"))),
        Some(8989),
    )];
    assert!(servarr_targets(&services, None).is_empty());
}

#[tokio::test]
async fn seed_wires_jellyfin_as_seerrs_identity() {
    // The whole command against the real manifest, which has both services;
    // Jellyfin reports its wizard done and no password was recorded, so the
    // household set it up and the identity is skipped for them to complete.
    let ctx = seed_ctx(None, true, Vec::new(), None, None).with_http(household(true, false));
    let report = seeded(dispatch(Command::Seed, &ctx).await).unwrap_or_default();
    let identity = report
        .wirings
        .iter()
        .find(|wiring| wiring.connection.contains("Seerr's identity"));
    assert!(
        identity.is_some_and(is_skipped),
        "an externally set-up Jellyfin leaves the identity for the household"
    );
}

#[test]
fn a_target_carries_the_servarr_api_version() {
    let project = std::path::Path::new("/opt/lemonfiber/stack");
    // Sonarr answers at v3, Lidarr at v1: the version travels with the target
    // rather than being assumed by the client.
    let sonarr = manifest_service(
        "sonarr",
        Some(servarr_api_at(Some("/config/config.xml"), Some(3))),
        Some(8989),
    );
    assert_eq!(
        super::super::target_for(&sonarr, project).map(|target| target.version),
        Some(3)
    );
    let lidarr = manifest_service(
        "lidarr",
        Some(servarr_api_at(Some("/config/config.xml"), Some(1))),
        Some(8686),
    );
    assert_eq!(
        super::super::target_for(&lidarr, project).map(|target| target.version),
        Some(1)
    );
    // A servarr service that names no version cannot be reached at a known
    // path, so it is no target rather than one guessed at the wrong version.
    let versionless = manifest_service(
        "sonarr",
        Some(servarr_api_at(Some("/config/config.xml"), None)),
        Some(8989),
    );
    assert!(super::super::target_for(&versionless, project).is_none());
}

/// A plugin's torrent client under the id one of the stack's own already has is held
/// apart from it: the password minted for the plugin's is never the stack's, and the
/// stack's is never handed to the plugin's.
#[tokio::test]
async fn a_plugin_client_sharing_a_stack_clients_id_never_shares_its_credential() {
    let ctx = seed_ctx(None, true, Vec::new(), None, None);
    let twin = crate::test_support::a_placed(
        "qbittorrent",
        &["download.torrent"],
        Some(torrent_api()),
        Some(8082),
    );
    let fillers = crate::test_support::stack()
        .manifest()
        .map(|manifest| {
            crate::wiring::Fillers::of(
                &manifest,
                &[crate::test_support::an_installed("twin", vec![twin])],
                &crate::wiring::Chosen::default(),
                None,
            )
        })
        .unwrap_or_default();

    let held = super::super::clients::held(
        &ctx,
        &fillers,
        &std::collections::BTreeMap::from([(
            holder(Some("twin"), "qbittorrent"),
            "the-twins".to_owned(),
        )]),
    )
    .await;

    // Compared rather than printed: the values are credentials.
    assert!(password_of(&held, &holder(Some("twin"), "qbittorrent"))
        .is_some_and(|password| password == "the-twins"));
    assert!(password_of(&held, &holder(None, "qbittorrent")).is_none());
}
