//! What the pass service answers the stream guard: whether the pass a forwarded request
//! carries is good for exactly the live stream that request's path names.
//!
//! The service is this binary, run as a service of the project with `lemonfiber pass`,
//! holding the key read-only. The guard asks `GET /pass` with the request it forwards in
//! `X-Forwarded-Uri`, and anything but a yes is a refusal.

use lemonfiber_sidecar::pass::PARAMETER;
pub use lemonfiber_sidecar::pass::{File, Key, Live, LivePath, KEY_BYTES};

/// The port the pass service listens on, inside the network it shares with the guard.
pub const PORT: u16 = 8090;

/// The path the guard asks the pass service at.
pub const PATH: &str = "/pass";

/// The header the guard names the request it forwards in.
pub const FORWARDED: &str = "X-Forwarded-Uri";

/// The group a live-stream path names its live stream with.
const STREAM: &str = lemonfiber_ports::service::STREAM_NAME;

/// The media server's live-stream paths, compiled, each with the shape its live stream's
/// id has.
#[derive(Debug)]
pub struct Watching {
    /// Each path's pattern and its id's shape, the first that covers a path deciding it.
    paths: Vec<(regex::Regex, regex::Regex)>,
}

impl Watching {
    /// The live-stream paths `live` holds, compiled.
    ///
    /// # Errors
    ///
    /// The pattern that does not compile, or names no `stream` group, where one does not.
    pub fn of(live: &Live) -> Result<Self, String> {
        let paths = live
            .paths
            .iter()
            .map(|path| {
                let pattern = regex::Regex::new(&path.pattern)
                    .ok()
                    .filter(|compiled| {
                        compiled
                            .capture_names()
                            .flatten()
                            .any(|name| name == STREAM)
                    })
                    .ok_or_else(|| path.pattern.clone())?;
                let shape = regex::Regex::new(&path.stream).map_err(|_| path.stream.clone())?;
                Ok((pattern, shape))
            })
            .collect::<Result<_, String>>()?;
        Ok(Self { paths })
    }

    /// The live stream `path` names: the one the first live-stream path covering it places
    /// in it, where its id has the shape that path declares.
    #[must_use]
    pub fn stream<'a>(&self, path: &'a str) -> Option<&'a str> {
        let (captured, shape) = self
            .paths
            .iter()
            .find_map(|(pattern, shape)| Some((pattern.captures(path)?, shape)))?;
        let stream = captured.name(STREAM)?.as_str();
        shape.is_match(stream).then_some(stream)
    }

    /// Whether the request `forwarded` names has a path the upstream reads only as
    /// written and carries, in its one `lfpass`, a pass `key` confirms for the live stream
    /// that path names at `now`, in seconds since the Unix epoch.
    #[must_use]
    pub fn confirms(&self, key: &Key, forwarded: &str, now: u64) -> bool {
        let (path, query) = forwarded.split_once('?').unwrap_or((forwarded, ""));
        if forwarded.contains('#') || !normal(path) {
            return false;
        }
        let mut carried = query
            .split('&')
            .filter(|pair| pair.split_once('=').map_or(*pair, |(name, _)| name) == PARAMETER);
        let (Some(pair), None) = (carried.next(), carried.next()) else {
            return false;
        };
        let Some((_, pass)) = pair.split_once('=') else {
            return false;
        };
        self.stream(path)
            .is_some_and(|stream| key.confirms(pass, stream, now))
    }
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
