//! Telling the indexer the challenge solver it fetches through.

use super::*;
use crate::ports::service::{KnownSolver, Protocol, SolverAt};
use crate::test_support::adapted::{answered, asked_at, refused, unavailable, Adapted};
use crate::test_support::indexer::{searching, taken, INDEXER};
use lemonfiber_contract::capabilities::indexer::proxy;
use lemonfiber_contract::wire::Kind;

/// The bundle's challenge solver and the adapter in front of it.
const SOLVER: Adapted = Adapted {
    plugin: "flaresolverr",
    adapter: "flaresolverr-adapter",
    port: 8191,
};

/// Where the solver is asked whether it answers.
fn answers() -> &'static str {
    SOLVER.at(proxy::CAPABILITY, proxy::MAJOR, "answers")
}

/// Where the indexer reaches the challenge solver.
const AT: &str = "http://flaresolverr:8191";

/// The bundle's challenge solver moved into its plugin, answering in the `FlareSolverr` API.
fn solving() -> Vec<crate::plugin::Installed> {
    crate::test_support::adapted::installed_parts()
        .into_iter()
        .filter(|part| part.plugin == "flaresolverr")
        .collect()
}

/// The moved indexer fetching through `held`, and answering an added solver with `added`,
/// beside a solver that answers.
fn fetching_through(held: &[KnownSolver], added: Answer) -> Arc<Fake> {
    answered_in_turn(held, added, vec![answered(&true)])
}

/// The same, the solver answering each of `answering` in turn, the last repeating.
fn answered_in_turn(held: &[KnownSolver], added: Answer, answering: Vec<Answer>) -> Arc<Fake> {
    Fake::by_path_in_turn(vec![
        (searching("proxies"), vec![answered(&held.to_vec())]),
        (searching("add_proxy"), vec![added]),
        (answers(), answering),
    ])
}

/// What the pass tells the moved indexer, asked over `http`, rehearsing where `rehearsing`.
async fn told(name: &str, http: Arc<Fake>, rehearsing: bool) -> Vec<Wiring> {
    let mut ctx =
        crate::test_support::adapted::asking(indexing(name, http, false), &[INDEXER, SOLVER]);
    ctx.dry_run = rehearsing;
    super::super::solvers::seed_solvers(&ctx, &indexed(Vec::new(), &solving())).await
}

/// How often the solver was asked whether it answers in `sent`.
fn asked_whether(sent: &[crate::ports::http::Request]) -> usize {
    asked_at(sent, answers()).len()
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
        (answers(), answered(&true)),
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

/// The indexer tries a solver as soon as it is told about it, so a solver still coming up
/// is asked again until it answers, and only then is the indexer told.
#[tokio::test(start_paused = true)]
async fn a_solver_still_coming_up_is_told_to_the_indexer_once_it_answers() {
    let http = answered_in_turn(
        &[],
        taken(),
        vec![unavailable(), answered(&false), answered(&true)],
    );

    let wirings = told("solvers-coming-up", http.clone(), false).await;

    assert!(
        matches!(wirings.as_slice(), [one] if one.state == State::Wired),
        "{wirings:?}"
    );
    assert_eq!(asked_whether(&http.requests()), 3);
    assert_eq!(added(&http.requests()), vec![the_solver()]);
}

/// A solver that never answers is reported, naming it and how long it was given, and the
/// indexer is told nothing.
#[tokio::test(start_paused = true)]
async fn a_solver_that_never_answers_is_reported_and_the_indexer_told_nothing() {
    let http = answered_in_turn(&[], taken(), vec![unavailable()]);

    let wirings = told("solvers-silent", http.clone(), false).await;

    let waited = crate::patience::STARTING.longest().as_secs();
    assert!(
        matches!(wirings.as_slice(), [one] if one.state == State::Failed {
            detail: format!(
                "FlareSolverr did not answer within {waited} seconds of being asked, so \
                 Prowlarr was not told about it"
            ),
        }),
        "{wirings:?}"
    );
    assert!(added(&http.requests()).is_empty());
}

/// A rehearsal does not wait on the solver.
#[tokio::test]
async fn a_rehearsal_does_not_ask_whether_the_solver_answers() {
    let http = answered_in_turn(&[], taken(), vec![unavailable()]);

    let _ = told("solvers-rehearsed-silent", http.clone(), true).await;

    assert_eq!(asked_whether(&http.requests()), 0);
}

/// A solver this machine cannot ask over its contract is not waited on: the indexer's own
/// test of it is what is left to say whether it answers.
#[tokio::test]
async fn a_solver_asked_over_no_contract_is_told_without_waiting() {
    let http = answered_in_turn(&[], taken(), vec![unavailable()]);
    let ctx = indexing("solvers-uncontracted", http.clone(), false);

    let wirings = super::super::solvers::seed_solvers(&ctx, &indexed(Vec::new(), &solving())).await;

    assert!(
        matches!(wirings.as_slice(), [one] if one.state == State::Wired),
        "{wirings:?}"
    );
    assert_eq!(asked_whether(&http.requests()), 0);
}

/// The moved indexer answering what it holds with each of `holding` in turn, the last
/// repeating, and an added solver with `added`, beside a solver that answers.
fn holding_in_turn(holding: Vec<Answer>, added: Answer) -> Arc<Fake> {
    Fake::by_path_in_turn(vec![
        (searching("proxies"), holding),
        (searching("add_proxy"), vec![added]),
        (answers(), vec![answered(&true)]),
    ])
}

/// An indexer that did not answer being told about the solver is still trying it, so it is
/// read until it holds the solver, reported wired, and never told about it again.
#[tokio::test(start_paused = true)]
async fn an_indexer_that_did_not_answer_being_told_is_read_until_it_holds_the_solver() {
    let none: &[KnownSolver] = &[];
    let http = holding_in_turn(
        vec![
            answered(&none),
            answered(&none),
            answered(&[known(AT, &["lemonfiber-solver"])]),
        ],
        unavailable(),
    );

    let wirings = told("solvers-trying", http.clone(), false).await;

    assert!(
        matches!(wirings.as_slice(), [one] if one.state == State::Wired),
        "{wirings:?}"
    );
    assert_eq!(added(&http.requests()), vec![the_solver()]);
}

/// An indexer that did not answer being told about the solver and never comes to hold it
/// is reported not answering, so a later pass tells it again, and it is told once here.
#[tokio::test(start_paused = true)]
async fn an_indexer_that_never_holds_a_solver_it_did_not_answer_for_is_not_answering() {
    let none: &[KnownSolver] = &[];
    let http = holding_in_turn(vec![answered(&none)], unavailable());

    let wirings = told("solvers-never-held", http.clone(), false).await;

    assert!(
        matches!(wirings.as_slice(), [one] if one.state == State::Skipped {
            reason: crate::seed::NOT_ANSWERING.to_owned(),
        }),
        "{wirings:?}"
    );
    assert_eq!(added(&http.requests()), vec![the_solver()]);
}
