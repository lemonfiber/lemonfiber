//! What one of this product's words means.
//!
//! The one read that is about lemonfiber rather than about a stack: it is answered
//! from a table compiled into the binary, so it needs neither a stack running nor a
//! daemon reachable. A browser meeting `indexer` in a report can ask what it means
//! without anything else being up.
//!
//! Served rather than shipped. The words live in one table, and a surface carrying
//! its own copy of them would be a surface explaining a word its own way — which is
//! the drift that ends with two answers to what `hardlink` means.

use axum::extract::{RawQuery, State};
use axum::response::Response;
use axum::routing::get;
use axum::Router;

use crate::reads::{Wanted, EXPLAIN};
use crate::router::Serving;

use super::{reading, Asked};

/// The parameter naming the word to explain.
const WORD: &str = "word";

/// The read about this product's own words.
pub(super) fn routes() -> Router<Serving> {
    Router::new().route(EXPLAIN, get(explain))
}

/// What one word means, or every word there is to ask about.
///
/// Naming none lists them and naming one explains that one, which is the fork
/// `lemonfiber explain` takes on the same word — and the listing is what a caller
/// that has never seen the vocabulary needs before it can name anything.
///
/// A word this product does not explain is refused rather than answered with the
/// list, which is the same refusal the command line reports and carries the list in
/// its detail. Naming an empty word is naming one it does not explain, and is
/// refused for that rather than read as having named none.
async fn explain(State(serving): State<Serving>, RawQuery(query): RawQuery) -> Response {
    let asked = Asked::read(query.as_deref());
    reading(
        &serving.ctx,
        EXPLAIN,
        Wanted {
            word: asked.one(WORD).map(str::to_owned),
            ..Wanted::default()
        },
    )
    .await
}
