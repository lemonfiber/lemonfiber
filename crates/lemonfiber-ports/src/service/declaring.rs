//! What a media server declares about the bytes it serves: where they are, what a guard
//! in front of it asks before it passes a request for them, what it refuses outright,
//! how a member's token reaches it, and where each item is found.
//!
//! The guard enforces exactly this declaration, so a media server's own refusals travel
//! with it rather than with whichever guard fronts it.
//!
//! Every pattern here is a regular expression in RE2's syntax, matched against the path
//! alone, never a host, once the guard has held the request to its normal form: one
//! canonical path, no percent-encoding of an unreserved character or of `%`, none at all
//! on a byte path, no byte the forward would re-escape, and one spelling of each
//! parameter it reads. A path pattern is anchored at the path's start, and a shape at
//! both ends of the value it is matched against.

use async_trait::async_trait;

use super::Failure;
use crate::http::Method;

mod checked;

pub use checked::Undeclarable;

/// What a location template's host is.
pub const BASE: &str = "{base}";

/// What a template names the item by.
pub const ITEM: &str = "{item}";

/// What a header template names the member's token by.
pub const TOKEN: &str = "{token}";

/// How a byte path's pattern opens the group the item's id sits in.
pub const ITEM_GROUP: &str = "(?P<item>";

/// A path the upstream serves an item's bytes under.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct BytePath {
    /// The paths it covers, holding one group named `item` where the item's id sits.
    pub pattern: String,
    /// The shape an item's id has, matched against what the group holds; a request whose
    /// item has another shape is refused without asking anything.
    pub item: String,
    /// Paths the pattern covers that name no item, passed on as any other path is.
    pub exempt: Vec<String>,
    /// The query parameters under which the upstream serves another item than the one
    /// in the path, each matched without regard to case, and a parameter on the path
    /// whose name holds anything but ASCII letters, digits, `_`, `-` and `.` refused.
    pub naming: Vec<Naming>,
}

/// A query parameter naming another item than the one in the path.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Naming {
    /// The parameter's name.
    pub parameter: String,
    /// What the guard does with a request carrying it.
    pub held: Held,
}

/// What the guard does with a request carrying a parameter that names another item.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum Held {
    /// Refuses it without asking anything.
    Refused,
    /// Asks the question about the item it names as well as the item in the path, and
    /// refuses it given more than once, under another spelling, or in another shape.
    Asked {
        /// The shape the item it names has.
        shape: String,
    },
}

/// Where the question is sent.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum Addressee {
    /// The upstream itself, carrying the member's token as presented to it.
    Upstream,
    /// The adapter that fills the capability for it.
    Adapter,
}

/// The statuses from `from` to `to`, both included.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Statuses {
    /// The first.
    pub from: u16,
    /// The last.
    pub to: u16,
}

impl Statuses {
    /// The one status `status`.
    #[must_use]
    pub const fn only(status: u16) -> Self {
        Self {
            from: status,
            to: status,
        }
    }
}

/// What each answer to the question means. An answer in none of them is a refusal.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Answers {
    /// The member may have the item's bytes.
    pub may: Vec<Statuses>,
    /// The member may not see the item.
    pub may_not: Vec<Statuses>,
    /// The token is absent, ended or wrong.
    pub no_session: Vec<Statuses>,
    /// The member may not be served from where the request came from.
    pub not_from_there: Vec<Statuses>,
}

/// The request the guard makes to decide a request for an item's bytes.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Question {
    /// Its method.
    pub method: Method,
    /// Its path, naming the item as `{item}`.
    pub path: String,
    /// Where it is sent.
    pub to: Addressee,
    /// What each answer means.
    pub answers: Answers,
}

/// How a member's `Authorization: Bearer <token>` is presented to the upstream.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum Presentation {
    /// As it came.
    AsIs,
    /// In place of the `Authorization` header, as one header.
    Header {
        /// The header's name.
        name: String,
        /// Its value, naming the token as `{token}`.
        value: String,
    },
    /// Taken out of the header and put in the query.
    Query {
        /// The parameter's name.
        parameter: String,
    },
}

/// A member's token, as the upstream reads it.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Token {
    /// How a bearer token is presented.
    pub presented: Presentation,
    /// The shape a token has; a bearer value of any other shape is left as it came.
    pub shape: String,
    /// The query parameters the upstream also reads a token from, carried into the
    /// question as it came; a request carrying more than one of them, one under another
    /// spelling, or one in another shape is refused.
    pub parameters: Vec<String>,
}

/// Where an item is found, each a template naming the household address as `{base}` and
/// the item as `{item}`.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Locations {
    /// Where it streams from.
    pub stream: String,
    /// Its poster.
    pub poster: String,
    /// Its backdrop.
    pub backdrop: String,
}

/// The ports the upstream listens on.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Ports {
    /// Its own.
    pub upstream: u16,
    /// Its own encrypted one, where a household client expects one.
    pub encrypted: Option<u16>,
}

/// Everything a guard in front of a media server enforces, as the media server's filler
/// declares it.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Declared {
    /// Where it serves an item's bytes, the first that covers a path deciding it.
    pub bytes: Vec<BytePath>,
    /// Paths refused without asking anything, because no question about an item covers
    /// them.
    pub refused: Vec<String>,
    /// What the guard asks before it passes a request for an item's bytes.
    pub question: Option<Question>,
    /// How a member's token reaches the upstream.
    pub token: Token,
    /// Where an item is found.
    pub locations: Locations,
    /// The ports it listens on.
    pub ports: Ports,
}

/// A media server, asked what a guard in front of it enforces.
#[async_trait]
pub trait Declaring: Send + Sync {
    /// What a guard in front of it enforces, and where it serves each item.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where it is unreachable or answers unreadably.
    async fn declared(&self) -> Result<Declared, Failure>;
}

#[cfg(test)]
mod tests;
