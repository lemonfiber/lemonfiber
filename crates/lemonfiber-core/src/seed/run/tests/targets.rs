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

#[test]
fn a_sabnzbd_config_path_is_the_config_mount_of_the_one_sabnzbd_service() {
    let project = std::path::Path::new("/opt/lemonfiber/stack");
    let sabnzbd_api = |path: Option<&str>| lemonfiber_manifest::Api {
        kind: lemonfiber_manifest::ApiKind::Sabnzbd,
        key_source: lemonfiber_manifest::KeySource::ConfigIni,
        path: path.map(str::to_owned),
        version: None,
    };
    let services = vec![
        manifest_service("jellyfin", None, Some(8096)),
        manifest_service(
            "sonarr",
            Some(servarr_api(Some("/config/config.xml"))),
            Some(8989),
        ),
        manifest_service(
            "sabnzbd",
            Some(sabnzbd_api(Some("/config/sabnzbd.ini"))),
            Some(8080),
        ),
    ];

    assert_eq!(
        sabnzbd_config_path(&services, project),
        Some(project.join("config/sabnzbd/sabnzbd.ini")),
        "read from where Compose mounts SABnzbd's config"
    );
    assert!(
        sabnzbd_config_path(&[], project).is_none(),
        "no SABnzbd service, no path"
    );
    assert!(
        sabnzbd_config_path(
            &[manifest_service(
                "sabnzbd",
                Some(sabnzbd_api(None)),
                Some(8080)
            )],
            project
        )
        .is_none(),
        "a SABnzbd that declares no config file"
    );
    assert!(
        sabnzbd_config_path(
            &[manifest_service(
                "sabnzbd",
                Some(sabnzbd_api(Some("/data/elsewhere.ini"))),
                Some(8080)
            )],
            project
        )
        .is_none(),
        "a config path outside the /config mount"
    );
}

#[tokio::test]
async fn a_sabnzbd_key_needs_a_project_and_a_sabnzbd_to_read() {
    let ctx = seed_ctx(None, true, Vec::new(), None, None);
    assert!(
        read_sabnzbd_key(&ctx, &[], None).await.is_none(),
        "without a project there is nowhere to read from"
    );
    assert!(
        read_sabnzbd_key(&ctx, &[], Some(std::path::Path::new("/srv/stack")))
            .await
            .is_none(),
        "without a SABnzbd service there is no key"
    );
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
