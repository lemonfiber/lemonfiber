//! The header names and encodings every HTTP request the core builds shares.

use std::fmt::Write as _;

/// The header a service of the Servarr shape authenticates with — the setup validator
/// sends a key in it, and a first run presents the key it read in it, so its name is
/// written once.
pub(crate) const API_KEY_HEADER: &str = "X-Api-Key";

/// The header a service that takes a scheme and a credential reads them from.
pub(crate) const AUTHORIZATION: &str = "Authorization";

/// The media type a JSON request declares, so a service binds the body rather
/// than refusing it.
use crate::ports::http::JSON;

/// The `Content-Type` header a JSON request carries: present only where the
/// request has a body, absent on a bodiless GET. Returned as an `Option` so a
/// caller folds it into its header list with `extend`.
pub(crate) fn json_content_type(body: Option<&String>) -> Option<(String, String)> {
    body.map(|_| ("Content-Type".to_owned(), JSON.to_owned()))
}

/// `value` as a query carries it: every byte outside the unreserved set
/// written as `%XX`, a space included, so no reader takes a `+` for one.
pub(crate) fn query_encoded(value: &str) -> String {
    value.bytes().fold(String::new(), |mut out, byte| {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(byte));
        } else {
            let _ = write!(out, "%{byte:02X}");
        }
        out
    })
}

#[cfg(test)]
mod tests;
