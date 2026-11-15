//! Telling the indexer the challenge solver it fetches through.

use super::*;
use crate::ports::service::{KnownSolver, Protocol, SolverAt};
use crate::test_support::adapted::{asked_at, refused};
use crate::test_support::indexer::{answering, searching, taken};
use lemonfiber_contract::wire::Kind;

/// Where the indexer reaches the challenge solver.
const AT: &str = "http://flaresolverr:8191";

/// The bundle's challenge solver moved into its plugin, answering in the `FlareSolverr` API.
fn solving() -> Vec<crate::plugin::Installed> {
    crate::test_support::adapted::installed_parts()
        .into_iter()
        .filter(|part| part.plugin == "flaresolverr")
        .collect()
}

/// The moved indexer fetching through `held`, and answering an added solver with `added`.
fn fetching_through(held: &[KnownSolver], added: Answer) -> Arc<Fake> {
    Fake::by_path(vec![
        answering("proxies", &held.to_vec()),
        (searching("add_proxy"), added),
    ])
}

/// What the pass tells the moved indexer, asked over `http`, rehearsing where `rehearsing`.
async fn told(name: &str, http: Arc<Fake>, rehearsing: bool) -> Vec<Wiring> {
    let mut ctx = indexing(name, http, false);
    ctx.dry_run = rehearsing;
    super::super::solvers::seed_solvers(&ctx, &indexed(Vec::new(), &solving())).await
}

/// The solver as the indexer is told about it.
fn the_solver() -> SolverAt {
    SolverAt {
        name: "FlareSolverr".to_owned(),
        url: AT.to_owned(),
        protocol: Protocol("flaresolverr".to_owned()),
        tag: "lemonfiber-solver".to_owned(),
    }
}

/// Each solver the indexer was told about in `sent`.
fn added(sent: &[crate::ports::http::Request]) -> Vec<SolverAt> {
    asked_at(sent, searching("add_proxy"))
        .into_iter()
        .filter_map(|asked| serde_json::from_value(asked.get("solver")?.clone()).ok())
        .collect()
}

/// A solver at `url` under `tags`.
fn known(url: &str, tags: &[&str]) -> KnownSolver {
    KnownSolver {
        id: "1".to_owned(),
        name: "FlareSolverr".to_owned(),
        url: url.to_owned(),
        protocol: Protocol("flaresolverr".to_owned()),
        tags: tags.iter().map(|tag| (*tag).to_owned()).collect(),
    }
}

/// The indexer is told the solver over its contract, in the solver's API, at the address
/// it listens on, under the tag, and the report names both.
#[tokio::test]
async fn the_indexer_is_told_the_solver_under_the_tag() {
    let http = fetching_through(&[], taken());

    let wirings = told("solvers-told", http.clone(), false).await;

    assert_eq!(
        wirings
            .iter()
            .map(|wiring| (wiring.connection.as_str(), &wiring.state))
            .collect::<Vec<_>>(),
        vec![("FlareSolverr into Prowlarr", &State::Wired)]
    );
    assert_eq!(added(&http.requests()), vec![the_solver()]);
}

/// A solver already held at that address under the tag is left alone.
#[tokio::test]
async fn a_solver_already_held_under_the_tag_is_not_added_again() {
    let http = fetching_through(&[known(AT, &["lemonfiber-solver"])], taken());

    let wirings = told("solvers-held", http.clone(), false).await;

    assert!(
        matches!(wirings.as_slice(), [one] if one.state == State::AlreadyWired),
        "{wirings:?}"
    );
    assert!(added(&http.requests()).is_empty());
}

/// A solver held at that address under no tag of ours, or one under the tag elsewhere,
/// is no solver the indexer sends a tagged site through, so it is added.
#[tokio::test]
async fn a_solver_held_elsewhere_or_untagged_is_added() {
    let http = fetching_through(
        &[
            known(AT, &["the-operators"]),
            known("http://elsewhere:8191", &["lemonfiber-solver"]),
        ],
        taken(),
    );

    let wirings = told("solvers-elsewhere", http.clone(), false).await;

    assert!(
        matches!(wirings.as_slice(), [one] if one.state == State::Wired),
        "{wirings:?}"
    );
    assert_eq!(added(&http.requests()), vec![the_solver()]);
}

/// A rehearsal reads what the indexer holds and adds nothing.
#[tokio::test]
async fn a_rehearsal_adds_no_solver() {
    let http = fetching_through(&[], taken());

    let wirings = told("solvers-rehearsed", http.clone(), true).await;

    assert!(
        matches!(wirings.as_slice(), [one] if one.state == State::WouldWire {
            yours: None,
            ours: Some(AT.to_owned()),
        }),
        "{wirings:?}"
    );
    assert!(added(&http.requests()).is_empty());
}

/// An indexer that cannot fetch through a solver of that API leaves the pair unpaired, in
/// its own words.
#[tokio::test]
async fn an_indexer_answering_unsupported_leaves_the_pair_unpaired() {
    let http = fetching_through(&[], refused(Kind::Unsupported));

    let wirings = told("solvers-unsupported", http, false).await;

    assert!(
        matches!(wirings.as_slice(), [one] if matches!(&one.state, State::Unmatched { reason }
            if reason.starts_with("Prowlarr cannot fetch through FlareSolverr, which it would \
                                   ask in the flaresolverr API: "))),
        "{wirings:?}"
    );
}

/// An indexer refusing the solver is reported unreached, not wired.
#[tokio::test]
async fn an_indexer_refusing_the_solver_is_reported_unreached() {
    let http = fetching_through(&[], refused(Kind::Refused));

    let wirings = told("solvers-refused", http, false).await;

    assert!(
        matches!(wirings.as_slice(), [one] if !matches!(one.state,
            State::Wired | State::AlreadyWired | State::Unmatched { .. })),
        "{wirings:?}"
    );
}

/// An indexer that would not say which solvers it fetches through is reported unreached,
/// and nothing is added to it.
#[tokio::test]
async fn an_indexer_that_will_not_say_its_solvers_is_reported_unreached() {
    let http = Fake::by_path(vec![
        (searching("proxies"), refused(Kind::Refused)),
        (searching("add_proxy"), taken()),
    ]);

    let wirings = told("solvers-unread", http.clone(), false).await;

    assert!(
        matches!(wirings.as_slice(), [one] if !matches!(one.state,
            State::Wired | State::AlreadyWired | State::Unmatched { .. })),
        "{wirings:?}"
    );
    assert!(added(&http.requests()).is_empty());
}

/// An indexer this machine cannot ask over its contract is told nothing and says nothing.
#[tokio::test]
async fn an_indexer_nothing_can_ask_is_passed_over() {
    let http = fetching_through(&[], taken());
    let ctx = seed_ctx(true, None, None).with_http(http.clone());

    let wirings = super::super::solvers::seed_solvers(&ctx, &indexed(Vec::new(), &solving())).await;

    assert!(wirings.is_empty(), "{wirings:?}");
    assert!(http.requests().is_empty(), "{:?}", http.requests());
}
