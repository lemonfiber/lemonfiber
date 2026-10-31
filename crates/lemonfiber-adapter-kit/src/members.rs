//! The guard's question about one item, answered for the member whose token it carries.
//!
//! A media server that serves an item's bytes under an id no question to it takes has
//! its guard ask the adapter instead, at [`MAY`]. That one route is authorised by the
//! member's token, presented as the declaration presents it to the upstream, never by the
//! plugin's key: the adapter answers it only by asking its upstream with that token, so
//! the member's own limits decide.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use axum::extract::{RawQuery, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::Response;
use lemonfiber_ports::service::{spelled_as_a_space, Answered, Failure, Presentation, TOKEN};

use crate::Kit;

pub use lemonfiber_ports::service::{MAY, SPACE_MOST};

/// The path [`MAY`] is asked at, without its query.
pub(crate) fn path() -> &'static str {
    MAY.split_once('?').map_or(MAY, |(path, _)| path)
}

/// The query parameter the item is named by.
const ITEM: &str = "item";

/// The query parameter the space the item's id is in is named by.
const SPACE: &str = "space";

/// The longest item id the question names, in bytes.
pub const ITEM_MOST: usize = 128;

/// The longest token the question carries, in bytes.
pub const TOKEN_MOST: usize = 4096;

/// Asking the upstream, with the member's token, whether that member may have the bytes of
/// an item in a space: the space, the item and the token, in that order. A space the
/// adapter does not know is answered [`Answered::MayNot`].
pub type Asking = Arc<
    dyn Fn(
            String,
            String,
            String,
        ) -> Pin<Box<dyn Future<Output = Result<Answered, Failure>> + Send>>
        + Send
        + Sync,
>;

/// Who answers the guard's question, and where the member's token is presented.
#[derive(Clone)]
pub(crate) struct Members {
    /// Where the guard presents the member's token, as the declaration presents it.
    pub(crate) presented: Presentation,
    /// What asks the upstream.
    pub(crate) asking: Asking,
}

/// The guard's question, answered by asking the upstream with the member's token; a
/// question carrying no token, or the plugin's key, answered as no session without asking.
pub(crate) async fn may(
    State(kit): State<Arc<Kit>>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
) -> Response {
    let Some(members) = &kit.members else {
        return crate::answered(StatusCode::NOT_FOUND, None, Vec::new());
    };
    let query = query.unwrap_or_default();
    let named = one_of(&query, SPACE)
        .filter(|space| spelled_as_a_space(space))
        .zip(item(&query));
    let Some((space, item)) = named else {
        return status(Answered::MayNot.status());
    };
    let token = token(&members.presented, &headers, &query)
        .filter(|token| !crate::same_secret(&kit.key, token));
    let Some(token) = token else {
        return status(Answered::NoSession.status());
    };
    match (members.asking)(space, item, token).await {
        Ok(answered) => status(answered.status()),
        Err(_) => status(StatusCode::BAD_GATEWAY.as_u16()),
    }
}

/// A response with `status` and nothing else.
fn status(status: u16) -> Response {
    crate::answered(
        StatusCode::from_u16(status).unwrap_or(StatusCode::BAD_GATEWAY),
        None,
        Vec::new(),
    )
}

/// The one item `query` names, where it names exactly one and that one is plain.
fn item(query: &str) -> Option<String> {
    one_of(query, ITEM).filter(|item| {
        (1..=ITEM_MOST).contains(&item.len())
            && item
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    })
}

/// The value of the one parameter `query` carries as `parameter`, where it carries exactly
/// one.
fn one_of(query: &str, parameter: &str) -> Option<String> {
    let mut named = form_urlencoded::parse(query.as_bytes())
        .filter(|(name, _)| name == parameter)
        .map(|(_, value)| value.into_owned());
    let value = named.next()?;
    named.next().is_none().then_some(value)
}

/// The member's token, where it is presented as `presented` says, once, on one line, and
/// no longer than [`TOKEN_MOST`].
fn token(presented: &Presentation, headers: &HeaderMap, query: &str) -> Option<String> {
    let token = match presented {
        Presentation::AsIs => one(headers, header::AUTHORIZATION.as_str())?
            .strip_prefix("Bearer ")?
            .to_owned(),
        Presentation::Header { name, value } => {
            let (before, after) = value.split_once(TOKEN)?;
            one(headers, name)?
                .strip_prefix(before)?
                .strip_suffix(after)?
                .to_owned()
        }
        Presentation::Query { parameter } => one_of(query, parameter)?,
    };
    let plain = (1..=TOKEN_MOST).contains(&token.len())
        && token.bytes().all(|byte| (b'!'..=b'~').contains(&byte));
    plain.then_some(token)
}

/// The one value of the header `name`, where there is exactly one and it is text.
fn one<'h>(headers: &'h HeaderMap, name: &str) -> Option<&'h str> {
    let mut values = headers.get_all(name).iter();
    let value = values.next()?;
    values.next().is_none().then(|| value.to_str().ok())?
}

#[cfg(test)]
mod tests;
