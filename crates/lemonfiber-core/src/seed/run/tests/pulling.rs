//! Telling a curator whose adapter pulls its indexers over `indexer.pull` about the
//! stack's indexer, and leaving one an indexer's app sync serves to that sync.

use super::super::connecting::{Connection, Unmade};
use super::aggregators::indexer;
use super::*;
use lemonfiber_manifest::ApiKind;

/// The plugin whose curator pulls its indexers over `indexer.pull`.
const PULLER: &str = "puller";

/// The adapter in front of [`PULLER`]'s curator.
const PULLER_ADAPTER: &str = "puller-adapter";

/// Where the adapter answers `add_aggregator`.
const ADD: &str = "/lemonfiber/indexer.pull/v1/add_aggregator";

/// Where the adapter answers `aggregators`.
const HELD: &str = "/lemonfiber/indexer.pull/v1/aggregators";

/// What [`PULLER`]'s curator files, and the API it answers in where it names one.
#[derive(Debug, Clone, Copy)]
struct Pulling<'a> {
    /// The media it files.
    media: &'a str,
    /// The API it answers in.
    native: Option<ApiKind>,
}

/// A curator filing `media` in an API of its own, which no indexer syncs.
const fn filing(media: &str) -> Pulling<'_> {
    Pulling {
        media,
        native: None,
    }
}

/// A curator filing `media` in the Servarr API, which the indexer syncs.
const fn servarr_filing(media: &str) -> Pulling<'_> {
    Pulling {
        media,
        native: Some(ApiKind::Servarr),
    }
}

/// [`PULLER`]: a curator filing as `pulling` says that asks for an indexer, and the adapter
/// in front of it speaking `indexer.pull`.
fn puller(pulling: Pulling<'_>) -> crate::plugin::Installed {
    let mut curator = crate::test_support::a_placed(PULLER, &[], Some(8787));
    curator.media_types = vec![pulling.media.to_owned()];
    curator.native = pulling.native.map(|kind| kind.name().to_owned());
    curator.asks = vec![crate::plugin::Asking {
        capability: "indexer.search".to_owned(),
        each: false,
        native: None,
        files: None,
        media_types: Vec::new(),
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

/// The stack's indexer moved into its plugin beside [`puller`] filing as `pulling` says,
/// written at `project`, trusting `trusted` beside the bundle's plugins.
fn beside_the_puller_trusting(
    project: &std::path::Path,
    pulling: Pulling<'_>,
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
        .chain([puller(pulling)])
        .collect();
    fillers_trusting(vec![indexer()], &installed, project, &trusted)
}

/// The same, with [`puller`] first-party.
fn beside_the_puller(project: &std::path::Path, pulling: Pulling<'_>) -> crate::wiring::Fillers {
    beside_the_puller_trusting(project, pulling, &crate::test_support::first_party(PULLER))
}

/// What the pass tells [`puller`] filing as `pulling` says, beside the stack's indexer,
/// with the adapter answering `added` to `add_aggregator`.
async fn pulled(tag: &str, pulling: Pulling<'_>, added: Answer) -> (Vec<Wiring>, Arc<Fake>) {
    let project = lemonfiber_fixtures::scratch::Scratch::new(&format!("pulled-{tag}"));
    let http = Fake::by_path(vec![(HELD, Answer::reply(200, "[]")), (ADD, added)]);
    let ctx = contracted_ctx(&project, PULLER_ADAPTER, true, http.clone());
    let fillers = beside_the_puller(&project, pulling);
    (
        super::super::aggregators::seed_aggregators(&ctx, &fillers).await,
        http,
    )
}

#[tokio::test]
async fn a_curator_pulling_over_the_contract_is_told_the_indexer_in_its_native_api() {
    let (wirings, http) = pulled("told", filing("books"), Answer::reply(204, "")).await;

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
        filing("books"),
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
async fn a_curator_the_indexers_app_sync_serves_is_never_told_to_pull_and_is_reported_as_synced() {
    let (wirings, http) = pulled("synced", servarr_filing("tv"), Answer::reply(204, "")).await;

    assert!(
        matches!(wirings.as_slice(), [one] if matches!(&one.state,
            crate::seed::State::Unmatched { reason }
                if reason.contains(" and syncs the media ") && reason.contains(PULLER))),
        "{wirings:?}"
    );
    assert!(
        !http.requests().iter().any(|asked| asked.url.ends_with(ADD)),
        "{:?}",
        http.requests()
    );
}

/// What the pass tells [`puller`], answering in the Servarr API and filing television,
/// where the stack's indexer answers `applications` with `synced`.
async fn pulled_beside_an_indexer_answering(tag: &str, synced: Answer) -> (Vec<Wiring>, Arc<Fake>) {
    pulled_beside_an_indexer(tag, synced, &[]).await
}

/// The same, with [`puller`]'s curator filling `filling`.
async fn pulled_beside_an_indexer(
    tag: &str,
    synced: Answer,
    filling: &[&str],
) -> (Vec<Wiring>, Arc<Fake>) {
    let project = lemonfiber_fixtures::scratch::Scratch::new(&format!("pulled-{tag}"));
    let http = Fake::by_path(vec![
        (HELD, Answer::reply(200, "[]")),
        (ADD, Answer::reply(204, "")),
        (
            crate::test_support::indexer::searching("applications"),
            synced,
        ),
    ]);
    let mut ctx = contracted_ctx(&project, PULLER_ADAPTER, true, http.clone());
    ctx.seams.engine = Arc::new(INDEXER.running().over(Arc::clone(&ctx.seams.engine)));
    ctx.seams.confined = crate::test_support::adapted::holding_keys(
        Arc::clone(&ctx.seams.confined),
        &[INDEXER.adapter],
    );
    let mut pulling = puller(servarr_filing("tv"));
    if let Some(curator) = pulling.services.first_mut() {
        curator.provides = filling.iter().map(|one| (*one).to_owned()).collect();
    }
    let trusted: Vec<crate::plugin::first_party::FirstParty> = crate::plugin::first_party::EMBEDDED
        .iter()
        .copied()
        .chain(crate::test_support::first_party(PULLER))
        .collect();
    let installed: Vec<crate::plugin::Installed> =
        INDEXER.installed().into_iter().chain([pulling]).collect();
    let fillers = fillers_trusting(vec![indexer()], &installed, &project, &trusted);
    (
        super::super::aggregators::seed_aggregators(&ctx, &fillers).await,
        http,
    )
}

#[tokio::test]
async fn a_curator_an_indexer_registers_itself_is_left_to_it_and_said_nothing_of() {
    let (wirings, http) =
        pulled_beside_an_indexer("registered", Answer::reply(200, "[]"), &["library.curate"]).await;

    assert!(wirings.is_empty(), "{wirings:?}");
    assert!(
        !http.requests().iter().any(|asked| asked.url.ends_with(ADD)),
        "{:?}",
        http.requests()
    );
}

#[tokio::test]
async fn a_curator_whose_indexer_cannot_say_whether_it_syncs_is_reported_and_told_nothing() {
    let (wirings, http) = pulled_beside_an_indexer_answering(
        "app-sync-refused",
        Answer::served(
            500,
            crate::ports::http::PROBLEM,
            r#"{"type":"refused","detail":"the indexer refused the read"}"#,
        ),
    )
    .await;

    assert!(
        matches!(wirings.as_slice(), [one] if matches!(one.state,
            crate::seed::State::Failed { .. } | crate::seed::State::Skipped { .. })),
        "{wirings:?}"
    );
    assert!(
        !http.requests().iter().any(|asked| asked.url.ends_with(ADD)),
        "{:?}",
        http.requests()
    );
}

#[tokio::test]
async fn a_curator_an_app_sync_would_serve_is_told_an_indexer_that_has_none() {
    let (wirings, http) = pulled_beside_an_indexer_answering(
        "no-app-sync",
        Answer::served(
            500,
            crate::ports::http::PROBLEM,
            r#"{"type":"unsupported","detail":"it is told about no application"}"#,
        ),
    )
    .await;

    assert!(
        matches!(wirings.as_slice(), [one] if one.state == crate::seed::State::Wired),
        "{wirings:?}"
    );
    assert!(
        http.requests().iter().any(|asked| asked.url.ends_with(ADD)),
        "{:?}",
        http.requests()
    );
}

#[tokio::test]
async fn a_curator_an_indexers_app_sync_answers_for_is_left_to_it() {
    let (wirings, http) =
        pulled_beside_an_indexer_answering("app-sync", Answer::reply(200, "[]")).await;

    assert!(
        matches!(wirings.as_slice(), [one] if matches!(&one.state,
            crate::seed::State::Unmatched { reason } if reason.contains(" and syncs the media "))),
        "{wirings:?}"
    );
    assert!(
        !http.requests().iter().any(|asked| asked.url.ends_with(ADD)),
        "{:?}",
        http.requests()
    );
}

#[tokio::test]
async fn a_curator_filing_synced_media_in_an_api_of_its_own_is_told_the_indexer() {
    let (wirings, http) = pulled("own-api", filing("tv"), Answer::reply(204, "")).await;

    assert!(
        matches!(wirings.as_slice(), [one] if one.state == crate::seed::State::Wired),
        "{wirings:?}"
    );
    assert!(
        http.requests().iter().any(|asked| asked.url.ends_with(ADD)),
        "{:?}",
        http.requests()
    );
}

#[tokio::test]
async fn a_curator_pulling_over_the_contract_its_adapter_does_not_answer_is_told_nothing() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("pulled-unanswered");
    let http = Fake::always(Answer::reply(204, ""));
    let ctx = seed_ctx(true, None, None).with_http(http.clone());

    let wirings = super::super::aggregators::seed_aggregators(
        &ctx,
        &beside_the_puller(&project, filing("books")),
    )
    .await;

    assert!(wirings.is_empty(), "{wirings:?}");
    assert!(http.requests().is_empty(), "{:?}", http.requests());
}

/// What the pairing of [`puller`], filing as `pulling` says, with the stack's indexer
/// comes to.
fn connected(tag: &str, pulling: Pulling<'_>) -> Vec<Result<Connection, Unmade>> {
    let project = lemonfiber_fixtures::scratch::Scratch::new(&format!("connected-{tag}"));
    let fillers = beside_the_puller(&project, pulling);
    super::super::connecting::pairings(&fillers)
        .into_iter()
        .filter(|pairing| pairing.asker.id == PULLER)
        .map(|pairing| pairing.made.map(|(made, _, _)| made))
        .collect()
}

#[test]
fn only_a_curator_answering_in_the_servarr_api_is_left_to_the_indexers_app_sync() {
    assert_eq!(
        connected("servarr", servarr_filing("tv")),
        vec![Ok(Connection::Aggregator { synced: true })]
    );
    assert_eq!(
        connected("own-api", filing("tv")),
        vec![Ok(Connection::Aggregator { synced: false })]
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
    let fillers = beside_the_puller_trusting(&project, filing("books"), &[]);

    let wirings = super::super::aggregators::seed_aggregators(&ctx, &fillers).await;

    assert!(wirings.is_empty(), "{wirings:?}");
    assert!(http.requests().is_empty(), "{:?}", http.requests());
}
