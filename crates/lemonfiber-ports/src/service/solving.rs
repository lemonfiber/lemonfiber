//! A challenge solver: a page fetched on another service's behalf from a site that
//! challenges a plain client, with what cleared the challenge.

use async_trait::async_trait;

use super::Failure;

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
