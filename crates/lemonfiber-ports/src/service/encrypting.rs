//! A media server, told the certificate the internal TLS port it serves presents.

use async_trait::async_trait;

use super::{Applied, Failure};

/// A media server, serving its internal TLS port with a certificate the core makes.
#[async_trait]
pub trait Encrypting: Send + Sync {
    /// Serve the internal TLS port its declaration names with the PKCS#12 at `path`
    /// inside its upstream's container, written with an empty password.
    ///
    /// # Errors
    ///
    /// Returns [`Failure::Unsupported`] where its upstream serves no internal TLS port,
    /// and [`Failure`] where it is unreachable or refuses.
    async fn serve_tls(&self, path: &str) -> Result<Applied, Failure>;
}
