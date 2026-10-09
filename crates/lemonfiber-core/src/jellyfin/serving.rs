//! The server behind the front door, and the keys it holds for the stack's services, as
//! the ports carry them.
//!
//! Each operation is the one its own file implements, so what Jellyfin is asked and how
//! it answers stays beside the configuration field or endpoint it concerns.

use async_trait::async_trait;

use super::{cors, keys, proxies, Jellyfin};
use crate::ports::service::{AppKeys, Dated, Failure, Fronted};

#[async_trait]
impl Fronted for Jellyfin {
    async fn known_proxies(&self) -> Result<Vec<String>, Failure> {
        proxies::known_proxies(self).await
    }

    async fn trust_only(&self, address: &str) -> Result<(), Failure> {
        proxies::trust_only(self, address).await
    }

    async fn allowed_origins(&self) -> Result<Vec<String>, Failure> {
        cors::allowed_origins(self).await
    }

    async fn allow_only(&self, origin: &str) -> Result<(), Failure> {
        cors::allow_only(self, origin).await
    }

    async fn restart(&self) -> Result<(), Failure> {
        proxies::restart(self).await
    }
}

#[async_trait]
impl AppKeys for Jellyfin {
    async fn filed_as(&self, app: &str) -> Result<Vec<String>, Failure> {
        keys::filed_as(self, app).await
    }

    async fn dated(&self, app: &str) -> Result<Vec<Dated>, Failure> {
        keys::dated(self, app).await
    }

    async fn mint(&self, app: &str) -> Result<String, Failure> {
        keys::mint(self, app).await
    }

    async fn answers_to(&self, key: &str) -> Result<(), Failure> {
        keys::answers_to(self, key).await
    }

    async fn revoke(&self, key: &str) -> Result<(), Failure> {
        keys::revoke(self, key).await
    }
}
