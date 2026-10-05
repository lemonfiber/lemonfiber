//! Root folders and download clients on each *arr.

use super::*;

/// The wirings whose connection names a root folder.
fn root_folder_wirings(report: &crate::seed::Report) -> Vec<&crate::seed::Wiring> {
    report
        .wirings
        .iter()
        .filter(|wiring| wiring.connection.contains("root folder"))
        .collect()
}

#[tokio::test]
async fn seed_wires_each_arrs_root_folders() {
    // Each application already holds the folders, so each is left as wired. Each
    // \*arr also reads its version for the schema check first; that read decodes a
    // folder list as no status, so a spare reply per \*arr covers it and the
    // version is simply not learned — the folders are what this test reads.
    const FOLDERS: &str = r#"[{"id":1,"path":"/data/media/tv"},{"id":2,"path":"/data/media/movies"},{"id":3,"path":"/data/media/music"}]"#;
    const KEYED: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
    let ctx = seed_ctx(
        None,
        true,
        vec![(200, FOLDERS); 6],
        Some(vec![0x11; 24]),
        None,
    )
    .with_filesystem(Arc::new(SeedFs::keyed(Some(KEYED), None)));

    let report = seeded(dispatch(Command::Seed, &ctx).await).unwrap_or_default();
    let folders = root_folder_wirings(&report);
    assert_eq!(folders.len(), 3, "one root folder per media-filing arr");
    let all_wired = folders
        .iter()
        .all(|wiring| wiring.state == crate::seed::State::AlreadyWired);
    assert!(all_wired, "a folder already present is left wired");
}

#[tokio::test]
async fn seed_skips_arr_root_folders_when_the_key_is_not_readable() {
    // No configuration to read a key from, so the arrs have not finished
    // starting: their folders are skipped for a re-run, not failed.
    let ctx = seed_ctx(None, true, Vec::new(), Some(vec![0x11; 24]), None)
        .with_filesystem(Arc::new(SeedFs::keyed(None, None)));

    let report = seeded(dispatch(Command::Seed, &ctx).await).unwrap_or_default();
    let folders = root_folder_wirings(&report);
    assert_eq!(folders.len(), 3);
    assert!(folders.iter().all(|wiring| is_skipped(wiring)));
}

#[test]
fn no_project_directory_means_no_arrs_to_wire() {
    assert!(servarr_arrs(&[], None).is_empty());
}

#[tokio::test]
async fn the_read_only_filesystem_fake_is_inert_elsewhere() {
    // The fake answers only `read`; the rest are stubs, exercised here so the
    // fake carries no uncovered lines of its own.
    use crate::ports::filesystem::FileSystem;
    let fs = SeedFs::keyed(None, None);
    let path = std::path::Path::new("/x");
    assert!(fs.canonicalize(path).await.is_ok());
    assert!(fs.touch(path).await.is_err());
    assert!(fs.link(path, path).await.is_err());
    assert!(fs.identify(path).await.is_err());
    fs.remove(path).await;
    assert!(fs.read(path).await.is_none());
    fs.write(path, "unused").await;
    assert!(fs.ownership(path).await.is_none());
    let _ = fs.describe(path).await;
}

#[test]
fn a_category_is_named_by_the_media_the_application_files() {
    assert_eq!(
        category_for("tv").map(|category| category.field),
        Some("tvCategory".to_owned())
    );
    assert_eq!(
        category_for("movies").map(|category| category.field),
        Some("movieCategory".to_owned())
    );
    assert_eq!(
        category_for("music").map(|category| category.field),
        Some("musicCategory".to_owned())
    );
    assert!(category_for("comics").is_none());
}

/// Both download clients' credentials, as a pass that read and recorded them holds them.
fn both_held() -> Held {
    Held::from(std::collections::BTreeMap::from([
        ("sabnzbd".to_owned(), Credential::ApiKey("k".to_owned())),
        (
            "qbittorrent".to_owned(),
            Credential::UserPass {
                username: "admin".to_owned(),
                password: "p".to_owned(),
            },
        ),
    ]))
}

/// The clients the shipped stack's Sonarr is told about, with `installed` and `chosen`,
/// filing under `media` — one list per Sonarr, which the shipped stack has one of.
fn told(
    installed: &[crate::plugin::Installed],
    chosen: &crate::wiring::Chosen,
    held: &Held,
    media: &str,
) -> Vec<Vec<crate::ports::service::DownloadClient>> {
    crate::test_support::stack()
        .manifest()
        .map(|manifest| {
            let fillers =
                crate::wiring::Fillers::of(&manifest, installed, chosen, Some(stack_root()));
            servarr_arrs(&manifest.services, Some(stack_root()))
                .into_iter()
                .filter(|arr| arr.target.id == "sonarr")
                .map(|mut arr| {
                    arr.media_types = vec![media.to_owned()];
                    super::super::arrs::wanted_clients(&arr, &fillers, held)
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Where each client is reached, by name, for each \*arr told about them.
fn reached(told: &[Vec<crate::ports::service::DownloadClient>]) -> Vec<Vec<(String, String, u16)>> {
    told.iter()
        .map(|clients| {
            clients
                .iter()
                .map(|client| (client.name.clone(), client.host.clone(), client.port))
                .collect()
        })
        .collect()
}

/// Each \*arr is told about what fills its download asks, at the port each says it
/// answers on beside the others — the torrent client through the tunnel whose network it
/// shares — and only where the credential is in hand.
#[test]
fn a_download_client_is_built_for_each_filler_whose_credential_is_in_hand() {
    let nothing = crate::wiring::Chosen::default();
    let usenet_only = Held::from(std::collections::BTreeMap::from([(
        "sabnzbd".to_owned(),
        Credential::ApiKey("k".to_owned()),
    )]));

    assert_eq!(
        reached(&told(&[], &nothing, &both_held(), "tv")),
        vec![vec![
            ("SABnzbd".to_owned(), "sabnzbd".to_owned(), 8080),
            ("qBittorrent".to_owned(), "gluetun".to_owned(), 8081),
        ]]
    );
    assert_eq!(
        reached(&told(&[], &nothing, &usenet_only, "tv")),
        vec![vec![("SABnzbd".to_owned(), "sabnzbd".to_owned(), 8080)]]
    );
    assert_eq!(
        told(&[], &nothing, &Held::default(), "tv"),
        vec![Vec::new()]
    );
}

/// A plugin chosen to stand in for the Usenet client is told about at its own id and
/// the port it answers on, with its own credential, and nothing else changes.
#[test]
fn a_plugin_standing_in_for_the_usenet_client_is_reached_where_it_answers() {
    let stand_in = crate::test_support::a_placed(
        "nzbget",
        &["download.usenet"],
        Some(lemonfiber_manifest::Api {
            kind: lemonfiber_manifest::ApiKind::Sabnzbd,
            key_source: lemonfiber_manifest::KeySource::ConfigIni,
            path: Some("/config/sabnzbd.ini".to_owned()),
            version: None,
        }),
        Some(6789),
    );
    let installed = [crate::test_support::an_installed("nzbget", vec![stand_in])];
    let chosen = crate::wiring::Chosen::read(Some("download.usenet=nzbget"));
    let mut held = both_held();
    held.keys.insert(
        holder(Some("nzbget"), "nzbget"),
        Credential::ApiKey("its-own".to_owned()),
    );

    let told = told(&installed, &chosen, &held, "tv");

    assert_eq!(
        reached(&told),
        vec![vec![
            ("nzbget the stand-in".to_owned(), "nzbget".to_owned(), 6789),
            ("qBittorrent".to_owned(), "gluetun".to_owned(), 8081),
        ]]
    );
    let credentials: Vec<&Credential> = told
        .iter()
        .filter_map(|clients| clients.first())
        .map(|client| &client.credential)
        .collect();
    assert_eq!(credentials, vec![&Credential::ApiKey("its-own".to_owned())]);
}

/// A filler the \*arr is connected to nothing by is told about as nothing, while the
/// client beside it is told about as ever.
#[test]
fn a_filler_nothing_connects_is_not_told_about() {
    let installed = [crate::test_support::an_installed(
        "unadapted",
        vec![crate::test_support::a_placed(
            "unadapted",
            &["download.usenet"],
            None,
            Some(6789),
        )],
    )];
    let chosen = crate::wiring::Chosen::read(Some("download.usenet=unadapted"));

    assert_eq!(
        reached(&told(&installed, &chosen, &both_held(), "tv")),
        vec![vec![("qBittorrent".to_owned(), "gluetun".to_owned(), 8081)]]
    );
}

/// A plugin's Usenet client whose key file leads away from the directory its container
/// owns is not read, and each \*arr that asks for it is told why on that connection.
#[tokio::test]
async fn a_plugin_client_whose_key_file_leads_away_is_refused_on_its_connection() {
    const SABNZBD: &str = "[misc]\napi_key = the-sab-key\n";
    let stand_in = crate::test_support::a_placed(
        "nzbget",
        &["download.usenet"],
        Some(lemonfiber_manifest::Api {
            kind: lemonfiber_manifest::ApiKind::Sabnzbd,
            key_source: lemonfiber_manifest::KeySource::ConfigIni,
            path: Some("/config/sabnzbd.ini".to_owned()),
            version: None,
        }),
        Some(6789),
    );
    let installed = [crate::test_support::an_installed("nzbget", vec![stand_in])];
    let chosen = crate::wiring::Chosen::read(Some("download.usenet=nzbget"));
    let ctx = seed_ctx(None, true, Vec::new(), None, None).with_filesystem(Arc::new(
        SeedFs::keyed(None, Some(SABNZBD)).leading_away(vec!["config/nzbget/"]),
    ));
    let fillers = crate::test_support::stack()
        .manifest()
        .map(|manifest| {
            crate::wiring::Fillers::of(&manifest, &installed, &chosen, Some(stack_root()))
        })
        .unwrap_or_default();

    let held =
        super::super::clients::held(&ctx, &fillers, &std::collections::BTreeMap::new()).await;
    let refused = super::super::clients::refused(&fillers, &held);

    assert!(
        held.of(&holder(Some("nzbget"), "nzbget")).is_none(),
        "the file was read"
    );
    assert_eq!(
        refused
            .iter()
            .map(|wiring| wiring.connection.as_str())
            .collect::<Vec<&str>>(),
        vec![
            "nzbget the stand-in into Sonarr",
            "nzbget the stand-in into Radarr",
            "nzbget the stand-in into Lidarr",
        ]
    );
    assert!(refused.iter().all(|wiring| matches!(
        &wiring.state,
        crate::seed::State::Refused { reason } if reason.starts_with("nzbget's credential file")
    )));
}

/// An \*arr filing a media type with no category field is told about no client.
#[test]
fn an_arr_with_no_category_is_told_about_no_client() {
    assert_eq!(
        told(
            &[],
            &crate::wiring::Chosen::default(),
            &both_held(),
            "comics"
        ),
        vec![Vec::new()]
    );
}

#[test]
fn a_recorded_qbittorrent_password_is_read_back_or_read_as_absent() {
    // Nowhere to read from.
    let ctx = seed_ctx(None, true, Vec::new(), None, None);
    assert!(recorded_qbittorrent_password(&ctx).is_none());

    let path = config_scratch("qbt-readback");
    let ctx = seed_ctx(None, true, Vec::new(), None, Some(path.to_path_buf()));
    // A file that holds no password of ours.
    let _ = store::set(&path, "SOMETHING_ELSE", "x");
    assert!(
        recorded_qbittorrent_password(&ctx).is_none(),
        "no password recorded"
    );
    // An empty value is not a password.
    let _ = store::set(&path, crate::config::QBITTORRENT_PASSWORD_KEY, "");
    assert!(
        recorded_qbittorrent_password(&ctx).is_none(),
        "an empty value is absent"
    );
    // The value recorded on an earlier run is handed back.
    let _ = store::set(
        &path,
        crate::config::QBITTORRENT_PASSWORD_KEY,
        "minted-earlier",
    );
    assert_eq!(
        recorded_qbittorrent_password(&ctx).as_deref(),
        Some("minted-earlier")
    );
}

#[tokio::test]
async fn seed_leaves_each_arrs_already_present_download_clients() {
    // qBittorrent announces a temporary password, so it is set and its value
    // threaded to the download clients; each arr already holds both clients.
    const SERVARR: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
    const SABNZBD: &str = "[misc]\napi_key = the-sab-key\n";
    let env = config_scratch("seed_leaves_each_arrs_already_present_download_clients");
    let ctx = seed_ctx(
        Some(TEMP_LOG),
        true,
        Vec::new(),
        Some(vec![0x11; 24]),
        Some(env.to_path_buf()),
    )
    .with_http(seeding())
    .with_filesystem(Arc::new(SeedFs::keyed(Some(SERVARR), Some(SABNZBD))));

    let report = seeded(dispatch(Command::Seed, &ctx).await).unwrap_or_default();
    let clients = download_client_wirings(&report);
    assert_eq!(
        clients.len(),
        6,
        "SABnzbd and qBittorrent into each of three arrs"
    );
    // Each client is already registered at its endpoint, so none is written a
    // second time. The service reports no category for them and there is no
    // baseline, so the three-way comparison cannot prove they are lemonfiber's
    // own value: it takes each as the operator's own, pre-existing and unmanaged,
    // and leaves it rather than overwriting it. The point the test guards is that
    // a present client is left, never duplicated.
    let none_rewired = clients
        .iter()
        .all(|wiring| wiring.state == crate::seed::State::Unmanaged);
    assert!(none_rewired, "a present client is left, not re-registered");
}

#[tokio::test]
async fn adopt_runs_the_wiring_and_reports_each_present_client() {
    // The adopt command runs the same wiring as a seed. The mock's present clients
    // report no category and there is no baseline, so each is unmanaged — and with
    // no value to take on, an adopt pass reports it unmanaged just as a seed does,
    // never registering it a second time. The point guarded here is that the adopt
    // command dispatches and reports every present client.
    const SERVARR: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
    const SABNZBD: &str = "[misc]\napi_key = the-sab-key\n";
    let env = config_scratch("adopt_runs_the_wiring_and_reports_each_present_client");
    let ctx = seed_ctx(
        Some(TEMP_LOG),
        true,
        Vec::new(),
        Some(vec![0x22; 24]),
        Some(env.to_path_buf()),
    )
    .with_http(seeding())
    .with_filesystem(Arc::new(SeedFs::keyed(Some(SERVARR), Some(SABNZBD))));

    let report = seeded(dispatch(Command::Adopt, &ctx).await).unwrap_or_default();
    let clients = download_client_wirings(&report);
    assert_eq!(
        clients.len(),
        6,
        "SABnzbd and qBittorrent into each of three arrs"
    );
    let all_reported = clients
        .iter()
        .all(|wiring| wiring.state == crate::seed::State::Unmanaged);
    assert!(all_reported, "an adopt pass reports each present client");
}

#[tokio::test]
async fn seed_skips_download_clients_when_the_arr_key_is_not_readable() {
    // The clients' own credentials are in hand, but the arrs have not written
    // their keys, so registration is skipped for a re-run rather than failed.
    const SABNZBD: &str = "[misc]\napi_key = the-sab-key\n";
    let env = config_scratch("seed_skips_download_clients_when_the_arr_key_is_not_readable");
    let ctx = seed_ctx(
        Some(TEMP_LOG),
        true,
        Vec::new(),
        Some(vec![0x11; 24]),
        Some(env.to_path_buf()),
    )
    .with_http(seeding())
    .with_filesystem(Arc::new(SeedFs::keyed(None, Some(SABNZBD))));

    let report = seeded(dispatch(Command::Seed, &ctx).await).unwrap_or_default();
    let clients = download_client_wirings(&report);
    assert_eq!(clients.len(), 6);
    assert!(clients.iter().all(|wiring| is_skipped(wiring)));
}
