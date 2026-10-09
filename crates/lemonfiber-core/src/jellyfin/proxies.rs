//! Which address Jellyfin believes when a request names the client it came from.
//!
//! Behind the guarded front door every request reaches Jellyfin from the door, so
//! Jellyfin would count every client as being on the household's own network and apply
//! none of the rules it keeps for a client that is not. Trusting the door's address to
//! name the client gives it back the client's own address, and trusting nothing else
//! keeps a client from naming itself.
//!
//! The list lives in the server's network configuration, which is written back whole,
//! read again to be held to it, and read by the server only when it starts.

use crate::ports::http::Method;
use crate::ports::service::Failure;

use super::Jellyfin;

/// Where the server's network configuration is read and written.
const NETWORK: &str = "/System/Configuration/network";

/// The field that holds the addresses trusted to name the client.
const KNOWN_PROXIES: &str = "KnownProxies";

/// Where the server is asked to start again.
const RESTART: &str = "/System/Restart";

impl Jellyfin {
    /// The addresses trusted to name the client now.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where Jellyfin is unreachable, refuses the sign-in, or answers
    /// with a configuration that cannot be read.
    pub async fn known_proxies(&self) -> Result<Vec<String>, Failure> {
        let configuration = self.network().await?;
        Ok(proxies(&configuration))
    }

    /// Trust `address` alone to name the client, and hold the server to having kept it.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where the address is empty, where Jellyfin is unreachable or
    /// refuses the write, or reads back a list other than the one written.
    pub async fn trust_only(&self, address: &str) -> Result<(), Failure> {
        if address.trim().is_empty() {
            return Err(self.endpoint.refused("no address was given to trust"));
        }
        let mut configuration = self.network().await?;
        let Some(fields) = configuration.as_object_mut() else {
            return Err(self
                .endpoint
                .refused("the network configuration is not an object"));
        };
        fields.insert(KNOWN_PROXIES.to_owned(), serde_json::json!([address]));
        let response = self
            .as_admin(Method::Post, NETWORK, Some(configuration.to_string()))
            .await?;
        self.endpoint.expect_success(&response)?;
        let kept = self.known_proxies().await?;
        if kept != [address] {
            return Err(self.endpoint.refused(&format!(
                "the trusted addresses were written as {address} and read back as {kept:?}"
            )));
        }
        Ok(())
    }

    /// Ask the server to start again, which is when it reads its network configuration.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where Jellyfin is unreachable or refuses.
    pub async fn restart(&self) -> Result<(), Failure> {
        let response = self.as_admin(Method::Post, RESTART, None).await?;
        self.endpoint.expect_success(&response)
    }

    /// The server's network configuration, as it answers it.
    async fn network(&self) -> Result<serde_json::Value, Failure> {
        let response = self.as_admin(Method::Get, NETWORK, None).await?;
        self.endpoint
            .decode(&response, "the network configuration could not be read")
    }
}

/// The addresses a network configuration trusts, in the order it names them.
fn proxies(configuration: &serde_json::Value) -> Vec<String> {
    configuration
        .get(KNOWN_PROXIES)
        .and_then(serde_json::Value::as_array)
        .map(|listed| {
            listed
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}
