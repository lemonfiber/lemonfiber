//! The proxy in front of the household's services: the names it answers, where each
//! goes, and how it holds TLS.

use async_trait::async_trait;

use super::Failure;

/// One household name the proxy answers, and the service it forwards to.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Route {
    /// The household name, the operator's domain included.
    pub name: String,
    /// The host the service answers on, on the network the proxy shares with it.
    pub host: String,
    /// The port the service listens on there.
    pub port: u16,
}

/// How the proxy holds TLS for every name it answers.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum Tls {
    /// The proxy obtains a certificate for each name itself where an authority issues one.
    Automatic,
    /// The certificate and key in these files, beneath the proxy's configuration directory.
    Files {
        /// The certificate chain.
        certificate: String,
        /// Its private key.
        key: String,
    },
}

/// Every route the proxy serves, and its TLS.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Routing {
    /// How it holds TLS.
    pub tls: Tls,
    /// Every name it answers, in the order applied.
    pub routes: Vec<Route>,
}

/// A proxy, told its routing table and asked to read it.
#[async_trait]
pub trait Proxying: Send + Sync {
    /// Serve `table` in place of the table it serves now.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where it is unreachable or refuses the table.
    async fn apply(&self, table: &Routing) -> Result<(), Failure>;

    /// The table it serves now.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where it is unreachable or answers unreadably.
    async fn routing(&self) -> Result<Routing, Failure>;

    /// Read the table applied, so it answers by it.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where it is unreachable or cannot read the table.
    async fn reload(&self) -> Result<(), Failure>;
}
