//! The TLS every connection this crate makes to a service outside it is wrapped in.
//!
//! rustls over ring, trusting the roots bundled into the build, so a static Linux build
//! carries no system TLS library and reads no certificate store of the machine it runs on.

use std::sync::Arc;

use rustls::{ClientConfig, RootCertStore};

/// A client configuration over ring and the bundled roots, or nothing where it could not
/// be built, which with the bundled provider it always can.
pub(crate) fn bundled() -> Option<ClientConfig> {
    let roots: RootCertStore = webpki_roots::TLS_SERVER_ROOTS.iter().cloned().collect();
    Some(
        ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_safe_default_protocol_versions()
            .ok()?
            .with_root_certificates(roots)
            .with_no_client_auth(),
    )
}

#[cfg(test)]
mod tests;
