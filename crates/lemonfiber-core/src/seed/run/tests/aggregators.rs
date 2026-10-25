//! Telling the book curator where its metadata aggregator is.

use super::*;

/// The stack's indexer, answering the book curator's ask for one.
fn indexer() -> lemonfiber_manifest::Service {
    let mut indexer = aggregator_svc();
    indexer.provides = vec!["indexer.search".to_owned()];
    indexer
}

/// What the pass tells each book curator among `services`, beside `installed`, with the
/// operator's choices `chosen`.
async fn told_among(
    ctx: &Ctx,
    services: Vec<lemonfiber_manifest::Service>,
    installed: &[crate::plugin::Installed],
    chosen: &str,
) -> Vec<Wiring> {
    let fillers = crate::test_support::stack()
        .manifest()
        .map(|mut manifest| {
            manifest.services = services;
            crate::wiring::Fillers::of(
                &manifest,
                installed,
                &crate::wiring::Chosen::read(Some(chosen)),
                Some(stack_root()),
            )
        })
        .unwrap_or_default();
    super::super::aggregators::seed_aggregators(ctx, &fillers).await
}

/// The same, among the stack's indexer and book curator alone.
async fn told(ctx: &Ctx) -> Vec<Wiring> {
    told_among(ctx, vec![indexer(), book_curator_svc()], &[], "").await
}

/// The aggregator is registered, under the names the service reads.
#[tokio::test]
async fn the_book_curator_is_told_where_the_aggregator_is() {
    const KEYED: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
    let env = recorded_admin("bindery-told");
    let _ = store::set(&env, crate::config::BOOK_CURATOR_API_KEY, "minted-earlier");
    let http = Fake::by_path_in_turn(vec![(
        "/api/v1/prowlarr",
        vec![Answer::reply(200, "[]"), Answer::reply(201, "{}")],
    )]);
    let ctx = seed_ctx(true, Vec::new(), None, Some(env.clone()))
        .with_http(http.clone())
        .with_filesystem(Arc::new(SeedFs::keyed(Some(KEYED))));

    let wirings = told(&ctx).await;

    assert_eq!(wirings.len(), 1, "{wirings:?}");
    assert_eq!(
        wirings.first().map(|wiring| &wiring.state),
        Some(&crate::seed::State::Wired),
        "{wirings:?}"
    );
    let body = http
        .requests()
        .into_iter()
        .find_map(|asked| asked.body)
        .unwrap_or_default();
    assert!(body.contains("\"apiKey\":\"the-key\""), "{body}");
    assert!(body.contains("http://prowlarr:9696"), "{body}");
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// An aggregator already registered with a key is left alone.
#[tokio::test]
async fn an_aggregator_already_known_is_not_registered_again() {
    const KEYED: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
    const HELD: &str = r#"[{"id":1,"url":"http://prowlarr:9696","apiKey":"set"}]"#;
    let env = recorded_admin("bindery-known");
    let _ = store::set(&env, crate::config::BOOK_CURATOR_API_KEY, "minted-earlier");
    let http = Fake::by_path(vec![("/api/v1/prowlarr", Answer::reply(200, HELD))]);
    let ctx = seed_ctx(true, Vec::new(), None, Some(env.clone()))
        .with_http(http.clone())
        .with_filesystem(Arc::new(SeedFs::keyed(Some(KEYED))));

    let wirings = told(&ctx).await;

    assert_eq!(wirings.len(), 1, "{wirings:?}");
    assert_eq!(
        wirings.first().map(|wiring| &wiring.state),
        Some(&crate::seed::State::AlreadyWired),
        "{wirings:?}"
    );
    assert!(
        http.requests().iter().all(|asked| asked.body.is_none()),
        "an aggregator already known was registered again"
    );
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// One held without a key counts as absent, and is registered again.
///
/// The service stores an entry from a registration whose key it did not
/// understand and answers success. Reading that back as already wired would leave
/// the connection unmade and reported as done.
#[tokio::test]
async fn an_entry_the_service_kept_without_a_key_is_registered_again() {
    const KEYED: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
    const KEYLESS: &str = r#"[{"id":1,"url":"http://prowlarr:9696","apiKey":""}]"#;
    let env = recorded_admin("bindery-keyless");
    let _ = store::set(&env, crate::config::BOOK_CURATOR_API_KEY, "minted-earlier");
    let http = Fake::by_path_in_turn(vec![(
        "/api/v1/prowlarr",
        vec![Answer::reply(200, KEYLESS), Answer::reply(201, "{}")],
    )]);
    let ctx = seed_ctx(true, Vec::new(), None, Some(env.clone()))
        .with_http(http)
        .with_filesystem(Arc::new(SeedFs::keyed(Some(KEYED))));

    let wirings = told(&ctx).await;

    assert_eq!(
        wirings.first().map(|wiring| &wiring.state),
        Some(&crate::seed::State::Wired),
        "an entry holding no key was read as one that did: {wirings:?}"
    );
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// Nothing to do where either end is absent, or the key to reach one is — the book
/// curator's, not minted yet, or the aggregator's, not written yet.
///
/// The aggregator's absence is its own case rather than a repeat of the book
/// curator's: the book curator is reached first, so a stack holding one and no
/// aggregator gets as far as having a client and nothing to tell it about.
#[tokio::test]
async fn nothing_is_told_where_a_book_curator_a_key_or_an_aggregator_is_missing() {
    const KEYED: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
    let env = recorded_admin("bindery-absent");
    let _ = store::set(&env, crate::config::BOOK_CURATOR_API_KEY, "minted-earlier");
    let http = Fake::by_path(vec![("/api/v1/prowlarr", Answer::reply(200, "[]"))]);
    let ctx = seed_ctx(true, Vec::new(), None, Some(env.clone()))
        .with_http(http)
        .with_filesystem(Arc::new(SeedFs::keyed(Some(KEYED))));

    assert!(
        told_among(&ctx, vec![indexer()], &[], "").await.is_empty(),
        "a stack with no book curator wired something"
    );

    let bare = recorded_admin("bindery-unkeyed");
    let unkeyed = seed_ctx(true, Vec::new(), None, Some(bare.clone()))
        .with_filesystem(Arc::new(SeedFs::keyed(Some(KEYED))));
    assert!(
        told(&unkeyed).await.is_empty(),
        "a book curator with no key minted yet wired something"
    );

    assert!(
        told_among(&ctx, vec![book_curator_svc()], &[], "")
            .await
            .is_empty(),
        "a stack with no aggregator wired something"
    );

    let unwritten = seed_ctx(true, Vec::new(), None, Some(env.clone()))
        .with_filesystem(Arc::new(SeedFs::keyed(None)));
    assert!(
        told(&unwritten).await.is_empty(),
        "an aggregator that has not written its key yet was told about"
    );
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
    let _ = std::fs::remove_dir_all(bare.parent().unwrap_or(std::path::Path::new("/")));
}

/// Which service the book curator pulls from is the stack's answer rather than a name
/// written in this crate, so an ask the stack has not settled registers nobody, and
/// one settled on a service this build tells the book curator nothing about is reported
/// as reached by nothing rather than registered on a guess.
#[tokio::test]
async fn an_indexer_ask_settled_on_nothing_it_can_tell_registers_nobody() {
    const KEYED: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
    let env = recorded_admin("bindery-unsettled");
    let _ = store::set(&env, crate::config::BOOK_CURATOR_API_KEY, "minted-earlier");
    let http = Fake::by_path(vec![("/api/v1/prowlarr", Answer::reply(200, "[]"))]);
    let ctx = seed_ctx(true, Vec::new(), None, Some(env.clone()))
        .with_http(http.clone())
        .with_filesystem(Arc::new(SeedFs::keyed(Some(KEYED))));
    // A searcher speaking an adapter nothing here tells the book curator about.
    let mut aggregator = manifest_service(
        "searcher",
        Some(lemonfiber_manifest::Api {
            kind: lemonfiber_manifest::ApiKind::Sabnzbd,
            key_source: lemonfiber_manifest::KeySource::ConfigIni,
            path: Some("/config/searcher.ini".to_owned()),
            version: None,
        }),
        Some(5076),
    );
    aggregator.provides = vec!["indexer.search".to_owned()];

    let unprovided = told_among(&ctx, vec![aggregator_svc(), book_curator_svc()], &[], "").await;
    let unspoken = told_among(
        &ctx,
        vec![indexer(), aggregator.clone(), book_curator_svc()],
        &[],
        "indexer.search=searcher",
    )
    .await;
    let reported = crate::test_support::stack()
        .manifest()
        .map(|mut manifest| {
            manifest.services = vec![indexer(), aggregator, book_curator_svc()];
            super::super::connecting::unmatched(&crate::wiring::Fillers::of(
                &manifest,
                &[],
                &crate::wiring::Chosen::read(Some("indexer.search=searcher")),
                Some(stack_root()),
            ))
        })
        .unwrap_or_default();

    assert!(unprovided.is_empty(), "{unprovided:?}");
    assert!(unspoken.is_empty(), "{unspoken:?}");
    assert!(
        reported.iter().any(
            |wiring| wiring.connection == "searcher the app into bindery the app"
                && matches!(wiring.state, crate::seed::State::Unmatched { .. })
        ),
        "{reported:?}"
    );
    assert!(http.requests().iter().all(|asked| asked.body.is_none()));
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// A plugin's aggregator chosen in the stack's place is the one the book curator is told
/// about: at its own id on the port it listens on, with the key it wrote for itself —
/// and one whose key file leads away from beneath its own directory is refused, naming
/// it, with nothing told.
#[tokio::test]
async fn a_plugin_aggregator_is_told_on_its_own_terms() {
    const KEYED: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
    let env = recorded_admin("bindery-plugin");
    let _ = store::set(&env, crate::config::BOOK_CURATOR_API_KEY, "minted-earlier");
    let stand_in = crate::test_support::an_installed(
        "kept",
        vec![crate::test_support::a_placed(
            "kept",
            &["indexer.search"],
            Some(curator_api(Some("/config/config.xml"))),
            Some(8990),
        )],
    );
    let http = Fake::by_path_in_turn(vec![(
        "/api/v1/prowlarr",
        vec![Answer::reply(200, "[]"), Answer::reply(201, "{}")],
    )]);
    let ctx = seed_ctx(true, Vec::new(), None, Some(env.clone()))
        .with_http(http.clone())
        .with_filesystem(Arc::new(SeedFs::keyed(Some(KEYED))));
    let escaping = seed_ctx(true, Vec::new(), None, Some(env.clone()))
        .with_http(Fake::by_path(vec![(
            "/api/v1/prowlarr",
            Answer::reply(200, "[]"),
        )]))
        .with_filesystem(Arc::new(leading_away_from_the_stand_in()));
    let services = vec![indexer(), book_curator_svc()];

    let wirings = told_among(
        &ctx,
        services.clone(),
        std::slice::from_ref(&stand_in),
        "indexer.search=kept",
    )
    .await;
    let refused = told_among(
        &escaping,
        services,
        std::slice::from_ref(&stand_in),
        "indexer.search=kept",
    )
    .await;

    assert_eq!(
        wirings.first().map(|wiring| &wiring.state),
        Some(&crate::seed::State::Wired),
        "{wirings:?}"
    );
    let body = http
        .requests()
        .into_iter()
        .find_map(|asked| asked.body)
        .unwrap_or_default();
    assert!(body.contains("http://kept:8990"), "{body}");
    assert!(body.contains("\"apiKey\":\"the-key\""), "{body}");
    assert!(
        refused.first().is_some_and(
            |wiring| wiring.connection == "Indexers into bindery the app"
                && matches!(wiring.state, crate::seed::State::Refused { .. })
        ),
        "{refused:?}"
    );
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// **A plugin is never told the indexer's key.** A plugin's service standing where the
/// book curator would ask, on a stack whose own book curator is gone, is told nothing, and
/// nothing carrying a key is sent anywhere.
#[tokio::test]
async fn a_plugin_under_the_book_curators_id_is_never_told_the_indexers_key() {
    const KEYED: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
    let env = recorded_admin("bindery-impostor");
    let _ = store::set(&env, crate::config::BOOK_CURATOR_API_KEY, "minted-earlier");
    let http = Fake::by_path(vec![("/api/v1/prowlarr", Answer::reply(200, "[]"))]);
    let ctx = seed_ctx(true, Vec::new(), None, Some(env.clone()))
        .with_http(http.clone())
        .with_filesystem(Arc::new(SeedFs::keyed(Some(KEYED))));
    let fillers = asked_by_a_plugin(&book_curator_svc(), vec![indexer()]);

    let wirings = super::super::aggregators::seed_aggregators(&ctx, &fillers).await;

    assert!(wirings.is_empty(), "{wirings:?}");
    assert!(
        http.requests().is_empty(),
        "the plugin under the book curator's id was asked something: {:?}",
        http.requests()
    );
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// A service that will not answer is reported, in its own words.
#[tokio::test]
async fn a_book_curator_that_refuses_is_reported() {
    const KEYED: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
    let env = recorded_admin("bindery-refuses");
    let _ = store::set(&env, crate::config::BOOK_CURATOR_API_KEY, "minted-earlier");
    let http = Fake::by_path(vec![("/api/v1/prowlarr", Answer::reply(401, ""))]);
    let ctx = seed_ctx(true, Vec::new(), None, Some(env.clone()))
        .with_http(http)
        .with_filesystem(Arc::new(SeedFs::keyed(Some(KEYED))));

    let wirings = told(&ctx).await;

    assert!(
        wirings
            .first()
            .is_some_and(|wiring| matches!(wiring.state, crate::seed::State::Failed { .. })),
        "{wirings:?}"
    );
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// A book curator that is not answering is skipped for a later run.
#[tokio::test]
async fn a_book_curator_that_is_not_answering_is_skipped() {
    const KEYED: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
    let env = recorded_admin("bindery-silent");
    let _ = store::set(&env, crate::config::BOOK_CURATOR_API_KEY, "minted-earlier");
    let http = Fake::by_path(vec![("/api/v1/prowlarr", Answer::Silent)]);
    let ctx = seed_ctx(None, true, Vec::new(), None, Some(env.clone()))
        .with_http(http)
        .with_filesystem(Arc::new(SeedFs::keyed(Some(KEYED))));

    let wirings = told(&ctx).await;

    assert!(wirings.first().is_some_and(is_skipped), "{wirings:?}");
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// A registration the service refuses is reported rather than counted as made.
///
/// The read succeeding says nothing about the write. A key the service will not
/// accept refuses only the registration, and reporting that connection as made
/// would leave it unmade with nothing anywhere to say so.
#[tokio::test]
async fn a_registration_the_book_curator_refuses_is_reported() {
    const KEYED: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
    let env = recorded_admin("bindery-refused-write");
    let _ = store::set(&env, crate::config::BOOK_CURATOR_API_KEY, "minted-earlier");
    let http = Fake::by_path_in_turn(vec![(
        "/api/v1/prowlarr",
        vec![Answer::reply(200, "[]"), Answer::reply(401, "")],
    )]);
    let ctx = seed_ctx(true, Vec::new(), None, Some(env.clone()))
        .with_http(http)
        .with_filesystem(Arc::new(SeedFs::keyed(Some(KEYED))));

    let wirings = told(&ctx).await;

    let state = wirings.first().map(|wiring| &wiring.state);
    assert!(
        matches!(state, Some(&crate::seed::State::Failed { .. })),
        "a refused registration was not reported: {wirings:?}"
    );
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// A rehearsal names the aggregator the book curator would be told to pull from, and
/// tells it nothing.
///
/// The read and the already-there check are true of a real run too, so what a
/// rehearsal leaves out is the registration and nothing above it — which is why the
/// address is there to report: the book curator pulls from whatever it is pointed at,
/// and an operator checking this is checking that it would be pointed at the
/// aggregator on the stack's own network rather than at a host address no container
/// can reach.
#[tokio::test]
async fn a_rehearsed_pass_names_the_aggregator_it_would_register_and_registers_none() {
    const KEYED: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
    let env = recorded_admin("bindery-rehearsed");
    let _ = store::set(&env, crate::config::BOOK_CURATOR_API_KEY, "minted-earlier");
    let http = Fake::by_path(vec![("/api/v1/prowlarr", Answer::reply(200, "[]"))]);
    let ctx = seed_ctx(true, Vec::new(), None, Some(env.clone()))
        .with_http(http.clone())
        .with_filesystem(Arc::new(SeedFs::keyed(Some(KEYED))))
        .rehearsing();

    let wirings = told(&ctx).await;

    assert_eq!(
        wirings.first().map(|wiring| &wiring.state),
        Some(&crate::seed::State::WouldWire {
            yours: None,
            ours: Some("http://prowlarr:9696".to_owned()),
        }),
        "{wirings:?}"
    );
    assert!(
        http.requests().iter().all(|asked| asked.body.is_none()),
        "a rehearsal registered the aggregator it was only asked about"
    );
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// The plugin whose curator pulls its indexers over `indexer.pull`.
const PULLER: &str = "puller";

/// The adapter in front of [`PULLER`]'s curator.
const PULLER_ADAPTER: &str = "puller-adapter";

/// Where the adapter answers `add_aggregator`.
const ADD: &str = "/lemonfiber/indexer.pull/v1/add_aggregator";

/// Where the adapter answers `aggregators`.
const HELD: &str = "/lemonfiber/indexer.pull/v1/aggregators";

/// [`PULLER`]: a curator filing `media` that asks for an indexer, and the adapter in front
/// of it speaking `indexer.pull`.
fn puller(media: &str) -> crate::plugin::Installed {
    let mut curator = crate::test_support::a_placed(PULLER, &[], None, Some(8787));
    curator.media_types = vec![media.to_owned()];
    curator.asks = vec![crate::plugin::Asking {
        capability: "indexer.search".to_owned(),
        each: false,
        native: None,
    }];
    let mut adapter = crate::test_support::a_placed(
        PULLER_ADAPTER,
        &[],
        None,
        Some(crate::test_support::CONTRACTED_PORT),
    );
    adapter.speaks = vec!["indexer.pull@1".to_owned()];
    adapter.fronts = Some(PULLER.to_owned());
    let mut installed = crate::test_support::an_installed(PULLER, vec![curator, adapter]);
    installed.manifest = crate::test_support::CONTRACTED_MANIFEST.to_owned();
    installed.tree = crate::test_support::A_TREE.to_owned();
    installed
}

/// The stack's indexer beside [`puller`] filing `media`, written at `project`.
fn beside_the_puller(project: &std::path::Path, media: &str) -> crate::wiring::Fillers {
    fillers_trusting(
        vec![indexer()],
        &[puller(media)],
        project,
        &crate::test_support::first_party(PULLER),
    )
}

/// What the pass tells [`puller`] filing `media`, beside the stack's indexer, with the
/// adapter answering `added` to `add_aggregator`.
async fn pulled(tag: &str, media: &str, added: Answer) -> (Vec<Wiring>, Arc<Fake>) {
    let project = lemonfiber_fixtures::scratch::Scratch::new(&format!("pulled-{tag}"));
    let http = Fake::by_path(vec![(HELD, Answer::reply(200, "[]")), (ADD, added)]);
    let ctx = contracted_ctx(&project, PULLER_ADAPTER, true, http.clone());
    let fillers = beside_the_puller(&project, media);
    (
        super::super::aggregators::seed_aggregators(&ctx, &fillers).await,
        http,
    )
}

#[tokio::test]
async fn a_curator_pulling_over_the_contract_is_told_the_indexer_in_its_native_api() {
    let (wirings, http) = pulled("told", "books", Answer::reply(204, "")).await;

    assert!(
        matches!(wirings.as_slice(), [one] if one.state == crate::seed::State::Wired
            && one.connection == format!("Indexers into {PULLER} the stand-in")),
        "{wirings:?}"
    );
    let body = http
        .requests()
        .into_iter()
        .filter(|asked| asked.url.ends_with(ADD))
        .find_map(|asked| asked.body)
        .unwrap_or_default();
    for said in ["\"servarr\"", "http://prowlarr:9696", "the-key"] {
        assert!(body.contains(said), "{said} missing from {body}");
    }
}

#[tokio::test]
async fn a_curator_that_cannot_pull_from_the_indexer_is_left_unpaired() {
    let (wirings, _) = pulled(
        "unsupported",
        "books",
        Answer::served(
            500,
            crate::ports::http::PROBLEM,
            r#"{"type":"unsupported","detail":"it reads one native API alone"}"#,
        ),
    )
    .await;

    assert!(
        matches!(wirings.as_slice(), [one] if matches!(&one.state,
            crate::seed::State::Unmatched { reason } if reason.contains("it reads one native API alone"))),
        "{wirings:?}"
    );
}

#[tokio::test]
async fn a_curator_the_indexers_app_sync_serves_is_never_told_to_pull() {
    let (wirings, http) = pulled("synced", "tv", Answer::reply(204, "")).await;

    assert!(wirings.is_empty(), "{wirings:?}");
    assert!(http.requests().is_empty(), "{:?}", http.requests());
}

#[tokio::test]
async fn a_curator_pulling_over_the_contract_its_adapter_does_not_answer_is_told_nothing() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("pulled-unanswered");
    let http = Fake::always(Answer::reply(204, ""));
    let ctx = seed_ctx(true, Vec::new(), None, None).with_http(http.clone());

    let wirings =
        super::super::aggregators::seed_aggregators(&ctx, &beside_the_puller(&project, "books"))
            .await;

    assert!(wirings.is_empty(), "{wirings:?}");
    assert!(http.requests().is_empty(), "{:?}", http.requests());
}

#[test]
fn a_curator_the_indexers_app_sync_serves_is_reported_as_synced() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("pulled-synced-said");

    let unmatched = super::super::connecting::unmatched(&beside_the_puller(&project, "tv"));

    assert!(
        unmatched.iter().any(|one| matches!(&one.state,
            crate::seed::State::Unmatched { reason }
                if reason.contains(" and syncs the media ") && reason.contains(PULLER))),
        "{unmatched:?}"
    );
}
