//! The indexer told the challenge solver it fetches through, in the API the solver answers
//! in.

use super::super::{pairings, unmatched, Connection};
use super::moved;
use crate::seed::State;
use crate::wiring::Chosen;

/// The port the challenge solver listens on inside the stack's network.
const LISTENS: u16 = 8191;

/// What the indexer's ask for a challenge solver comes to with the solver answering in
/// `api` on [`LISTENS`], inside the fence or not: the connection made, or why there is none.
fn solving(api: &str, fenced: bool) -> (Vec<Option<Connection>>, Vec<String>) {
    let fillers = moved(
        &[],
        &Chosen::default(),
        |_| (),
        |parts| {
            for upstream in parts
                .iter_mut()
                .flat_map(|part| part.services.iter_mut())
                .filter(|placed| placed.service == "flaresolverr")
            {
                upstream.native = Some(api.to_owned());
                upstream.listens = Some(LISTENS);
                if !fenced {
                    upstream.fields.network = None;
                }
            }
        },
    );
    let made = pairings(&fillers)
        .iter()
        .filter(|pairing| pairing.asker.id == "prowlarr" && pairing.filler.id == "flaresolverr")
        .map(|pairing| {
            pairing
                .made
                .clone()
                .ok()
                .map(|(connection, _, _)| connection)
        })
        .collect();
    let said = unmatched(&fillers)
        .into_iter()
        .filter(|wiring| wiring.connection == "FlareSolverr into Prowlarr")
        .filter_map(|wiring| match wiring.state {
            State::Unmatched { reason } => Some(reason),
            _ => None,
        })
        .collect();
    (made, said)
}

/// What the indexer's ask comes to with the solver answering in `api`, fenced as the
/// bundle pins it.
fn solving_in(api: &str) -> (Vec<Option<Connection>>, Vec<String>) {
    solving(api, true)
}

/// A solver answering in the `FlareSolverr` API is told to the indexer, over the contract
/// the indexer's adapter speaks.
#[test]
fn the_indexer_is_told_a_solver_answering_in_the_flaresolverr_api() {
    assert_eq!(
        solving_in("flaresolverr"),
        (vec![Some(Connection::Solver)], Vec::new())
    );
}

/// A solver answering in an API no indexer asks a solver in comes to nothing, and is said.
#[test]
fn a_solver_answering_in_another_api_is_reported() {
    assert_eq!(
        solving_in("sabnzbd"),
        (
            vec![None],
            vec![
                "FlareSolverr fills indexer.proxy, which Prowlarr asks for, and nothing in \
                 lemonfiber tells Prowlarr about a service answering in FlareSolverr's API"
                    .to_owned()
            ]
        )
    );
}

/// A solver answering in the `FlareSolverr` API outside the fence is not told to the
/// indexer, which would hand it the addresses it fetches past any adapter's screen, and
/// that is said.
#[test]
fn an_unfenced_solver_is_not_told_to_the_indexer() {
    assert_eq!(
        solving("flaresolverr", false),
        (
            vec![None],
            vec![
                "FlareSolverr fills indexer.proxy, which Prowlarr asks for, and is not fenced; \
                 Prowlarr would hand it the addresses it fetches, past any adapter's screen, so \
                 lemonfiber tells Prowlarr only about a fenced solver"
                    .to_owned()
            ]
        )
    );
}
