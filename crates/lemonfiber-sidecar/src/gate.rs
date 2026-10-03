//! What the core and the request gate hand each other.
//!
//! Three files, in the gate's configuration directory:
//!
//! - [`File::Upstreams`], each route's upstream and the credential the gate presents
//!   to it, written by the core owner-only and mounted read-only.
//! - [`File::Tokens`], the tokens the request service may present on each route, held
//!   only as hashes, written by the core and mounted read-only. A route holds two
//!   while one replaces the other.
//! - [`File::Record`], every call the gate refused and every removal it passed on,
//!   written by the gate and read back by the core.
//!
//! The gate listens on [`PORT`] inside its own networks and publishes nothing.

use serde::{Deserialize, Serialize};

use crate::shape::written;
use crate::{TokenHash, Unreadable};

/// The port the gate listens on, reached only across its own networks and never
/// published.
pub const PORT: u16 = 5057;

/// The shape of every file here this build reads and writes. A file in another
/// shape is refused rather than read as far as it happens to agree.
pub const FORMAT: u32 = 1;

/// The files in the gate's configuration directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum File {
    /// Each route's upstream and the credential presented to it.
    Upstreams,
    /// The tokens the request service may present, as hashes.
    Tokens,
    /// What the gate refused and what removals it passed on.
    Record,
}

impl File {
    /// The file's name in the configuration directory.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Upstreams => "upstreams.json",
            Self::Tokens => "tokens.json",
            Self::Record => "record.json",
        }
    }
}

/// What a route reaches, which decides the calls the gate answers on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// A television \*arr.
    Sonarr,
    /// A film \*arr.
    Radarr,
    /// The media server.
    Jellyfin,
}

/// The credential the gate presents upstream.
///
/// Its `Debug` withholds it, so a value carrying it that is logged is not logged with
/// it.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Credential(String);

impl std::fmt::Debug for Credential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Credential(withheld)")
    }
}

impl Credential {
    /// `value` as a credential.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// The credential itself, for the one header it is sent in.
    #[must_use]
    pub fn reveal(&self) -> &str {
        &self.0
    }
}

/// One route: the path segment it answers under, what it reaches, where, and with what.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Upstream {
    /// The upstream's service id, which is the route's path segment: `/{route}`.
    pub route: String,
    /// What it reaches.
    pub kind: Kind,
    /// Where the gate reaches it, across the stack's network.
    pub address: String,
    /// What the gate presents to it.
    pub credential: Credential,
}

/// Every route the gate answers, as the core writes them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Upstreams {
    /// The shape this file is written in; [`FORMAT`] for this build.
    pub format: u32,
    /// Every route.
    pub upstreams: Vec<Upstream>,
}

impl Upstreams {
    /// The routes in `upstreams`.
    #[must_use]
    pub fn of(upstreams: Vec<Upstream>) -> Self {
        Self {
            format: FORMAT,
            upstreams,
        }
    }

    /// The route answering under `route`, matched as the upstreams match a path:
    /// without regard to case.
    #[must_use]
    pub fn route(&self, route: &str) -> Option<&Upstream> {
        self.upstreams
            .iter()
            .find(|one| one.route.eq_ignore_ascii_case(route))
    }

    /// The routes as they are written to [`File::Upstreams`].
    #[must_use]
    pub fn written(&self) -> String {
        written(self)
    }

    /// The routes [`File::Upstreams`] holds.
    ///
    /// # Errors
    ///
    /// [`Unreadable`] where the text is not a list of routes, or one in another format.
    pub fn read(text: &str) -> Result<Self, Unreadable> {
        let upstreams: Self = crate::shape::read(File::Upstreams.name(), text)?;
        crate::shape::formatted(File::Upstreams.name(), upstreams.format, FORMAT)?;
        Ok(upstreams)
    }
}

/// The tokens one route accepts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Accepted {
    /// The route they are accepted on.
    pub route: String,
    /// Their hashes: one, or two while one replaces the other.
    pub tokens: Vec<TokenHash>,
}

/// The tokens every route accepts, as the core writes them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tokens {
    /// The shape this file is written in; [`FORMAT`] for this build.
    pub format: u32,
    /// Every route's tokens.
    pub routes: Vec<Accepted>,
}

impl Tokens {
    /// The tokens in `routes`.
    #[must_use]
    pub fn of(routes: Vec<Accepted>) -> Self {
        Self {
            format: FORMAT,
            routes,
        }
    }

    /// Whether `token` is one `route` accepts.
    #[must_use]
    pub fn accepts(&self, route: &str, token: &str) -> bool {
        let presented = TokenHash::of(token);
        self.routes
            .iter()
            .filter(|one| one.route.eq_ignore_ascii_case(route))
            .any(|one| one.tokens.contains(&presented))
    }

    /// The tokens as they are written to [`File::Tokens`].
    #[must_use]
    pub fn written(&self) -> String {
        written(self)
    }

    /// The tokens [`File::Tokens`] holds.
    ///
    /// # Errors
    ///
    /// [`Unreadable`] where the text is not a list of tokens, or one in another format.
    pub fn read(text: &str) -> Result<Self, Unreadable> {
        let tokens: Self = crate::shape::read(File::Tokens.name(), text)?;
        crate::shape::formatted(File::Tokens.name(), tokens.format, FORMAT)?;
        Ok(tokens)
    }
}

/// What became of a call the gate recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Outcome {
    /// Refused, and not passed on.
    Refused,
    /// A removal, passed on.
    Removed,
}

/// One call the gate recorded: never a token, a query string or a body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    /// Its place in the record, counting from one and never reused, so a reader can
    /// tell how many fell off between two reads.
    pub seq: u64,
    /// When, in seconds since the Unix epoch.
    pub at: u64,
    /// The route it was made on.
    pub route: String,
    /// Its method, as it was sent.
    pub method: String,
    /// Its path, without a query string.
    pub path: String,
    /// What became of it.
    pub outcome: Outcome,
}

/// How many entries the record keeps before the oldest fall off.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Kept(usize);

impl Kept {
    /// The last thousand: enough to read back what happened between two diagnostic
    /// runs, and a bound on what a caller refused over and over can make the gate
    /// write.
    #[must_use]
    pub const fn standard() -> Self {
        Self(1000)
    }

    /// How many entries that is.
    #[must_use]
    pub const fn entries(self) -> usize {
        self.0
    }
}

/// What the gate refused and what removals it passed on, as it records them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    /// The shape this file is written in; [`FORMAT`] for this build.
    pub format: u32,
    /// The entries kept, oldest first.
    pub entries: Vec<Entry>,
}

impl Default for Record {
    fn default() -> Self {
        Self {
            format: FORMAT,
            entries: Vec::new(),
        }
    }
}

impl Record {
    /// This record with one more entry after the last, keeping no more than `kept`.
    #[must_use]
    pub fn with(
        mut self,
        at: u64,
        route: &str,
        method: &str,
        path: &str,
        outcome: Outcome,
        kept: Kept,
    ) -> Self {
        let seq = self.entries.last().map_or(1, |last| last.seq + 1);
        self.entries.push(Entry {
            seq,
            at,
            route: route.to_owned(),
            method: method.to_owned(),
            path: path.split('?').next().unwrap_or_default().to_owned(),
            outcome,
        });
        let over = self.entries.len().saturating_sub(kept.entries());
        self.entries.drain(..over);
        self
    }

    /// The entries after `seq`, and how many after it fell off before they were read.
    #[must_use]
    pub fn since(&self, seq: u64) -> (Vec<&Entry>, u64) {
        let unread: Vec<&Entry> = self.entries.iter().filter(|one| one.seq > seq).collect();
        let lost = unread
            .first()
            .map_or(0, |first| first.seq.saturating_sub(seq + 1));
        (unread, lost)
    }

    /// The record as it is written to [`File::Record`].
    #[must_use]
    pub fn written(&self) -> String {
        written(self)
    }

    /// The record [`File::Record`] holds.
    ///
    /// # Errors
    ///
    /// [`Unreadable`] where the text is not a record, or one in another format.
    pub fn read(text: &str) -> Result<Self, Unreadable> {
        let record: Self = crate::shape::read(File::Record.name(), text)?;
        crate::shape::formatted(File::Record.name(), record.format, FORMAT)?;
        Ok(record)
    }
}

#[cfg(test)]
mod tests;
