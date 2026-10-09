//! A media server standing behind the stack's front door, and the keys it hands the
//! stack's own services.
//!
//! Behind the door every request reaches the server from the door, so the server is told
//! which address alone may name the client, and which one origin a browser may read it
//! from. It reads both only when it starts, so it is asked to start again after either is
//! written. Each write is read back and held to: a write the server accepted and did not
//! keep is the failure this exists to catch.
//!
//! A key filed under an app's name is a credential one of the stack's services holds to
//! reach the server, filed so an operator reading the server's own key list can tell
//! whose each one is.

use async_trait::async_trait;

use super::Failure;

/// When a key was made and last used, as the server writes each moment.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Dated {
    /// When it was made.
    pub created: Option<String>,
    /// When it was last used, absent where the server says it never was.
    pub last_used: Option<String>,
}

/// Which address names the client, which origin a browser may read the server from, and
/// starting it again so it reads them.
#[async_trait]
pub trait Fronted: Send + Sync {
    /// The addresses trusted to name the client now.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where the server is unreachable or answers unreadably.
    async fn known_proxies(&self) -> Result<Vec<String>, Failure>;

    /// Trust `address` alone to name the client, and hold the server to having kept it.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where the address is empty, where the server is unreachable or
    /// refuses the write, or reads back a list other than the one written.
    async fn trust_only(&self, address: &str) -> Result<(), Failure>;

    /// The origins a browser may read the server from now.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where the server is unreachable or answers unreadably.
    async fn allowed_origins(&self) -> Result<Vec<String>, Failure>;

    /// Allow `origin` alone, and hold the server to having kept it.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where the origin is empty or a wildcard, where the server is
    /// unreachable or refuses the write, or reads back a list other than the one written.
    async fn allow_only(&self, origin: &str) -> Result<(), Failure>;

    /// Ask the server to start again, which is when it reads what was written.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where the server is unreachable or refuses.
    async fn restart(&self) -> Result<(), Failure>;
}

/// The keys the server holds for the stack's services, each filed under an app's name.
#[async_trait]
pub trait AppKeys: Send + Sync {
    /// Every key filed under `app`, oldest first.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where the server is unreachable or answers unreadably.
    async fn filed_as(&self, app: &str) -> Result<Vec<String>, Failure>;

    /// When each key filed under `app` was made and last used, and none where no key is
    /// filed under it. Every one is answered: a second key filed under the same name would
    /// otherwise hide the uses of the first.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where the server is unreachable or answers unreadably.
    async fn dated(&self, app: &str) -> Result<Vec<Dated>, Failure>;

    /// Mint a key filed under `app`, and answer it. Anything but exactly one new key under
    /// `app` is refused rather than guessed at: a key handed over that is not the one just
    /// made is a credential somebody else may also hold.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where the server is unreachable, refuses the mint, or answers
    /// other than one new key.
    async fn mint(&self, app: &str) -> Result<String, Failure>;

    /// Whether the server answers to `key` alone, with no session.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where the server is unreachable or refuses the key.
    async fn answers_to(&self, key: &str) -> Result<(), Failure>;

    /// Revoke the key `key`.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where the server is unreachable or refuses the revocation.
    async fn revoke(&self, key: &str) -> Result<(), Failure>;
}
