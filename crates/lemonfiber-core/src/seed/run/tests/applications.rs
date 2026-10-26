//! Registering each curator with the indexer manager.

use super::*;
use crate::ports::service::RegisteredApplication;
use crate::test_support::curators::{FILM, MUSIC, TELEVISION};
use crate::test_support::indexer::{answered, registered, searching, taken};

/// A curator's configuration, holding the key it answers to.
const CURATOR_CONFIG: &str = "<Config><ApiKey>the-key</ApiKey></Config>";

/// The moved indexer, holding the television curator as an application that answers it.
fn holding_sonarr() -> Arc<Fake> {
    Fake::by_path(vec![
        (
            searching("applications"),
            answered(&[RegisteredApplication {
                id: "1".to_owned(),
                base_url: "http://sonarr:8989".to_owned(),
            }]),
        ),
        (searching("test_application"), taken()),
    ])
}

/// The wirings whose connection registers a curator into the indexer aggregator's app sync.
fn application_wirings(report: &crate::seed::Report) -> Vec<&crate::seed::Wiring> {
    report
        .wirings
        .iter()
        .filter(|wiring| wiring.connection.contains("indexer sync via"))
        .collect()
}

#[test]
fn the_application_kind_follows_from_the_media() {
    use crate::ports::service::ApplicationKind;
    assert_eq!(
        application_kind(&["tv".to_owned()]),
        Some(ApplicationKind::Tv)
    );
    assert_eq!(
        application_kind(&["movies".to_owned()]),
        Some(ApplicationKind::Movies)
    );
    assert_eq!(
        application_kind(&["music".to_owned()]),
        Some(ApplicationKind::Music)
    );
    // The book curator files books but is not one of the aggregator's applications.
    assert!(application_kind(&["books".to_owned()]).is_none());
    // A service that files no media is not an application at all.
    assert!(application_kind(&[]).is_none());
}

#[tokio::test]
async fn app_sync_does_nothing_where_the_stack_has_no_indexer_aggregator() {
    let ctx = seed_ctx(true, Vec::new(), None, None);
    // Only a media-filing curator, so nothing asks for it at all.
    let wirings =
        super::super::seed_applications(&ctx, &fillers_of(vec![curator("sonarr", 8989, "tv")]))
            .await;
    assert!(wirings.is_empty(), "no Prowlarr, no app sync");
}

/// An indexer whose adapter this machine cannot ask has nothing registered into it and
/// says nothing.
#[tokio::test]
async fn app_sync_passes_over_an_indexer_nothing_can_reach() {
    let http = Fake::always(answered(&Vec::<RegisteredApplication>::new()));
    let ctx = seed_ctx(true, Vec::new(), None, None)
        .with_http(http.clone())
        .with_filesystem(Arc::new(SeedFs::keyed(Some(CURATOR_CONFIG))));

    let wirings =
        super::super::seed_applications(&ctx, &indexed(vec![curator("sonarr", 8989, "tv")], &[]))
            .await;

    assert!(wirings.is_empty(), "{wirings:?}");
    assert!(http.requests().is_empty(), "{:?}", http.requests());
}

/// A plugin's curator is never registered into the indexer, which would hand it the
/// indexer's own key and every indexer behind it: nothing is asked of the indexer about
/// it, the pair is said as reached by nothing with why, and the stack's own curators are
/// registered as ever.
#[tokio::test]
async fn app_sync_never_registers_a_plugin_curator() {
    let http = holding_sonarr();
    let ctx = indexing(http.clone(), SeedFs::keyed(Some(CURATOR_CONFIG)));
    let fillers = indexed(vec![curator("sonarr", 8989, "tv")], &[a_stand_in("movies")]);

    let wirings = super::super::seed_applications(&ctx, &fillers).await;
    let said: Vec<crate::seed::Wiring> = super::super::connecting::unmatched(&fillers)
        .into_iter()
        .filter(|wiring| wiring.connection.starts_with("kept"))
        .collect();

    assert!(
        wirings
            .iter()
            .all(|wiring| wiring.connection.starts_with("sonarr the app")),
        "{wirings:?}"
    );
    assert!(!wirings.is_empty());
    assert!(http.requests().iter().all(|asked| {
        !asked.url.contains("kept") && !asked.body.as_deref().unwrap_or_default().contains("kept")
    }));
    assert!(
        matches!(said.as_slice(), [wiring]
            if wiring.connection == "kept the stand-in into Prowlarr"
                && matches!(&wiring.state, crate::seed::State::Unmatched { reason }
                    if reason.contains("whose credential is handed only to the stack's own"))),
        "{said:?}"
    );
}

/// A plugin's service asking where the stack's indexer would is never handed a curator's
/// key: nothing is asked of it or of any curator, and a replaced key owes it nothing.
#[tokio::test]
async fn app_sync_never_hands_a_plugin_indexer_a_curators_key() {
    let http = seeding();
    let ctx = seed_ctx(true, Vec::new(), None, None)
        .with_http(http.clone())
        .with_filesystem(Arc::new(SeedFs::keyed(Some(CURATOR_CONFIG))));
    let fillers = asked_by_a_plugin(&aggregator_svc(), vec![curator("sonarr", 8989, "tv")]);

    let wirings = super::super::seed_applications(&ctx, &fillers).await;
    let resynced = super::super::resync_application(&ctx, &fillers, "sonarr").await;

    assert!(wirings.is_empty(), "{wirings:?}");
    assert!(resynced.is_empty(), "{resynced:?}");
    assert!(http.requests().is_empty(), "{:?}", http.requests());
}

/// What replacing a curator's key owes each indexer is its application held to the new
/// one, and nothing for a curator no indexer registers.
#[tokio::test]
async fn a_replaced_key_resyncs_each_indexer_that_registers_the_curator() {
    let ctx = indexing(holding_sonarr(), SeedFs::keyed(Some(CURATOR_CONFIG)));
    let fillers = indexed(vec![curator("sonarr", 8989, "tv")], &[]);

    let resynced = super::super::resync_application(&ctx, &fillers, "sonarr").await;
    let nobody = super::super::resync_application(&ctx, &fillers, "radarr").await;

    assert_eq!(
        resynced,
        vec![("Prowlarr".to_owned(), crate::seed::State::AlreadyWired)]
    );
    assert!(nobody.is_empty());
}

/// An indexer that has not written its key yet holds no application to bring up to a
/// replaced key, so a replacement owes it nothing yet.
#[tokio::test]
async fn a_replaced_key_owes_an_indexer_with_no_key_yet_nothing() {
    let ctx = seed_ctx(true, Vec::new(), None, None).with_filesystem(Arc::new(SeedFs::keyed(None)));
    let fillers = fillers_of(vec![aggregator_svc(), curator("sonarr", 8989, "tv")]);

    let resynced = super::super::resync_application(&ctx, &fillers, "sonarr").await;

    assert!(resynced.is_empty(), "{resynced:?}");
}

/// Replacing a plugin's curator's key owes the indexer nothing: it was never registered
/// there, so there is no application to hold to the new key.
#[tokio::test]
async fn a_replaced_key_owes_the_indexer_nothing_for_a_plugin_curator() {
    let http = seeding();
    let ctx = seed_ctx(true, Vec::new(), None, None)
        .with_http(http.clone())
        .with_filesystem(Arc::new(SeedFs::keyed(Some(CURATOR_CONFIG))));
    let fillers = beside_a_stand_in(
        vec![aggregator_svc(), curator("sonarr", 8989, "tv")],
        "movies",
    );

    let resynced = super::super::resync_application(&ctx, &fillers, "kept").await;

    assert!(resynced.is_empty(), "{resynced:?}");
    assert!(http.requests().is_empty());
}

/// An indexer whose adapter holds no key yet cannot be asked, so nothing is registered
/// into it and nothing is asked of it until a later run.
#[tokio::test]
async fn app_sync_registers_nothing_until_the_indexer_can_be_asked() {
    let http = holding_sonarr();
    let mut ctx = seed_ctx(true, Vec::new(), None, None)
        .with_http(http.clone())
        .with_filesystem(Arc::new(SeedFs::keyed(Some(CURATOR_CONFIG))));
    ctx.seams.engine = Arc::new(INDEXER.running());

    let wirings =
        super::super::seed_applications(&ctx, &indexed(vec![curator("sonarr", 8989, "tv")], &[]))
            .await;

    assert!(wirings.is_empty(), "{wirings:?}");
    assert!(http.requests().is_empty(), "{:?}", http.requests());
}

/// The curator has not written its key yet, so its application waits while the
/// indexer itself proceeds.
#[tokio::test]
async fn app_sync_skips_only_the_curator_that_has_not_written_its_key() {
    let ctx = indexing(holding_sonarr(), SeedFs::keyed(None));
    let wirings =
        super::super::seed_applications(&ctx, &indexed(vec![curator("sonarr", 8989, "tv")], &[]))
            .await;
    assert_eq!(wirings.len(), 1);
    assert!(wirings.iter().all(is_skipped));
}

/// The indexer already holds the curator, so the connection reads back as already
/// wired.
#[tokio::test]
async fn app_sync_registers_a_curator_whose_keys_are_all_readable() {
    let ctx = indexing(holding_sonarr(), SeedFs::keyed(Some(CURATOR_CONFIG)));
    let wirings =
        super::super::seed_applications(&ctx, &indexed(vec![curator("sonarr", 8989, "tv")], &[]))
            .await;
    assert_eq!(wirings.len(), 1);
    assert_eq!(
        wirings.first().map(|wiring| &wiring.state),
        Some(&crate::seed::State::AlreadyWired)
    );
}

/// The indexer holds no applications, so the curator is genuinely written and then
/// read back — the write path a pre-populated list would hide.
#[tokio::test]
async fn app_sync_registers_an_absent_curator_and_reads_it_back() {
    let http = Fake::by_path_in_turn(vec![
        (
            searching("applications"),
            vec![
                answered(&Vec::<RegisteredApplication>::new()),
                answered(&[RegisteredApplication {
                    id: "9".to_owned(),
                    base_url: "http://sonarr:8989".to_owned(),
                }]),
            ],
        ),
        (searching("register_application"), vec![taken()]),
    ]);
    let ctx = indexing(http.clone(), SeedFs::keyed(Some(CURATOR_CONFIG)));

    let wirings =
        super::super::seed_applications(&ctx, &indexed(vec![curator("sonarr", 8989, "tv")], &[]))
            .await;
    assert_eq!(wirings.len(), 1);
    assert_eq!(
        wirings.first().map(|wiring| &wiring.state),
        Some(&crate::seed::State::Wired),
        "an absent application is written and confirmed by read-back"
    );

    let told = registered(&http.requests());
    assert_eq!(
        told.iter()
            .map(|one| (
                one.base_url.as_str(),
                one.indexer_url.as_str(),
                one.api_key.as_str()
            ))
            .collect::<Vec<_>>(),
        vec![("http://sonarr:8989", "http://prowlarr:9696", "the-key")]
    );
}

/// The whole command against the real manifest, its indexer and its three media-filing
/// curators moved into their plugins, each curator already registered.
#[tokio::test]
async fn seed_registers_each_curator_into_the_indexer_aggregator() {
    let registered: Vec<RegisteredApplication> = [
        "http://sonarr:8989",
        "http://radarr:7878",
        "http://lidarr:8686",
    ]
    .iter()
    .enumerate()
    .map(|(id, base_url)| RegisteredApplication {
        id: id.to_string(),
        base_url: (*base_url).to_owned(),
    })
    .collect();
    let mut ctx = minting_curators(seed_ctx(
        true,
        Vec::new(),
        Some(vec![0x11; 24]),
        Some(crate::test_support::env_without_password(
            "seed-applications",
        )),
    ))
    .with_http(seeding_with(vec![
        (searching("applications"), answered(&registered)),
        (searching("test_application"), taken()),
    ]));
    INDEXER.moved_in(&ctx);
    ctx.seams.engine = Arc::new(
        crate::test_support::adapted::running(&[TELEVISION, FILM, MUSIC, INDEXER])
            .alongside(crate::test_support::media::running()),
    );

    let report = seeded(dispatch(Command::Seed, &ctx).await).unwrap_or_default();
    let applications = application_wirings(&report);
    assert_eq!(
        applications.len(),
        3,
        "Sonarr, Radarr and Lidarr each registered into Prowlarr"
    );
    assert!(applications
        .iter()
        .all(|wiring| wiring.state == crate::seed::State::AlreadyWired));
}

/// A plugin `indexing` whose service stands in for the stack's indexer over
/// `indexer.search`, beside both curators.
fn contracted_indexer(
    project: &std::path::Path,
    trusted: &[crate::plugin::first_party::FirstParty],
) -> crate::wiring::Fillers {
    fillers_trusting(
        vec![
            curator("sonarr", 8989, "tv"),
            curator("radarr", 7878, "movies"),
        ],
        &[contracted("indexing", "prowlarr", "indexer.search")],
        project,
        trusted,
    )
}

#[tokio::test]
async fn a_first_party_indexer_speaking_the_contract_registers_each_curator_over_it() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("applications-contracted");
    let http = Fake::by_path_in_turn(vec![
        (
            "/lemonfiber/indexer.search/v1/applications",
            vec![
                Answer::reply(200, "[]"),
                Answer::reply(
                    200,
                    r#"[{"id": "1", "base_url": "http://sonarr:8989"}, {"id": "2", "base_url": "http://radarr:7878"}]"#,
                ),
            ],
        ),
        (
            "/lemonfiber/indexer.search/v1/register_application",
            vec![Answer::reply(204, "")],
        ),
    ]);
    let ctx = contracted_ctx(&project, "prowlarr", true, http.clone());

    let wirings = super::super::seed_applications(
        &ctx,
        &contracted_indexer(&project, &first_party("indexing")),
    )
    .await;

    assert_eq!(wirings.len(), 2, "{wirings:?}");
    assert!(
        wirings
            .iter()
            .all(|wiring| wiring.state == crate::seed::State::Wired),
        "{wirings:?}"
    );
    let asked = http.requests();
    assert!(asked.iter().all(|one| one
        .url
        .starts_with("http://127.0.0.1:8080/lemonfiber/indexer.search/v1/")));
    let registered: Vec<String> = asked
        .iter()
        .filter(|one| one.url.ends_with("/register_application"))
        .filter_map(|one| one.body.clone())
        .collect();
    assert_eq!(registered.len(), 2, "{registered:?}");
    assert!(registered.iter().all(|body| body.contains("the-key")));
}

#[tokio::test]
async fn a_contracted_indexer_untrusted_or_unreachable_registers_nothing_and_is_never_the_bundled_one(
) {
    for (tag, trusted, keyed) in [
        ("untrusted", &[][..], true),
        ("unkeyed", &first_party("indexing")[..], false),
    ] {
        let project =
            lemonfiber_fixtures::scratch::Scratch::new(&format!("applications-contracted-{tag}"));
        let http = Fake::always(Answer::reply(200, "[]"));
        let ctx = contracted_ctx(&project, "prowlarr", keyed, http.clone());

        let wirings =
            super::super::seed_applications(&ctx, &contracted_indexer(&project, trusted)).await;

        assert!(
            wirings
                .iter()
                .all(|wiring| !matches!(wiring.state, crate::seed::State::Wired)),
            "{tag}: {wirings:?}"
        );
        assert!(http.requests().is_empty(), "{tag}: {:?}", http.requests());
    }
}

/// An indexer the stack publishes on no address has nothing a curator could reach it back
/// on, so nothing is registered into it and nothing is asked.
#[tokio::test]
async fn an_indexer_on_no_address_is_told_about_nobody() {
    let http = seeding();
    let ctx = seed_ctx(true, Vec::new(), None, None).with_http(http.clone());
    let unreached = manifest_service(
        "prowlarr",
        Some(curator_api(Some("/config/config.xml"))),
        None,
    );
    let fillers = fillers_of(vec![unreached, curator("sonarr", 8989, "tv")]);

    let wirings = super::super::seed_applications(&ctx, &fillers).await;

    assert!(wirings.is_empty(), "{wirings:?}");
    assert!(http.requests().is_empty(), "{:?}", http.requests());
}

/// A plugin's curator is never registered into the stack's own indexer either, since the
/// indexer would hand it its own key: the pair is said as reached by nothing, naming that.
#[tokio::test]
async fn the_stacks_indexer_is_never_registered_a_plugin_curator() {
    let fillers = beside_a_stand_in(
        vec![aggregator_svc(), curator("sonarr", 8989, "tv")],
        "movies",
    );

    let said: Vec<crate::seed::Wiring> = super::super::connecting::unmatched(&fillers)
        .into_iter()
        .filter(|wiring| wiring.connection.starts_with("kept"))
        .collect();

    assert!(
        matches!(said.as_slice(), [wiring]
            if matches!(&wiring.state, crate::seed::State::Unmatched { reason }
                if reason.contains("hands every service it registers its own API key"))),
        "{said:?}"
    );
}
