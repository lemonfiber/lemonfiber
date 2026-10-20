//! The one reading of a declared path or template as a place on an address: the guard's
//! question on its addressee, and every location the core gives a member.

use super::{Locations, BASE, ITEM};

/// `path` on `base`, where it stays there exactly as written: joined onto `base` with the
/// URL parser every reader uses, its scheme, host and port must be `base`'s, and what
/// follows them must be `path` byte for byte, so nothing in it is read as another host,
/// re-encoded, dropped or resolved away.
#[must_use]
pub fn joined(base: &str, path: &str) -> Option<String> {
    let parsed = url::Url::parse(base).ok()?;
    let unpathed = parsed.path() == "/" && parsed.query().is_none() && parsed.fragment().is_none();
    if !unpathed || !path.starts_with('/') || path.starts_with("//") {
        return None;
    }
    let on = parsed.join(path).ok()?;
    let same_place = on.scheme() == parsed.scheme()
        && on.host() == parsed.host()
        && on.port_or_known_default() == parsed.port_or_known_default()
        && on.username().is_empty()
        && on.password().is_none();
    let query = on
        .query()
        .map(|query| format!("?{query}"))
        .unwrap_or_default();
    let as_written = on.fragment().is_none() && format!("{}{query}", on.path()) == path;
    (same_place && as_written).then(|| on.into())
}

impl Locations {
    /// Where `template`, one of these, locates `item` on the household address `base`.
    #[must_use]
    pub fn located(template: &str, base: &str, item: &str) -> Option<String> {
        let rest = template.strip_prefix(BASE)?;
        joined(base, &rest.replace(ITEM, item))
    }
}

#[cfg(test)]
mod tests;
