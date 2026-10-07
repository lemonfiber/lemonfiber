//! Where a recipe's call goes, built the one way both reading a manifest and making the
//! call build it.
//!
//! **One function decides.** The address a call is sent to is built here, and reading a
//! manifest builds the same address with the same function and refuses whatever it
//! will not build. A rule held in two places is two rules, and the gap between a check
//! that passed and a call that went elsewhere is where the two disagree; with one
//! function there is no second reading to disagree with.
//!
//! **Built, never written.** The address starts from a scheme and a placeholder host,
//! and its host, port, path and query are each set on it, so no path can be read as
//! another host. Then it is held to its destination: the host it carries is the one it
//! was given, as written, on the port it was given, and it carries no user, password or
//! fragment. A host an address would carry spelled any other way, such as a hexadecimal
//! address written as a name, is refused rather than followed.
//!
//! **A path is plain.** One leading `/`, then segments and an optional query. Text that
//! is not could still be read as one more host by whatever joined them: `//elsewhere`,
//! `@elsewhere`, a backslash some parsers read as a slash, a `#` that ends the path
//! early, or the same written percent-encoded. A dot segment walks somewhere the
//! destination does not name.
//!
//! **A substitution is read one way.** `{{name}}` is found in a call's text by
//! [`pieces`], and reading a manifest and making the call both read it there.

use url::Url;

use crate::schema::StepCall;

/// What opens a substitution.
pub const OPENS: &str = "{{";

/// What closes one.
pub const CLOSES: &str = "}}";

/// What every path begins with, once.
const ROOT: char = '/';

/// What divides a path from its query.
pub const QUERY: char = '?';

/// What divides one parameter of a query from the next.
pub const PARAMETERS: char = '&';

/// What divides a parameter's name from its value.
pub const VALUED: char = '=';

/// The address every service in the stack is reached on from this machine.
const HERE: &str = "127.0.0.1";

/// The scheme a service in the stack is reached over.
const PLAIN_SCHEME: &str = "http";

/// The scheme a host outside it is reached over, on that scheme's own port.
const SECURE_SCHEME: &str = "https";

/// The host an address is parsed with before its own is set on it.
const BLANK: &str = "unset.invalid";

/// Characters a path never holds, each with what it could do there.
const NEVER: &[(char, &str)] = &[
    (
        '@',
        "an `@`, which a URL reads as ending a user and beginning a host",
    ),
    ('\\', "a backslash, which some parsers read as a slash"),
    ('#', "a `#`, which ends the path and starts a fragment"),
];

/// Percent-encodings a path never holds, each with what it decodes to.
const NEVER_ENCODED: &[(&str, &str)] = &[
    ("%2F", "`/`"),
    ("%5C", "a backslash"),
    ("%40", "`@`"),
    ("%23", "`#`"),
    ("%2E", "`.`"),
];

/// Where a call goes, as its address is built.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Toward<'a> {
    /// A service in the stack, at the port it publishes on this machine.
    Stack(u16),
    /// A host outside it, by the name the manifest writes.
    Outside(&'a str),
}

/// Why no address could be built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unaddressed {
    /// The path is not a plain absolute path, and why.
    Path(String),
    /// The host would not be carried as itself, and what an address makes of it.
    Host(String),
}

/// One piece of a call's text: written out, or a substitution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Piece<'a> {
    /// Text as written.
    Written(&'a str),
    /// A `{{name}}`, holding what stands between the braces as written.
    Named(&'a str),
}

impl<'a> Piece<'a> {
    /// The name a substitution stands for, where this is one.
    #[must_use]
    pub fn name(self) -> Option<&'a str> {
        match self {
            Self::Written(_) => None,
            Self::Named(name) => Some(name.trim()),
        }
    }
}

/// A call's text in the pieces it is made of.
///
/// A substitution runs from a `{{` to the next `}}` with no other `{{` between them; a
/// `{{` that no `}}` closes that way is written text.
#[must_use]
pub fn pieces(text: &str) -> Vec<Piece<'_>> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find(OPENS) {
        let after = &rest[at + OPENS.len()..];
        match after.find(CLOSES) {
            Some(end) if !after[..end].contains(OPENS) => {
                out.extend((at > 0).then_some(Piece::Written(&rest[..at])));
                out.push(Piece::Named(&after[..end]));
                rest = &after[end + CLOSES.len()..];
            }
            _ => {
                out.push(Piece::Written(&rest[..at + OPENS.len()]));
                rest = after;
            }
        }
    }
    out.extend((!rest.is_empty()).then_some(Piece::Written(rest)));
    out
}

/// Every name substituted into one piece of text.
pub fn named(text: &str) -> impl Iterator<Item = &str> {
    pieces(text).into_iter().filter_map(Piece::name)
}

/// Every name substituted into the values of a path's query.
pub fn queried(path: &str) -> impl Iterator<Item = &str> {
    path.split_once(QUERY)
        .map(|(_, query)| query)
        .into_iter()
        .flat_map(|query| query.split(PARAMETERS))
        .filter_map(|parameter| parameter.split_once(VALUED).map(|(_, value)| value))
        .flat_map(named)
}

/// Every value one call carries, with where in the call it is put: a query value, the
/// body, or a header's value. Nothing else in a call can carry one.
#[must_use]
pub fn placed(call: &StepCall) -> Vec<(String, &str)> {
    let mut placed: Vec<(String, &str)> = queried(&call.path)
        .map(|name| ("path".to_owned(), name))
        .collect();
    if let Some(body) = &call.body {
        placed.extend(named(body).map(|name| ("body".to_owned(), name)));
    }
    for (header, value) in call.headers.iter().flatten() {
        placed.extend(named(value).map(|name| (format!("headers.{header}"), name)));
    }
    placed
}

/// The address a call to `path` toward this destination is sent to, each query value
/// put through `value` as it is set.
///
/// # Errors
///
/// Where the path is not a plain absolute path, or the address built does not carry
/// exactly the destination's host on exactly its port.
pub fn address(
    path: &str,
    toward: Toward<'_>,
    value: &dyn Fn(&str) -> String,
) -> Result<Url, Unaddressed> {
    if let Some(why) = unplain(path) {
        return Err(Unaddressed::Path(why));
    }
    let (scheme, host, port) = spelled(toward);
    let (segments, query) = path
        .split_once(QUERY)
        .map_or((path, None), |(segments, query)| (segments, Some(query)));
    let built = Url::parse(&format!("{scheme}://{BLANK}/"))
        .ok()
        .and_then(|mut url| {
            url.set_host(Some(host)).ok()?;
            url.set_port(port).ok()?;
            url.set_path(segments);
            url.set_query(query.map(|query| valued(query, value)).as_deref());
            Some(url)
        });
    built
        .filter(|url| lands(url, scheme, host, port))
        .ok_or_else(|| Unaddressed::Host(carried(host)))
}

/// The scheme, host and port an address toward this destination carries; no port is
/// the scheme's own.
const fn spelled(toward: Toward<'_>) -> (&'static str, &str, Option<u16>) {
    match toward {
        Toward::Stack(port) => (PLAIN_SCHEME, HERE, Some(port)),
        Toward::Outside(host) => (SECURE_SCHEME, host, None),
    }
}

/// Whether a written address goes exactly toward this destination, as [`address`]
/// builds one: asked again of what is about to be sent.
#[must_use]
pub fn carries(written: &str, toward: Toward<'_>) -> bool {
    let (scheme, host, port) = spelled(toward);
    Url::parse(written).is_ok_and(|url| lands(&url, scheme, host, port))
}

/// Whether an address carries exactly this scheme, host and port, and nothing in front
/// of the host or after the query.
fn lands(url: &Url, scheme: &str, host: &str, port: Option<u16>) -> bool {
    url.scheme() == scheme
        && url.host_str() == Some(host)
        && url.port() == port
        && url.username().is_empty()
        && url.password().is_none()
        && url.fragment().is_none()
}

/// What an address makes of a host, said for a refusal: why it cannot carry it, or what
/// it carries in its place.
fn carried(host: &str) -> String {
    let mut url = Url::parse(&format!("{SECURE_SCHEME}://{BLANK}/")).ok();
    let set = url.as_mut().map(|url| url.set_host(Some(host)));
    match (set, url.as_ref().and_then(Url::host_str)) {
        (Some(Err(why)), _) => format!("an address cannot carry {host:?}: {why}"),
        (_, read) => format!(
            "an address carries {host:?} as {:?}",
            read.unwrap_or_default()
        ),
    }
}

/// A query with each parameter's value put through `value`; its names are written out.
fn valued(query: &str, value: &dyn Fn(&str) -> String) -> String {
    query
        .split(PARAMETERS)
        .map(|parameter| match parameter.split_once(VALUED) {
            Some((name, written)) => format!("{name}{VALUED}{}", value(written)),
            None => parameter.to_owned(),
        })
        .collect::<Vec<_>>()
        .join(&PARAMETERS.to_string())
}

/// Why a path is not a plain absolute path, or nothing where it is.
#[must_use]
pub fn unplain(path: &str) -> Option<String> {
    if !path.starts_with(ROOT) || path[1..].starts_with(ROOT) {
        return Some("it does not begin with exactly one `/`".to_owned());
    }
    if let Some((_, what)) = NEVER.iter().find(|(held, _)| path.contains(*held)) {
        return Some(format!("it holds {what}"));
    }
    if path
        .chars()
        .any(|one| one.is_whitespace() || one.is_control())
    {
        return Some("it holds whitespace or a control character".to_owned());
    }
    let upper = path.to_ascii_uppercase();
    if let Some((written, what)) = NEVER_ENCODED.iter().find(|(held, _)| upper.contains(held)) {
        return Some(format!("it holds {what} percent-encoded, as {written}"));
    }
    let segments = path.split(QUERY).next().unwrap_or_default();
    if segments
        .split(ROOT)
        .any(|segment| segment == "." || segment == "..")
    {
        return Some("it holds a `.` or `..` segment".to_owned());
    }
    None
}

#[cfg(test)]
mod tests;
