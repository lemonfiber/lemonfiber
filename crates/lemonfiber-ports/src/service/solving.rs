//! A challenge solver: a page fetched on another service's behalf from a site that
//! challenges a plain client, with what cleared the challenge; and the indexer told which
//! solvers it fetches through.

use async_trait::async_trait;

use super::{Failure, Protocol};

/// A cookie a site set while a page was fetched.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Cookie {
    /// Its name.
    pub name: String,
    /// Its value.
    pub value: String,
    /// The domain it is sent to.
    pub domain: String,
}

/// A page fetched past its challenge.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Solved {
    /// The status the page answered with.
    pub status: u16,
    /// The address the page ended at, after every redirect.
    pub url: String,
    /// The browser it was fetched as.
    pub user_agent: String,
    /// The cookies the site set.
    pub cookies: Vec<Cookie>,
    /// The page.
    pub page: String,
}

/// A challenge solver.
#[async_trait]
pub trait Solving: Send + Sync {
    /// The page at `url`, fetched past whatever challenge stands in front of it.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where it is unreachable or the challenge was not solved.
    async fn solve(&self, url: &str) -> Result<Solved, Failure>;

    /// Whether it answers.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where it is unreachable or cannot tell.
    async fn answers(&self) -> Result<bool, Failure>;
}

/// A challenge solver, as the indexer fetching through it is told about it.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct SolverAt {
    /// What it is called where somebody reads the list.
    pub name: String,
    /// Where it is reached, on the stack's own network.
    pub url: String,
    /// The API it is asked in, so an indexer is never handed a solver it cannot ask.
    pub protocol: Protocol,
    /// The tag that sends an indexer site through it.
    pub tag: String,
}

/// A challenge solver the indexer already fetches through.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct KnownSolver {
    /// The identifier the indexer gave it.
    pub id: String,
    /// Where the indexer says it is.
    pub url: String,
    /// The tags that send an indexer site through it.
    pub tags: Vec<String>,
}

/// An indexer told about the challenge solvers it fetches through.
#[async_trait]
pub trait Solvers: Send + Sync {
    /// The solvers it already fetches through.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when it is unreachable or refuses, and
    /// [`Failure::Unsupported`] where it fetches through none.
    async fn proxies(&self) -> Result<Vec<KnownSolver>, Failure>;

    /// Fetch through one more solver, tested before it is saved.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when it is unreachable or refuses, and
    /// [`Failure::Unsupported`] where it cannot ask a solver in that API.
    async fn add_proxy(&self, solver: &SolverAt) -> Result<(), Failure>;
}
