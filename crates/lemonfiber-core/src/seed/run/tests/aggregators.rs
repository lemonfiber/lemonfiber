//! Telling a curator that pulls its indexers where its aggregator is.

use super::*;
use crate::ports::service::{AggregatorAt, Protocol};
use crate::test_support::books;

/// The key lemonfiber minted for the moved indexer.
const KEYED: &str = "the-key";

/// The stack's indexer, answering the book curator's ask for one.
fn indexer() -> lemonfiber_manifest::Service {
    let mut indexer = aggregator_svc();
    indexer.provides = vec!["indexer.search".to_owned()];
    indexer
}

/// The stack's `services` and its own book curator, the curator moved into its plugin
/// beside `installed`, and the indexer where `services` runs one filling `indexer.search`,
/// with the operator's choices `chosen`.
fn moved_beside(
    services: Vec<lemonfiber_manifest::Service>,
    installed: Vec<crate::plugin::Installed>,
    chosen: &str,
) -> crate::wiring::Fillers {
    moved_trusting(
        services,
        installed,
        chosen,
        crate::plugin::first_party::EMBEDDED,
    )
}

/// The same, with `trusted` as the plugins that are first-party.
fn moved_trusting(
    mut services: Vec<lemonfiber_manifest::Service>,
    mut installed: Vec<crate::plugin::Installed>,
    chosen: &str,
    trusted: &[crate::plugin::first_party::FirstParty],
) -> crate::wiring::Fillers {
    services.push(book_curator_svc());
    installed.extend(books::moved());
    let indexing = services
        .iter()
        .any(|service| service.provides.iter().any(|one| one == "indexer.search"));
    installed.extend(INDEXER.installed().filter(|_| indexing));
    crate::test_support::stack()
        .manifest()
        .map(|mut manifest| {
            manifest.services = services;
            crate::wiring::Fillers::trusting(
                &manifest,
                &installed,
                &crate::wiring::Chosen::read(Some(chosen)),
                Some(stack_root()),
                trusted,
            )
        })
        .unwrap_or_default()
}

/// A context of the test `name` asking the moved curator's adapter over `http`, holding
/// `keyed` as the key lemonfiber minted for the moved indexer where it holds one.
fn curator_ctx(name: &str, http: Arc<Fake>, keyed: Option<&str>) -> Ctx {
    let parts: &[crate::test_support::adapted::Adapted] =
        if keyed.is_some() { &[INDEXER] } else { &[] };
    books::asking(minted_ctx(name, parts, keyed.unwrap_or_default())).with_http(http)
}

/// A moved curator reading from `held`, and taking every aggregator it is told about with
/// `added`.
fn reading(held: &[crate::ports::service::KnownAggregator], added: Answer) -> Arc<Fake> {
    Fake::by_path(vec![books::aggregators(held), books::add_aggregator(added)])
}

/// What the pass tells the moved curator beside the stack's indexer.
async fn told(ctx: &Ctx) -> Vec<Wiring> {
    super::super::aggregators::seed_aggregators(ctx, &moved_beside(vec![indexer()], Vec::new(), ""))
        .await
}

/// The moved indexer as the moved curator is told about it: in its native API, where it
/// answers on the stack's network, with the key lemonfiber minted for it.
fn the_indexer() -> AggregatorAt {
    AggregatorAt {
        name: "Prowlarr".to_owned(),
        url: "http://prowlarr:9696".to_owned(),
        protocol: Protocol("servarr".to_owned()),
        key: "the-key".to_owned(),
    }
}

/// The moved curator is told where the aggregator is over its contract, through the
/// adapter in front of it, and the report names the curator.
#[tokio::test]
async fn the_moved_book_curator_is_told_where_the_aggregator_is_over_its_contract() {
    let http = reading(&[], books::taken());
    let ctx = curator_ctx("aggregators-told", http.clone(), Some(KEYED));

    let wirings = told(&ctx).await;

    assert!(
        matches!(wirings.as_slice(), [one] if one.state == crate::seed::State::Wired
            && one.connection == "Indexers into Bindery"),
        "{wirings:?}"
    );
    let sent = http.requests();
    assert!(sent.iter().all(|one| one
        .url
        .starts_with("http://127.0.0.1:8080/lemonfiber/indexer.pull/v1/")));
    assert_eq!(books::added(&sent), vec![the_indexer()]);
}

/// An aggregator already read from with a key is left alone.
#[tokio::test]
async fn an_aggregator_already_known_is_not_registered_again() {
    let http = reading(
        &[books::reading_from("http://prowlarr:9696", true)],
        books::taken(),
    );
    let ctx = curator_ctx("aggregators-known", http.clone(), Some(KEYED));

    let wirings = told(&ctx).await;

    assert_eq!(
        wirings.first().map(|wiring| &wiring.state),
        Some(&crate::seed::State::AlreadyWired),
        "{wirings:?}"
    );
    assert!(books::added(&http.requests()).is_empty());
}

/// One held without a key counts as absent, and is registered again: reading it back as
/// already wired would leave the connection unmade and reported as done.
#[tokio::test]
async fn an_entry_the_curator_kept_without_a_key_is_registered_again() {
    let http = reading(
        &[books::reading_from("http://prowlarr:9696", false)],
        books::taken(),
    );
    let ctx = curator_ctx("aggregators-keyless-entry", http.clone(), Some(KEYED));

    let wirings = told(&ctx).await;

    assert_eq!(
        wirings.first().map(|wiring| &wiring.state),
        Some(&crate::seed::State::Wired),
        "an entry holding no key was read as one that did: {wirings:?}"
    );
    assert_eq!(books::added(&http.requests()), vec![the_indexer()]);
}

/// Nothing to do where either end is absent, or the aggregator has not written its key
/// yet.
#[tokio::test]
async fn nothing_is_told_where_a_curator_an_aggregator_or_its_key_is_missing() {
    let http = reading(&[], books::taken());
    let ctx = curator_ctx("aggregators-missing", http.clone(), Some(KEYED));

    let unmoved =
        super::super::aggregators::seed_aggregators(&ctx, &fillers_of(vec![indexer()])).await;
    let alone = super::super::aggregators::seed_aggregators(
        &ctx,
        &moved_beside(Vec::new(), Vec::new(), ""),
    )
    .await;
    let unwritten = told(&curator_ctx("aggregators-unminted", http.clone(), None)).await;

    assert!(unmoved.is_empty(), "{unmoved:?}");
    assert!(alone.is_empty(), "{alone:?}");
    assert!(unwritten.is_empty(), "{unwritten:?}");
    assert!(books::added(&http.requests()).is_empty());
}

/// The stack's own book curator, which no adapter fronts, is told nothing: it is asked
/// over its contract or not at all.
#[tokio::test]
async fn the_stacks_own_book_curator_is_told_nothing() {
    let http = Fake::always(books::taken());
    let ctx = curator_ctx("aggregators-own", http.clone(), Some(KEYED));

    let wirings = super::super::aggregators::seed_aggregators(
        &ctx,
        &fillers_of(vec![indexer(), book_curator_svc()]),
    )
    .await;

    assert!(wirings.is_empty(), "{wirings:?}");
    assert!(http.requests().is_empty(), "{:?}", http.requests());
}

/// An indexer ask nothing fills registers nobody.
#[tokio::test]
async fn an_indexer_ask_nothing_fills_registers_nobody() {
    let http = reading(&[], books::taken());
    let ctx = curator_ctx("aggregators-unfilled", http.clone(), Some(KEYED));

    let wirings = super::super::aggregators::seed_aggregators(
        &ctx,
        &moved_beside(vec![aggregator_svc()], Vec::new(), ""),
    )
    .await;

    assert!(wirings.is_empty(), "{wirings:?}");
    assert!(books::added(&http.requests()).is_empty());
}

/// A plugin `kept` whose aggregator answers `indexer.search` in `servarr`, holding a key
/// lemonfiber minted for it.
fn kept_aggregator() -> crate::plugin::Installed {
    let mut aggregator = crate::test_support::a_placed("kept", &["indexer.search"], Some(8990));
    aggregator.native = Some("servarr".to_owned());
    let mut installed = crate::test_support::an_installed("kept", vec![aggregator]);
    installed.manifest = crate::test_support::CONTRACTED_MANIFEST.to_owned();
    installed.tree = crate::test_support::A_TREE.to_owned();
    installed.declared.secrets = vec![crate::plugin::Secret {
        id: "key".to_owned(),
        of: "kept".to_owned(),
        why: "The key the aggregator answers to".to_owned(),
        minted: true,
        was: None,
        serves: None,
        needs: lemonfiber_manifest::container::choices::Needs::new(),
        needed_to_start: false,
        lapses_s: None,
        captured: false,
    }];
    installed
}

/// A first-party plugin's aggregator chosen in the stack's place is the one the moved
/// curator is told about: at its own id on the port it listens on, with the key lemonfiber
/// minted for it.
#[tokio::test]
async fn a_first_party_plugin_aggregator_is_told_on_its_own_terms() {
    let trusted: Vec<crate::plugin::first_party::FirstParty> = crate::plugin::first_party::EMBEDDED
        .iter()
        .copied()
        .chain(first_party("kept"))
        .collect();
    let fillers = moved_trusting(
        vec![indexer()],
        vec![kept_aggregator()],
        "indexer.search=kept",
        &trusted,
    );
    let http = reading(&[], books::taken());
    let ctx = curator_ctx("aggregators-first-party", http.clone(), None);
    keeping_minted(&ctx, Some(kept_aggregator()), |_| KEYED);

    let wirings = super::super::aggregators::seed_aggregators(&ctx, &fillers).await;

    assert_eq!(
        wirings.first().map(|wiring| &wiring.state),
        Some(&crate::seed::State::Wired),
        "{wirings:?}"
    );
    assert_eq!(
        books::added(&http.requests())
            .iter()
            .map(|added| (added.url.as_str(), added.key.as_str()))
            .collect::<Vec<_>>(),
        vec![("http://kept:8990", KEYED)]
    );
}

/// A third-party plugin's aggregator is never handed to the moved curator, a first-party
/// plugin's service: nothing is told, and the pair is reported as reached by nothing.
#[tokio::test]
async fn a_third_party_aggregator_is_never_handed_to_the_moved_curator() {
    let fillers = moved_beside(
        vec![indexer()],
        vec![kept_aggregator()],
        "indexer.search=kept",
    );
    let http = reading(&[], books::taken());
    let ctx = curator_ctx("aggregators-third-party", http.clone(), Some(KEYED));

    let wirings = super::super::aggregators::seed_aggregators(&ctx, &fillers).await;
    let reported = super::super::connecting::unmatched(&fillers);

    assert!(wirings.is_empty(), "{wirings:?}");
    assert!(http.requests().is_empty(), "{:?}", http.requests());
    assert!(
        reported.iter().any(
            |wiring| wiring.connection == "kept the stand-in into Bindery"
                && matches!(wiring.state, crate::seed::State::Unmatched { .. })
        ),
        "{reported:?}"
    );
}

/// A curator that refuses, and one whose adapter's upstream is not answering, are
/// reported as each is: refused in its own words, and skipped for a later run.
#[tokio::test]
async fn a_curator_that_refuses_or_is_not_answering_is_reported_as_such() {
    let refusing = told(&curator_ctx(
        "aggregators-refusing",
        Fake::always(books::refusing()),
        Some(KEYED),
    ))
    .await;
    let silent = told(&curator_ctx(
        "aggregators-silent",
        Fake::always(books::unavailable()),
        Some(KEYED),
    ))
    .await;

    assert!(
        refusing
            .first()
            .is_some_and(|wiring| matches!(wiring.state, crate::seed::State::Failed { .. })),
        "{refusing:?}"
    );
    assert!(silent.first().is_some_and(is_skipped), "{silent:?}");
}

/// A registration the curator refuses is reported rather than counted as made: the read
/// succeeding says nothing about the write.
#[tokio::test]
async fn a_registration_the_curator_refuses_is_reported() {
    let ctx = curator_ctx(
        "aggregators-refused",
        reading(&[], books::refusing()),
        Some(KEYED),
    );

    let wirings = told(&ctx).await;

    assert!(
        matches!(
            wirings.first().map(|wiring| &wiring.state),
            Some(&crate::seed::State::Failed { .. })
        ),
        "a refused registration was not reported: {wirings:?}"
    );
}

/// A rehearsal names the aggregator the curator would be told to pull from, on the stack's
/// own network, and tells it nothing.
#[tokio::test]
async fn a_rehearsed_pass_names_the_aggregator_it_would_register_and_registers_none() {
    let http = reading(&[], books::taken());
    let ctx = curator_ctx("aggregators-rehearsed", http.clone(), Some(KEYED)).rehearsing();

    let wirings = told(&ctx).await;

    assert_eq!(
        wirings.first().map(|wiring| &wiring.state),
        Some(&crate::seed::State::WouldWire {
            yours: None,
            ours: Some("http://prowlarr:9696".to_owned()),
        }),
        "{wirings:?}"
    );
    assert!(books::added(&http.requests()).is_empty());
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
    let mut curator = crate::test_support::a_placed(PULLER, &[], Some(8787));
    curator.media_types = vec![media.to_owned()];
    curator.asks = vec![crate::plugin::Asking {
        capability: "indexer.search".to_owned(),
        each: false,
        native: None,
    }];
    let mut adapter = crate::test_support::a_placed(
        PULLER_ADAPTER,
        &[],
        Some(crate::test_support::CONTRACTED_PORT),
    );
    adapter.speaks = vec!["indexer.pull@1".to_owned()];
    adapter.fronts = Some(PULLER.to_owned());
    let mut installed = crate::test_support::an_installed(PULLER, vec![curator, adapter]);
    installed.manifest = crate::test_support::CONTRACTED_MANIFEST.to_owned();
    installed.tree = crate::test_support::A_TREE.to_owned();
    installed
}

/// The stack's indexer moved into its plugin beside [`puller`] filing `media`, written at
/// `project`, trusting `trusted` beside the bundle's plugins.
fn beside_the_puller_trusting(
    project: &std::path::Path,
    media: &str,
    trusted: &[crate::plugin::first_party::FirstParty],
) -> crate::wiring::Fillers {
    let trusted: Vec<crate::plugin::first_party::FirstParty> = crate::plugin::first_party::EMBEDDED
        .iter()
        .chain(trusted)
        .copied()
        .collect();
    let installed: Vec<crate::plugin::Installed> = INDEXER
        .installed()
        .into_iter()
        .chain([puller(media)])
        .collect();
    fillers_trusting(vec![indexer()], &installed, project, &trusted)
}

/// The same, with [`puller`] first-party.
fn beside_the_puller(project: &std::path::Path, media: &str) -> crate::wiring::Fillers {
    beside_the_puller_trusting(project, media, &crate::test_support::first_party(PULLER))
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
    let ctx = seed_ctx(true, None, None).with_http(http.clone());

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

/// A third-party plugin's curator pulling over the contract is never handed the indexer's
/// key: nothing is asked of it.
#[tokio::test]
async fn a_third_party_curator_pulling_over_the_contract_is_never_told_the_indexers_key() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("pulled-third-party");
    let http = Fake::by_path(vec![
        (HELD, Answer::reply(200, "[]")),
        (ADD, Answer::reply(204, "")),
    ]);
    let ctx = contracted_ctx(&project, PULLER_ADAPTER, true, http.clone());
    let fillers = beside_the_puller_trusting(&project, "books", &[]);

    let wirings = super::super::aggregators::seed_aggregators(&ctx, &fillers).await;

    assert!(wirings.is_empty(), "{wirings:?}");
    assert!(http.requests().is_empty(), "{:?}", http.requests());
}
