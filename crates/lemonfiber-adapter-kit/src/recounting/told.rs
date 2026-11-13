//! Where a recounter sends what happened: the core's address, reached only where it
//! presents the certificate pinned, with the key the core minted for this adapter.

use lemonfiber_pinned::Https;
use lemonfiber_ports::service::Event;
use reqwest::header::{HeaderValue, CONTENT_TYPE};

use crate::credential::spelled_as_a_key;
use crate::web::REQUEST_TIMEOUT;

/// The core's address, the client pinned to its certificate, and the key presented.
pub(crate) struct Told {
    /// The address events are posted to.
    address: Https,
    /// A client reaching only a server presenting the pinned certificate.
    client: reqwest::Client,
    /// The key presented as the bearer.
    key: String,
}

impl Told {
    /// Sending to `address`, pinning `pin`, with `key`: nothing where the address is not
    /// https, the pin is not a SHA-256 in lower-case hexadecimal or the key is not spelled
    /// as a key is.
    pub(crate) fn new(address: &str, pin: &str, key: &str) -> Option<Self> {
        if !lemonfiber_pinned::is_pin(pin) || !spelled_as_a_key(key) {
            return None;
        }
        Some(Self {
            address: Https::parse(address)?,
            client: lemonfiber_pinned::client(&[pin.to_owned()], Some(REQUEST_TIMEOUT))?,
            key: key.to_owned(),
        })
    }

    /// Whether the core answered `event` with a success.
    pub(crate) async fn sent(&self, event: &Event) -> bool {
        self.client
            .post(self.address.as_url().clone())
            .bearer_auth(&self.key)
            .header(
                CONTENT_TYPE,
                HeaderValue::from_static(lemonfiber_contract::wire::JSON),
            )
            .json(event)
            .send()
            .await
            .is_ok_and(|answer| answer.status().is_success())
    }
}

#[cfg(test)]
pub(crate) mod tests;
