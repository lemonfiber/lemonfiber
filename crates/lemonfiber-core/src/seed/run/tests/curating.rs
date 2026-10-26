//! Root folders and download clients on each curator.

use super::*;
use crate::ports::service::Client as _;

/// The wirings whose connection names a root folder.
fn root_folder_wirings(report: &crate::seed::Report) -> Vec<&crate::seed::Wiring> {
    report
        .wirings
        .iter()
        .filter(|wiring| wiring.connection.contains("root folder"))
        .collect()
}

/// Each curator already holds the folders, so each is left as wired.
#[tokio::test]
async fn seed_wires_each_curators_root_folders() {
    let ctx = curating(
        seed_ctx(
            true,
            Some(vec![0x11; 24]),
            Some(crate::test_support::env_without_password("roots-held")),
        ),
        true,
    )
    .with_http(seeding_curated(Vec::new()));

    let report = seeded(dispatch(Command::Seed, &ctx).await).unwrap_or_default();
    let folders = root_folder_wirings(&report);
    assert_eq!(folders.len(), 3, "one root folder per curator");
    let all_wired = folders
        .iter()
        .all(|wiring| wiring.state == crate::seed::State::AlreadyWired);
    assert!(all_wired, "a folder already present is left wired");
}

/// Each root folder a curator lacks has its directory made under the recorded data
/// root before it is registered, and one whose directory cannot be made is failed
/// naming the host path rather than handed to the curator.
#[tokio::test]
async fn seed_makes_each_root_folders_directory_under_the_data_root_first() {
    let env = config_scratch("roots-made");
    let _ = store::write(&env, "DATA_ROOT=/srv/data\n");
    let ctx = curating(
        a_context()
            .settings(Settings {
                env_file: Some(env.to_path_buf()),
                ..Settings::default()
            })
            .build(),
        true,
    )
    .with_filesystem(Arc::new(
        SeedFs::keyed(None).missing(vec!["/srv/data/media/"]),
    ))
    .with_http(seeding_curated(vec![(
        curated("root_folders"),
        crate::test_support::curators::answered(
            &Vec::<crate::ports::service::RegisteredFolder>::new(),
        ),
    )]));

    let report = seeded(dispatch(Command::Seed, &ctx).await).unwrap_or_default();
    let folders = root_folder_wirings(&report);
    assert_eq!(folders.len(), 3);
    for wiring in folders {
        assert!(
            matches!(&wiring.state, State::Failed { detail } if detail.contains("/srv/data/media/")),
            "{wiring:?}"
        );
    }
}

/// No adapter holds a key yet, so the curators cannot be asked: their folders are
/// skipped for a re-run, not failed.
#[tokio::test]
async fn seed_skips_curator_root_folders_when_the_key_is_not_readable() {
    let ctx = curating(
        seed_ctx(
            true,
            Some(vec![0x11; 24]),
            Some(crate::test_support::env_without_password("roots-unkeyed")),
        ),
        false,
    );

    let report = seeded(dispatch(Command::Seed, &ctx).await).unwrap_or_default();
    let folders = root_folder_wirings(&report);
    assert_eq!(folders.len(), 3);
    assert!(folders.iter().all(|wiring| is_skipped(wiring)));
}

#[test]
fn no_project_directory_means_no_curators_to_wire() {
    let fillers = crate::test_support::stack()
        .manifest()
        .map(|manifest| {
            crate::wiring::Fillers::of(&manifest, &[], &crate::wiring::Chosen::default(), None)
        })
        .unwrap_or_default();
    assert!(curators(&fillers).is_empty());
}

#[tokio::test]
async fn the_read_only_filesystem_fake_is_inert_elsewhere() {
    // The fake answers only `read`; the rest are stubs, exercised here so the
    // fake carries no uncovered lines of its own.
    use crate::ports::filesystem::FileSystem;
    let fs = SeedFs::keyed(None);
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
    assert_eq!(
        category_for("books").map(|category| category.field),
        Some("category".to_owned())
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

/// The clients the shipped stack's TV curator is told about, with `installed` and `chosen`,
/// filing under `media` — one list per TV curator, which the shipped stack has one of.
fn told(
    installed: &[crate::plugin::Installed],
    chosen: &crate::wiring::Chosen,
    held: &Held,
    media: &str,
) -> Vec<Vec<crate::ports::service::DownloadClient>> {
    crate::test_support::stack()
        .manifest()
        .map(|manifest| {
            let mut moved = crate::test_support::curators::TELEVISION.installed();
            for placed in moved
                .iter_mut()
                .flat_map(|plugin| plugin.services.iter_mut())
                .filter(|placed| !placed.media_types.is_empty())
            {
                placed.media_types = vec![media.to_owned()];
            }
            let installed: Vec<_> = moved.into_iter().chain(installed.iter().cloned()).collect();
            let fillers =
                crate::wiring::Fillers::of(&manifest, &installed, chosen, Some(stack_root()));
            curators(&fillers)
                .iter()
                .filter(|curator| curator.id() == "sonarr")
                .map(|curator| super::super::curating::wanted_clients(*curator, &fillers, held))
                .collect()
        })
        .unwrap_or_default()
}

/// Where each client is reached, by name, for each curator told about them.
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

/// Each curator is told about what fills its download asks, at the port each says it
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

/// A third-party plugin chosen to stand in for the Usenet client is not told to a curator
/// moved into its first-party plugin: its credential does not cross to another plugin's
/// service, and the client beside it is told about as ever.
#[test]
fn a_third_party_usenet_stand_in_is_not_told_to_a_moved_curator() {
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

    assert_eq!(
        reached(&told(&installed, &chosen, &held, "tv")),
        vec![vec![("qBittorrent".to_owned(), "gluetun".to_owned(), 8081)]]
    );
}

/// A filler the curator is connected to nothing by is told about as nothing, while the
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

/// A curator filing a media type with no category field is told about no client.
#[test]
fn a_curator_with_no_category_is_told_about_no_client() {
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
fn a_recorded_password_is_read_back_or_read_as_absent() {
    // Nowhere to read from.
    let ctx = seed_ctx(true, None, None);
    assert!(recorded_secret(&ctx, crate::config::TORRENT_PASSWORD_KEY).is_none());

    let path = config_scratch("qbt-readback");
    let ctx = seed_ctx(true, None, Some(path.to_path_buf()));
    // A file that holds no password of ours.
    let _ = store::set(&path, "SOMETHING_ELSE", "x");
    assert!(
        recorded_secret(&ctx, crate::config::TORRENT_PASSWORD_KEY).is_none(),
        "no password recorded"
    );
    // An empty value is not a password.
    let _ = store::set(&path, crate::config::TORRENT_PASSWORD_KEY, "");
    assert!(
        recorded_secret(&ctx, crate::config::TORRENT_PASSWORD_KEY).is_none(),
        "an empty value is absent"
    );
    // The value recorded on an earlier run is handed back.
    let _ = store::set(&path, crate::config::TORRENT_PASSWORD_KEY, "minted-earlier");
    assert_eq!(
        recorded_secret(&ctx, crate::config::TORRENT_PASSWORD_KEY).as_deref(),
        Some("minted-earlier")
    );
}

#[tokio::test]
async fn seed_leaves_each_curators_already_present_download_clients() {
    // Both download clients are moved into their plugins with what lemonfiber minted for
    // each in hand; each curator already holds both clients.
    let env = config_scratch("seed_leaves_each_curators_already_present_download_clients");
    let ctx = seed_ctx(true, Some(vec![0x11; 24]), Some(env.to_path_buf()))
        .with_http(seeding_curated(Vec::new()));
    let ctx = curating(minting_torrent(minting_usenet(ctx)), true);

    let report = seeded(dispatch(Command::Seed, &ctx).await).unwrap_or_default();
    let clients = download_client_wirings(&report);
    assert_eq!(
        clients.len(),
        6,
        "SABnzbd and qBittorrent into each of three curators"
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
    let env = config_scratch("adopt_runs_the_wiring_and_reports_each_present_client");
    let ctx = seed_ctx(true, Some(vec![0x22; 24]), Some(env.to_path_buf()))
        .with_http(seeding_curated(Vec::new()));
    let ctx = curating(minting_torrent(minting_usenet(ctx)), true);

    let report = seeded(dispatch(Command::Adopt, &ctx).await).unwrap_or_default();
    let clients = download_client_wirings(&report);
    assert_eq!(
        clients.len(),
        6,
        "SABnzbd and qBittorrent into each of three curators"
    );
    let all_reported = clients
        .iter()
        .all(|wiring| wiring.state == crate::seed::State::Unmanaged);
    assert!(all_reported, "an adopt pass reports each present client");
}

/// The clients' own credentials are in hand, but no curator's adapter can be asked
/// yet, so registration is skipped for a re-run rather than failed.
#[tokio::test]
async fn seed_skips_download_clients_when_the_curator_key_is_not_readable() {
    let env = config_scratch("seed_skips_download_clients_when_the_curator_key_is_not_readable");
    let ctx = seed_ctx(true, Some(vec![0x11; 24]), Some(env.to_path_buf()))
        .with_http(seeding_curated(Vec::new()));
    let ctx = curating(minting_torrent(minting_usenet(ctx)), false);

    let report = seeded(dispatch(Command::Seed, &ctx).await).unwrap_or_default();
    let clients = download_client_wirings(&report);
    assert_eq!(clients.len(), 6);
    assert!(clients.iter().all(|wiring| is_skipped(wiring)));
}

/// A plugin `curating` whose service `shows` files `media` over `library.curate` and
/// declares the bundled curator's adapter as well, beside the stack's own curators.
fn contracted_curator(
    project: &std::path::Path,
    media: &[&str],
) -> (crate::plugin::Installed, crate::wiring::Fillers) {
    let mut installed = contracted("curating", "shows", "library.curate");
    for placed in &mut installed.services {
        placed.media_types = media.iter().map(|one| (*one).to_owned()).collect();
        placed.api = Some(curator_api(Some("/config/config.xml")));
    }
    let fillers = fillers_trusting(
        vec![curator("sonarr", 8989, "tv")],
        std::slice::from_ref(&installed),
        project,
        &first_party("curating"),
    );
    (installed, fillers)
}

#[tokio::test]
async fn a_curator_speaking_the_contract_is_asked_over_it_and_never_as_the_bundled_one() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("curators-contracted");
    let (_, fillers) = contracted_curator(&project, &["tv"]);
    let http = Fake::always(Answer::reply(200, "[]"));
    let ctx = contracted_ctx(&project, "shows", true, http.clone());

    let listed: Vec<&str> = curators(&fillers).iter().map(|one| one.id()).collect();
    assert_eq!(listed, vec!["shows"], "{listed:?}");
    let Some(shows) = curators(&fillers)
        .into_iter()
        .find(|one| one.id() == "shows")
    else {
        unreachable!("the contracted curator is listed");
    };
    let folders = match shows.client(&ctx).await {
        Some(client) => client.root_folders().await.ok(),
        None => None,
    };
    assert_eq!(folders.map(|held| held.len()), Some(0));
    let asked = http.request();
    assert_eq!(
        asked.as_ref().map(|asked| asked.url.as_str()),
        Some("http://127.0.0.1:8080/lemonfiber/library.curate/v1/root_folders")
    );
    assert_eq!(
        asked.map(|asked| asked.headers).and_then(|headers| {
            headers
                .into_iter()
                .find(|(name, _)| name == "Authorization")
                .map(|(_, value)| value)
        }),
        Some("Bearer contracted-key".to_owned())
    );

    let unkeyed = lemonfiber_fixtures::scratch::Scratch::new("curators-contracted-unkeyed");
    let (_, fillers) = contracted_curator(&unkeyed, &["tv"]);
    let http = Fake::always(Answer::reply(200, "[]"));
    let ctx = contracted_ctx(&unkeyed, "shows", false, http.clone());
    let shows = curators(&fillers)
        .into_iter()
        .find(|one| one.id() == "shows");
    let reached = match shows {
        Some(shows) => shows.client(&ctx).await.is_some(),
        None => true,
    };
    assert!(!reached);
    assert!(http.request().is_none());
}

#[test]
fn a_contracted_service_filing_no_media_is_no_curator() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("curators-contracted-no-media");
    let (_, fillers) = contracted_curator(&project, &[]);
    assert!(curators(&fillers).iter().all(|one| one.id() != "shows"));
}

/// A plugin's service under the id of one of the stack's curators, speaking no contract, is
/// no curator and is told about none of the clients: only the moved curator is told.
#[test]
fn a_plugin_under_a_stack_curators_id_is_told_about_none_of_its_clients() {
    let mut impostor = crate::test_support::a_placed(
        "sonarr",
        &["library.curate"],
        Some(curator_api(Some("/config/config.xml"))),
        Some(8989),
    );
    impostor.media_types = vec!["tv".to_owned()];
    let installed = [crate::test_support::an_installed(
        "impostor",
        vec![impostor],
    )];

    let told = told(
        &installed,
        &crate::wiring::Chosen::default(),
        &both_held(),
        "tv",
    );

    assert_eq!(
        told.iter().map(Vec::len).collect::<Vec<_>>(),
        vec![2],
        "{told:?}"
    );
}

/// A record of what is installed that will not read refuses the seed rather than letting
/// it wire past a contest nobody could see.
#[tokio::test]
async fn a_seed_over_a_record_of_plugins_that_will_not_read_is_refused() {
    let env = config_scratch("seed-unread-register");
    let kept = env.with_file_name(crate::config::paths::PLUGINS);
    let _ = std::fs::create_dir_all(kept.parent().unwrap_or(&kept));
    let _ = std::fs::write(&kept, "not a register");
    let ctx = seed_ctx(true, None, Some(env.to_path_buf())).with_filesystem(Arc::new(
        SeedFs::keyed(Some(crate::test_support::KEYED_CURATOR)),
    ));

    let refused = dispatch(Command::Seed, &ctx).await;

    assert!(
        refused.is_err_and(|problem| problem.summary.contains("record of what is installed")),
        "a seed wired past a record it could not read"
    );
}

/// The shipped stack with its book curator moved into its plugin.
fn books_moved() -> crate::wiring::Fillers {
    crate::test_support::stack()
        .manifest()
        .map(|manifest| {
            crate::wiring::Fillers::of(
                &manifest,
                &crate::test_support::books::moved()
                    .into_iter()
                    .collect::<Vec<_>>(),
                &crate::wiring::Chosen::default(),
                Some(stack_root()),
            )
        })
        .unwrap_or_default()
}

/// The moved book curator is a curator under its own name, asked over `library.curate`
/// through the adapter in front of it; the stack's own, which no adapter fronts, is none.
#[tokio::test]
async fn the_moved_book_curator_is_asked_through_the_adapter_in_front_of_it() {
    use crate::test_support::books;
    let fillers = books_moved();
    let http = Fake::by_path(vec![books::root_folders(&[])]);
    let ctx = books::asking(seed_ctx(true, None, None)).with_http(http.clone());

    let curating = curators(&fillers);
    let bindery = curating.iter().find(|one| one.id() == books::PLUGIN);
    let folders = match bindery {
        Some(bindery) => match bindery.client(&ctx).await {
            Some(client) => client.root_folders().await.ok(),
            None => None,
        },
        None => None,
    };

    assert_eq!(
        bindery.map(|one| (one.name(), one.media_types())),
        Some(("Bindery", ["books".to_owned()].as_slice()))
    );
    assert_eq!(folders.map(|held| held.len()), Some(0));
    assert_eq!(
        http.request().map(|asked| asked.url),
        Some(format!(
            "http://127.0.0.1:8080{}",
            books::curating("root_folders")
        ))
    );
    assert!(curators(&fillers_of(vec![book_curator_svc()])).is_empty());
}

/// The moved book curator is told about both download clients over its contract, each
/// filing under the books category in the field the book curator names it.
#[test]
fn the_moved_book_curator_is_told_both_clients_under_the_books_category() {
    let fillers = books_moved();
    let told: Vec<Vec<crate::ports::service::DownloadClient>> = curators(&fillers)
        .iter()
        .filter(|curator| curator.id() == crate::test_support::books::PLUGIN)
        .map(|curator| super::super::curating::wanted_clients(*curator, &fillers, &both_held()))
        .collect();

    assert_eq!(
        told.iter()
            .map(|clients| clients
                .iter()
                .map(|client| (
                    client.name.as_str(),
                    client.category.field.as_str(),
                    client.category.value.as_str()
                ))
                .collect::<Vec<_>>())
            .collect::<Vec<_>>(),
        vec![vec![
            ("SABnzbd", "category", "books"),
            ("qBittorrent", "category", "books"),
        ]]
    );
}

/// A seed of the moved book curator registers its books folder and both download clients
/// through the adapter in front of it.
#[tokio::test]
async fn the_moved_book_curator_is_seeded_its_folder_and_clients_over_its_contract() {
    use crate::test_support::books;
    let fillers = books_moved();
    let registered = |host: &str, port: u16| crate::ports::service::RegisteredClient {
        id: host.to_owned(),
        host: host.to_owned(),
        port,
        category: category_for("books"),
    };
    let http = Fake::by_path_in_turn(vec![
        (
            books::curating("root_folders"),
            vec![
                books::answered(&Vec::<crate::ports::service::RegisteredFolder>::new()),
                books::answered(&[crate::ports::service::RegisteredFolder {
                    id: "1".to_owned(),
                    path: "/data/media/books".to_owned(),
                }]),
            ],
        ),
        (
            books::curating("download_clients"),
            vec![
                books::answered(&Vec::<crate::ports::service::RegisteredClient>::new()),
                books::answered(&[registered("sabnzbd", 8080)]),
                books::answered(&[registered("sabnzbd", 8080), registered("gluetun", 8081)]),
            ],
        ),
        (
            books::curating("register_root_folder"),
            vec![books::taken()],
        ),
        (
            books::curating("register_download_client"),
            vec![books::taken(), books::taken()],
        ),
        (books::curating("identity"), vec![books::unavailable()]),
    ]);
    let ctx = books::asking(seed_ctx(true, None, None)).with_http(http.clone());
    let curating = curators(&fillers);
    let contested = super::super::curating::contested(&curating);
    let held = both_held();
    let expected = crate::baseline::Baseline::new();
    let seeding = super::super::curating::CuratorSeeding {
        contested: &contested,
        fillers: &fillers,
        held: &held,
        data_root: None,
        expected: &expected,
        adopt: false,
    };
    let Some(bindery) = curating.iter().find(|one| one.id() == books::PLUGIN) else {
        unreachable!("the moved book curator is a curator");
    };

    let (wirings, _) = super::super::curating::seed_curator(&ctx, *bindery, &seeding).await;

    assert!(
        !wirings.is_empty()
            && wirings
                .iter()
                .all(|wiring| wiring.state == crate::seed::State::Wired),
        "{wirings:?}"
    );
    let sent = http.requests();
    assert_eq!(
        books::folders_registered(&sent)
            .iter()
            .map(|folder| folder.path.as_str())
            .collect::<Vec<_>>(),
        vec!["/data/media/books"]
    );
    assert_eq!(
        books::clients_registered(&sent)
            .iter()
            .map(|client| (client.host.as_str(), client.category.value.as_str()))
            .collect::<Vec<_>>(),
        vec![("sabnzbd", "books"), ("gluetun", "books")]
    );
}
