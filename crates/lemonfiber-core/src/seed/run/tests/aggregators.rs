//! Telling a curator that pulls its indexers where its aggregator is.

use super::*;
use crate::ports::service::{AggregatorAt, Protocol};
use crate::test_support::books;

/// The key lemonfiber minted for the moved indexer.
const KEYED: &str = "the-key";

/// The stack's indexer, answering the book curator's ask for one.
pub(super) fn indexer() -> lemonfiber_manifest::Service {
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
    pulling_again(held, added, books::taken())
}

/// The same, answering each ask to pull its indexers again with `pulled`.
fn pulling_again(
    held: &[crate::ports::service::KnownAggregator],
    added: Answer,
    pulled: Answer,
) -> Arc<Fake> {
    Fake::by_path(vec![
        books::aggregators(held),
        books::add_aggregator(added),
        (books::pulling("pull"), pulled),
    ])
}

/// How many times the moved curator was asked to pull its indexers again in `sent`.
fn pulls(sent: &[crate::ports::http::Request]) -> usize {
    sent.iter()
        .filter(|one| one.url.ends_with(books::pulling("pull")))
        .count()
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
    assert_eq!(pulls(&http.requests()), 1);
}

/// A curator already reading from the aggregator that cannot be asked to pull again keeps
/// the pair as it was, and one whose pull fails is reported in its own words.
#[tokio::test]
async fn a_curator_asked_to_pull_again_that_cannot_keeps_the_pair_and_one_that_fails_is_reported() {
    let known = [books::reading_from("http://prowlarr:9696", true)];
    let unsupported = told(&curator_ctx(
        "aggregators-pull-unsupported",
        pulling_again(
            &known,
            books::taken(),
            crate::test_support::adapted::refused(lemonfiber_contract::wire::Kind::Unsupported),
        ),
        Some(KEYED),
    ))
    .await;
    let refused = told(&curator_ctx(
        "aggregators-pull-refused",
        pulling_again(&known, books::taken(), books::refusing()),
        Some(KEYED),
    ))
    .await;

    assert_eq!(
        unsupported.first().map(|wiring| &wiring.state),
        Some(&crate::seed::State::AlreadyWired),
        "{unsupported:?}"
    );
    assert!(
        refused
            .first()
            .is_some_and(|wiring| matches!(wiring.state, crate::seed::State::Failed { .. })),
        "{refused:?}"
    );
}

/// A rehearsal over a curator already reading from the aggregator asks it nothing.
#[tokio::test]
async fn a_rehearsal_asks_no_curator_to_pull_again() {
    let http = reading(
        &[books::reading_from("http://prowlarr:9696", true)],
        books::taken(),
    );
    let ctx = curator_ctx("aggregators-pull-rehearsed", http.clone(), Some(KEYED)).rehearsing();

    let wirings = told(&ctx).await;

    assert_eq!(
        wirings.first().map(|wiring| &wiring.state),
        Some(&crate::seed::State::AlreadyWired),
        "{wirings:?}"
    );
    assert_eq!(pulls(&http.requests()), 0);
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

/// Nothing to do where either end is absent, and a later run's to do where lemonfiber does
/// not hold the aggregator's key yet.
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
    assert!(
        matches!(unwritten.as_slice(), [one] if is_skipped(one)),
        "{unwritten:?}"
    );
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

/// An aggregator declaring no key of its own is never told to the moved curator with
/// another of its secrets in the key's place: the pair is said as unpaired, naming why.
#[tokio::test]
async fn an_aggregator_declaring_no_key_of_its_own_is_reported_unpaired() {
    let mut unkeyed = kept_aggregator();
    unkeyed.declared.secrets.clear();
    let trusted: Vec<crate::plugin::first_party::FirstParty> = crate::plugin::first_party::EMBEDDED
        .iter()
        .copied()
        .chain(first_party("kept"))
        .collect();
    let fillers = moved_trusting(
        vec![indexer()],
        vec![unkeyed],
        "indexer.search=kept",
        &trusted,
    );
    let http = reading(&[], books::taken());
    let ctx = curator_ctx("aggregators-unkeyed", http.clone(), None);

    let wirings = super::super::aggregators::seed_aggregators(&ctx, &fillers).await;

    assert!(
        matches!(wirings.as_slice(), [one] if matches!(&one.state,
            crate::seed::State::Unmatched { reason } if reason.contains("declares no key"))),
        "{wirings:?}"
    );
    assert!(books::added(&http.requests()).is_empty());
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
