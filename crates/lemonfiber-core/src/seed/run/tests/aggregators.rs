//! Telling the book *arr where its metadata aggregator is.

use super::*;

/// The stack's indexer, answering the book \*arr's ask for one.
fn indexer() -> lemonfiber_manifest::Service {
    let mut indexer = prowlarr();
    indexer.provides = vec!["indexer.search".to_owned()];
    indexer
}

/// What the pass tells each book \*arr among `services`, beside `installed`, with the
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

/// The same, among the stack's indexer and book \*arr alone.
async fn told(ctx: &Ctx) -> Vec<Wiring> {
    told_among(ctx, vec![indexer(), bindery_svc()], &[], "").await
}

/// The aggregator is registered, under the names the service reads.
#[tokio::test]
async fn the_book_arr_is_told_where_the_aggregator_is() {
    const KEYED: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
    let env = recorded_admin("bindery-told");
    let _ = store::set(&env, crate::config::BINDERY_API_KEY, "minted-earlier");
    let http = Fake::by_path_in_turn(vec![(
        "/api/v1/prowlarr",
        vec![Answer::reply(200, "[]"), Answer::reply(201, "{}")],
    )]);
    let ctx = seed_ctx(None, true, Vec::new(), None, Some(env.clone()))
        .with_http(http.clone())
        .with_filesystem(Arc::new(SeedFs::keyed(Some(KEYED), None)));

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
    let _ = store::set(&env, crate::config::BINDERY_API_KEY, "minted-earlier");
    let http = Fake::by_path(vec![("/api/v1/prowlarr", Answer::reply(200, HELD))]);
    let ctx = seed_ctx(None, true, Vec::new(), None, Some(env.clone()))
        .with_http(http.clone())
        .with_filesystem(Arc::new(SeedFs::keyed(Some(KEYED), None)));

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
    let _ = store::set(&env, crate::config::BINDERY_API_KEY, "minted-earlier");
    let http = Fake::by_path_in_turn(vec![(
        "/api/v1/prowlarr",
        vec![Answer::reply(200, KEYLESS), Answer::reply(201, "{}")],
    )]);
    let ctx = seed_ctx(None, true, Vec::new(), None, Some(env.clone()))
        .with_http(http)
        .with_filesystem(Arc::new(SeedFs::keyed(Some(KEYED), None)));

    let wirings = told(&ctx).await;

    assert_eq!(
        wirings.first().map(|wiring| &wiring.state),
        Some(&crate::seed::State::Wired),
        "an entry holding no key was read as one that did: {wirings:?}"
    );
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// Nothing to do where either end is absent, or the key to reach one is — the book
/// \*arr's, not minted yet, or the aggregator's, not written yet.
///
/// The aggregator's absence is its own case rather than a repeat of the book
/// \*arr's: the book \*arr is reached first, so a stack holding one and no
/// aggregator gets as far as having a client and nothing to tell it about.
#[tokio::test]
async fn nothing_is_told_where_a_book_arr_a_key_or_an_aggregator_is_missing() {
    const KEYED: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
    let env = recorded_admin("bindery-absent");
    let _ = store::set(&env, crate::config::BINDERY_API_KEY, "minted-earlier");
    let http = Fake::by_path(vec![("/api/v1/prowlarr", Answer::reply(200, "[]"))]);
    let ctx = seed_ctx(None, true, Vec::new(), None, Some(env.clone()))
        .with_http(http)
        .with_filesystem(Arc::new(SeedFs::keyed(Some(KEYED), None)));

    assert!(
        told_among(&ctx, vec![indexer()], &[], "").await.is_empty(),
        "a stack with no book *arr wired something"
    );

    let bare = recorded_admin("bindery-unkeyed");
    let unkeyed = seed_ctx(None, true, Vec::new(), None, Some(bare.clone()))
        .with_filesystem(Arc::new(SeedFs::keyed(Some(KEYED), None)));
    assert!(
        told(&unkeyed).await.is_empty(),
        "a book *arr with no key minted yet wired something"
    );

    assert!(
        told_among(&ctx, vec![bindery_svc()], &[], "")
            .await
            .is_empty(),
        "a stack with no aggregator wired something"
    );

    let unwritten = seed_ctx(None, true, Vec::new(), None, Some(env.clone()))
        .with_filesystem(Arc::new(SeedFs::keyed(None, None)));
    assert!(
        told(&unwritten).await.is_empty(),
        "an aggregator that has not written its key yet was told about"
    );
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
    let _ = std::fs::remove_dir_all(bare.parent().unwrap_or(std::path::Path::new("/")));
}

/// Which service the book \*arr pulls from is the stack's answer rather than a name
/// written in this crate, so an ask the stack has not settled registers nobody, and
/// one settled on a service this build tells the book \*arr nothing about is reported
/// as reached by nothing rather than registered on a guess.
#[tokio::test]
async fn an_indexer_ask_settled_on_nothing_it_can_tell_registers_nobody() {
    const KEYED: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
    let env = recorded_admin("bindery-unsettled");
    let _ = store::set(&env, crate::config::BINDERY_API_KEY, "minted-earlier");
    let http = Fake::by_path(vec![("/api/v1/prowlarr", Answer::reply(200, "[]"))]);
    let ctx = seed_ctx(None, true, Vec::new(), None, Some(env.clone()))
        .with_http(http.clone())
        .with_filesystem(Arc::new(SeedFs::keyed(Some(KEYED), None)));
    // A searcher speaking an adapter nothing here tells the book *arr about.
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

    let unprovided = told_among(&ctx, vec![prowlarr(), bindery_svc()], &[], "").await;
    let unspoken = told_among(
        &ctx,
        vec![indexer(), aggregator.clone(), bindery_svc()],
        &[],
        "indexer.search=searcher",
    )
    .await;
    let reported = crate::test_support::stack()
        .manifest()
        .map(|mut manifest| {
            manifest.services = vec![indexer(), aggregator, bindery_svc()];
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

/// A plugin's aggregator chosen in the stack's place is the one the book \*arr is told
/// about: at its own id on the port it listens on, with the key it wrote for itself —
/// and one whose key file leads away from beneath its own directory is refused, naming
/// it, with nothing told.
#[tokio::test]
async fn a_plugin_aggregator_is_told_on_its_own_terms() {
    const KEYED: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
    let env = recorded_admin("bindery-plugin");
    let _ = store::set(&env, crate::config::BINDERY_API_KEY, "minted-earlier");
    let stand_in = crate::test_support::an_installed(
        "kept",
        vec![crate::test_support::a_placed(
            "kept",
            &["indexer.search"],
            Some(servarr_api(Some("/config/config.xml"))),
            Some(8990),
        )],
    );
    let http = Fake::by_path_in_turn(vec![(
        "/api/v1/prowlarr",
        vec![Answer::reply(200, "[]"), Answer::reply(201, "{}")],
    )]);
    let ctx = seed_ctx(None, true, Vec::new(), None, Some(env.clone()))
        .with_http(http.clone())
        .with_filesystem(Arc::new(SeedFs::keyed(Some(KEYED), None)));
    let escaping = seed_ctx(None, true, Vec::new(), None, Some(env.clone()))
        .with_http(Fake::by_path(vec![(
            "/api/v1/prowlarr",
            Answer::reply(200, "[]"),
        )]))
        .with_filesystem(Arc::new(leading_away_from_the_stand_in()));
    let services = vec![indexer(), bindery_svc()];

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
/// book \*arr would ask, on a stack whose own book \*arr is gone, is told nothing, and
/// nothing carrying a key is sent anywhere.
#[tokio::test]
async fn a_plugin_under_the_book_arrs_id_is_never_told_the_indexers_key() {
    const KEYED: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
    let env = recorded_admin("bindery-impostor");
    let _ = store::set(&env, crate::config::BINDERY_API_KEY, "minted-earlier");
    let http = Fake::by_path(vec![("/api/v1/prowlarr", Answer::reply(200, "[]"))]);
    let ctx = seed_ctx(None, true, Vec::new(), None, Some(env.clone()))
        .with_http(http.clone())
        .with_filesystem(Arc::new(SeedFs::keyed(Some(KEYED), None)));
    let fillers = asked_by_a_plugin(&bindery_svc(), vec![indexer()]);

    let wirings = super::super::aggregators::seed_aggregators(&ctx, &fillers).await;

    assert!(wirings.is_empty(), "{wirings:?}");
    assert!(
        http.requests().is_empty(),
        "the plugin under the book *arr's id was asked something: {:?}",
        http.requests()
    );
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// The same rule on the other converted connection: what the request service signs
/// in against is whatever fills the identity ask, and an ask nothing settles is
/// nothing to wire.
///
/// The last case is the one worth having. A filler this build has no adapter for
/// is a service it cannot speak to, and the stack already answers that way for a
/// service it declares no API for — so the two agree rather than one of them
/// guessing at a protocol from a name.
#[test]
fn the_identity_source_is_whatever_fills_the_ask_and_nothing_where_that_is_unsettled() {
    let apiless = manifest_service("lockbox", None, Some(9000));
    let services = [jellyfin_svc(), seerr_with_settings(), apiless];

    assert_eq!(
        super::super::identity::identity_source(&services, &identified()).map(|addr| addr.id),
        Some("jellyfin".to_owned())
    );

    for unsettled in [
        std::collections::BTreeMap::new(),
        std::collections::BTreeMap::from([(
            "identity.source".to_owned(),
            vec!["jellyfin".to_owned(), "lockbox".to_owned()],
        )]),
        filling("identity.source", "plex"),
        filling("identity.source", "lockbox"),
    ] {
        assert_eq!(
            super::super::identity::identity_source(&services, &unsettled).map(|addr| addr.id),
            None,
            "an ask settled as {unsettled:?} was wired to something anyway"
        );
    }
}

/// A service that will not answer is reported, in its own words.
#[tokio::test]
async fn a_book_arr_that_refuses_is_reported() {
    const KEYED: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
    let env = recorded_admin("bindery-refuses");
    let _ = store::set(&env, crate::config::BINDERY_API_KEY, "minted-earlier");
    let http = Fake::by_path(vec![("/api/v1/prowlarr", Answer::reply(401, ""))]);
    let ctx = seed_ctx(None, true, Vec::new(), None, Some(env.clone()))
        .with_http(http)
        .with_filesystem(Arc::new(SeedFs::keyed(Some(KEYED), None)));

    let wirings = told(&ctx).await;

    assert!(
        wirings
            .first()
            .is_some_and(|wiring| matches!(wiring.state, crate::seed::State::Failed { .. })),
        "{wirings:?}"
    );
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// A registration the service refuses is reported rather than counted as made.
///
/// The read succeeding says nothing about the write. A key the service will not
/// accept refuses only the registration, and reporting that connection as made
/// would leave it unmade with nothing anywhere to say so.
#[tokio::test]
async fn a_registration_the_book_arr_refuses_is_reported() {
    const KEYED: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
    let env = recorded_admin("bindery-refused-write");
    let _ = store::set(&env, crate::config::BINDERY_API_KEY, "minted-earlier");
    let http = Fake::by_path_in_turn(vec![(
        "/api/v1/prowlarr",
        vec![Answer::reply(200, "[]"), Answer::reply(401, "")],
    )]);
    let ctx = seed_ctx(None, true, Vec::new(), None, Some(env.clone()))
        .with_http(http)
        .with_filesystem(Arc::new(SeedFs::keyed(Some(KEYED), None)));

    let wirings = told(&ctx).await;

    let state = wirings.first().map(|wiring| &wiring.state);
    assert!(
        matches!(state, Some(&crate::seed::State::Failed { .. })),
        "a refused registration was not reported: {wirings:?}"
    );
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// A rehearsal names the aggregator the book \*arr would be told to pull from, and
/// tells it nothing.
///
/// The read and the already-there check are true of a real run too, so what a
/// rehearsal leaves out is the registration and nothing above it — which is why the
/// address is there to report: the book \*arr pulls from whatever it is pointed at,
/// and an operator checking this is checking that it would be pointed at the
/// aggregator on the stack's own network rather than at a host address no container
/// can reach.
#[tokio::test]
async fn a_rehearsed_pass_names_the_aggregator_it_would_register_and_registers_none() {
    const KEYED: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
    let env = recorded_admin("bindery-rehearsed");
    let _ = store::set(&env, crate::config::BINDERY_API_KEY, "minted-earlier");
    let http = Fake::by_path(vec![("/api/v1/prowlarr", Answer::reply(200, "[]"))]);
    let ctx = seed_ctx(None, true, Vec::new(), None, Some(env.clone()))
        .with_http(http.clone())
        .with_filesystem(Arc::new(SeedFs::keyed(Some(KEYED), None)))
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
