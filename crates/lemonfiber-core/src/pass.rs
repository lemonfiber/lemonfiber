//! What the pass service answers the stream guard: whether the pass a forwarded request
//! carries is good for exactly the live stream or the item that request names.
//!
//! The service is this binary, run as a service of the project with `lemonfiber pass`,
//! holding the key read-only. The guard asks `GET /pass` with the request it forwards in
//! `X-Forwarded-Uri`, and anything but a yes is a refusal. A yes to an item pass hands
//! the guard the session it seals, in [`SESSION`].

use lemonfiber_sidecar::pass::PARAMETER;
pub use lemonfiber_sidecar::pass::{
    renewed_as, File, InQuery, ItemPath, Key, LivePath, Opened, Paths, Renewals, Renewed, Session,
    FORWARDED, KEY_BYTES, LIFETIME, NONCE_BYTES, SESSION,
};

/// The pass service's id in the project.
pub const SERVICE: &str = lemonfiber_manifest::container::PASS_SERVICE;

/// The request this binary answers as the pass service under.
pub const REQUEST: &str = "pass";

/// The file beneath the project directory the pass service's image is pinned in.
pub const PINNED: &str = "pass.image";

/// Where the pass service reads its files inside its container.
pub const CONFIGURATION: &str = lemonfiber_manifest::container::CONFIGURATION;

/// The memory the pass service is held to, in MiB.
pub const MEMORY_MIB: u32 = 64;

/// The port the pass service listens on, inside the network it shares with the guard.
pub const PORT: u16 = 8090;

/// The path the guard asks the pass service at.
pub const PATH: &str = "/pass";

/// The image the pass service runs for release `version`: the release's own, at
/// `digest`.
#[must_use]
pub fn pin(version: &str, digest: &str) -> String {
    format!("{}:{version}@{digest}", crate::IMAGE)
}

/// The image `held` pins the pass service to, where it pins release `version`'s own by
/// its digest.
#[must_use]
pub fn pinned(held: &str, version: &str) -> Option<String> {
    let held = held.trim();
    held.strip_prefix(&pin(version, ""))
        .filter(|digest| lemonfiber_ports::registry::is_digest(digest))
        .map(|_| held.to_owned())
}

/// What the pass service is started with: answering at `at`.
#[must_use]
pub fn command(at: std::net::SocketAddr) -> Vec<String> {
    vec![REQUEST.to_owned(), "--listen".to_owned(), at.to_string()]
}

/// The paths the pass service is told, as the media server's filler `declared` them.
#[must_use]
pub fn told(declared: &lemonfiber_ports::service::Declared) -> Paths {
    Paths::of(
        declared
            .live
            .iter()
            .map(|path| LivePath {
                pattern: path.pattern.clone(),
                parameter: path.parameter.clone(),
                stream: path.stream.clone(),
            })
            .collect(),
        declared
            .bytes
            .iter()
            .map(|path| ItemPath {
                pattern: path.pattern.clone(),
                query: path.query.as_ref().map(|query| InQuery {
                    parameter: query.parameter.clone(),
                    pattern: query.pattern.clone(),
                }),
                item: path.item.clone(),
            })
            .collect(),
    )
}

/// The group a live-stream path names its live stream with.
const STREAM: &str = lemonfiber_ports::service::STREAM_NAME;

/// The group a byte path names its item with.
const ITEM: &str = "item";

/// Where a compiled path finds what it names.
#[derive(Debug)]
enum Naming {
    /// In the group of the path pattern.
    Path,
    /// In the whole value of this query parameter.
    Parameter(String),
    /// In the group of this pattern, matched whole against this query parameter's value.
    Value(String, regex::Regex),
}

/// One compiled path: the paths it covers, where it finds what it names, and the shape
/// what it names has.
#[derive(Debug)]
struct Compiled {
    pattern: regex::Regex,
    naming: Naming,
    shape: regex::Regex,
}

impl Compiled {
    /// Whether a request on `path` with the query pairs `pairs` is one this path covers:
    /// its pattern matches, and where it finds what it names in a parameter, the request
    /// carries that parameter under any spelling.
    fn covers(&self, path: &str, pairs: &[(&str, Option<&str>)]) -> bool {
        self.pattern.is_match(path)
            && match &self.naming {
                Naming::Path => true,
                Naming::Parameter(parameter) | Naming::Value(parameter, _) => pairs
                    .iter()
                    .any(|(name, _)| name.eq_ignore_ascii_case(parameter)),
            }
    }

    /// What a request on `path` with the query pairs `pairs` names, read from the one
    /// place this path finds it, where it has this path's shape; nothing where the
    /// parameter it is found in is carried other than once under its spelling.
    fn named(&self, group: &str, path: &str, pairs: &[(&str, Option<&str>)]) -> Option<String> {
        let found = match &self.naming {
            Naming::Path => self
                .pattern
                .captures(path)?
                .name(group)?
                .as_str()
                .to_owned(),
            Naming::Parameter(parameter) => once(pairs, parameter)?,
            Naming::Value(parameter, value) => value
                .captures(&once(pairs, parameter)?)?
                .name(group)?
                .as_str()
                .to_owned(),
        };
        self.shape.is_match(&found).then_some(found)
    }
}

/// The value of `parameter` among `pairs`, decoded once and holding no `%` after, where
/// it is carried exactly once under exactly that spelling and under no other.
fn once(pairs: &[(&str, Option<&str>)], parameter: &str) -> Option<String> {
    let mut carried = pairs
        .iter()
        .filter(|(name, _)| name.eq_ignore_ascii_case(parameter));
    let (Some((name, Some(value))), None) = (carried.next(), carried.next()) else {
        return None;
    };
    let decoded = decoded(value)?;
    (*name == parameter && !decoded.contains('%') && !decoded.is_empty()).then_some(decoded)
}

/// `value` percent-decoded once, where every `%` begins an escape and what it decodes to
/// is UTF-8 holding no control character; a `+` is refused rather than read either way.
fn decoded(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while let Some(&byte) = bytes.get(at) {
        match byte {
            b'%' => {
                let hex = std::str::from_utf8(bytes.get(at + 1..at + 3)?).ok()?;
                out.push(u8::from_str_radix(hex, 16).ok()?);
                at += 3;
            }
            b'+' => return None,
            _ => {
                out.push(byte);
                at += 1;
            }
        }
    }
    String::from_utf8(out)
        .ok()
        .filter(|text| !text.chars().any(char::is_control))
}

/// What a pass a forwarded request carries was confirmed as.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Confirmed {
    /// A live-stream pass good for the live stream the request names.
    Live,
    /// An item pass good for the item the request names, sealing this session.
    Item(Session),
}

/// The media server's live-stream paths and byte paths, compiled.
#[derive(Debug)]
pub struct Watching {
    /// Each live-stream path, the first that covers a path deciding it.
    live: Vec<Compiled>,
    /// Each byte path, the first that covers a path deciding it.
    bytes: Vec<Compiled>,
}

impl Watching {
    /// The paths `paths` holds, compiled.
    ///
    /// # Errors
    ///
    /// The pattern that does not compile, or that does not name what its path finds in
    /// its group, where one does not.
    pub fn of(paths: &Paths) -> Result<Self, String> {
        let live = paths
            .live
            .iter()
            .map(|path| {
                let naming = path
                    .parameter
                    .clone()
                    .map_or(Naming::Path, Naming::Parameter);
                compiled(&path.pattern, naming, &path.stream, STREAM)
            })
            .collect::<Result<_, String>>()?;
        let bytes = paths
            .bytes
            .iter()
            .map(|path| {
                let naming = match &path.query {
                    None => Naming::Path,
                    Some(query) => Naming::Value(
                        query.parameter.clone(),
                        grouped(&format!("^(?:{})$", query.pattern), ITEM)?,
                    ),
                };
                compiled(&path.pattern, naming, &path.item, ITEM)
            })
            .collect::<Result<_, String>>()?;
        Ok(Self { live, bytes })
    }

    /// The live stream a request on `path` with the query `query` names: the one the
    /// first live-stream path covering it places in it, where it has that path's shape.
    #[must_use]
    pub fn stream(&self, path: &str, query: &str) -> Option<String> {
        let pairs = pairs(query)?;
        self.live
            .iter()
            .find(|live| live.covers(path, &pairs))?
            .named(STREAM, path, &pairs)
    }

    /// What the pass a request `forwarded` names carries in its one `lfpass` was
    /// confirmed as by `key` at `now`, in seconds since the Unix epoch, as `renewals`
    /// hold it, where its path is one the upstream reads only as written and the pass is
    /// good for exactly what it names.
    #[must_use]
    pub fn confirms(
        &self,
        key: &Key,
        forwarded: &str,
        now: u64,
        renewals: &Renewals,
    ) -> Option<Confirmed> {
        let (path, query) = forwarded.split_once('?').unwrap_or((forwarded, ""));
        if forwarded.contains('#') || !normal(path) {
            return None;
        }
        let pairs = pairs(query)?;
        let mut carried = pairs
            .iter()
            .filter(|(name, _)| name.eq_ignore_ascii_case(PARAMETER));
        let (Some((PARAMETER, Some(pass))), None) = (carried.next(), carried.next()) else {
            return None;
        };
        if let Some(live) = self.live.iter().find(|live| live.covers(path, &pairs)) {
            let stream = live.named(STREAM, path, &pairs)?;
            return key
                .confirms(pass, &stream, now, renewals)
                .then_some(Confirmed::Live);
        }
        let bytes = self
            .bytes
            .iter()
            .find(|bytes| bytes.pattern.is_match(path))?;
        let item = bytes.named(ITEM, path, &pairs)?;
        key.open(pass, &item, now, renewals)
            .filter(|opened| !opened.lapsed)
            .map(|opened| Confirmed::Item(opened.session))
    }
}

/// `pattern`, anchored at the path's start, finding what it names through `naming`, of
/// `shape`, matched whole; a pattern that finds it in the path has to hold `group`.
fn compiled(pattern: &str, naming: Naming, shape: &str, group: &str) -> Result<Compiled, String> {
    let anchored = format!("^(?:{pattern})");
    let pattern = match naming {
        Naming::Path => grouped(&anchored, group)?,
        _ => regex::Regex::new(&anchored).map_err(|_| pattern.to_owned())?,
    };
    let shape = regex::Regex::new(&format!("^(?:{shape})$")).map_err(|_| shape.to_owned())?;
    Ok(Compiled {
        pattern,
        naming,
        shape,
    })
}

/// `pattern` compiled, where it holds a group named `group`.
fn grouped(pattern: &str, group: &str) -> Result<regex::Regex, String> {
    regex::Regex::new(pattern)
        .ok()
        .filter(|compiled| compiled.capture_names().flatten().any(|name| name == group))
        .ok_or_else(|| pattern.to_owned())
}

/// The pairs of `query`, each name with its value where it has one, empty pairs left
/// out; nothing where a name holds anything but ASCII letters, digits, `_`, `-` and `.`,
/// so no name reads as another once the upstream decodes it.
fn pairs(query: &str) -> Option<Vec<(&str, Option<&str>)>> {
    query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| match pair.split_once('=') {
            Some((name, value)) => (name, Some(value)),
            None => (pair, None),
        })
        .map(|(name, value)| plain(name).then_some((name, value)))
        .collect()
}

/// Whether `name` is spelled in ASCII letters, digits, `_`, `-` and `.` alone.
fn plain(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
}

/// Whether `path` is one the upstream reads only as written: starting at `/`, with no
/// percent-encoding, `;` parameter, backslash, doubled slash, control character,
/// whitespace or dot segment.
fn normal(path: &str) -> bool {
    path.starts_with('/')
        && !path.contains("//")
        && !path.chars().any(|character| {
            matches!(character, '%' | ';' | '\\')
                || character.is_control()
                || character.is_whitespace()
        })
        && path
            .split('/')
            .all(|segment| segment != "." && segment != "..")
}

#[cfg(test)]
mod tests;
