//! A server answering over TLS on loopback with a certificate made for one test, and that
//! certificate's SHA-256: what a crate reaching a pinned service tests against.

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;

use axum::Router;
use futures_util::future::OptionFuture;
use futures_util::StreamExt as _;
use hyper_util::rt::TokioIo;
use hyper_util::service::TowerToHyperService;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::SupportedProtocolVersion;
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::TlsAcceptor;

/// Where a fake answers, and the SHA-256 of the certificate it presents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Served {
    /// `https://127.0.0.1:<port>`.
    pub base: super::Https,
    /// The SHA-256 of the certificate it presents, as a pin is written.
    pub pin: String,
}

/// TLS 1.2, and no other version.
const TLS12_ONLY: &[&SupportedProtocolVersion] = &[&rustls::version::TLS12];

/// `app`, answered over TLS on loopback with a certificate made for it, on the runtime
/// the caller is on.
#[must_use]
pub fn tls(app: Router) -> Option<Served> {
    tls_of(app, rustls::DEFAULT_VERSIONS)
}

/// `app`, answered over TLS 1.2 alone.
#[must_use]
pub fn tls12(app: Router) -> Option<Served> {
    tls_of(app, TLS12_ONLY)
}

/// `app`, answered over the TLS `versions` on loopback with a certificate made for it.
fn tls_of(app: Router, versions: &'static [&'static SupportedProtocolVersion]) -> Option<Served> {
    let made = rcgen::generate_simple_self_signed(vec!["127.0.0.1".to_owned()]).ok()?;
    let der = made.cert.der().clone();
    let listener = std::net::TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0))).ok()?;
    listener.set_nonblocking(true).ok()?;
    let at = listener.local_addr().ok()?;
    let listener = TcpListener::from_std(listener).ok()?;
    let pin = super::sha256(&der);
    let acceptor = acceptor(der, made.signing_key.serialize_der(), versions);
    tokio::spawn(
        futures_util::stream::unfold(listener, |listener| async move {
            let (tcp, _) = listener.accept().await.ok()?;
            Some((tcp, listener))
        })
        .for_each_concurrent(None, move |tcp| {
            answering(tcp, acceptor.clone(), app.clone())
        }),
    );
    Some(Served {
        base: super::Https::parse(&format!("https://{at}"))?,
        pin,
    })
}

/// What accepts a connection over TLS presenting `der`, signing with `key`, a PKCS#8 DER
/// encoding, and speaking the TLS `versions`; nothing where its TLS cannot be set up.
fn acceptor(
    der: CertificateDer<'static>,
    key: Vec<u8>,
    versions: &'static [&'static SupportedProtocolVersion],
) -> Option<TlsAcceptor> {
    let key = PrivateKeyDer::try_from(key).ok()?;
    let config = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_protocol_versions(versions)
    .ok()?
    .with_no_client_auth()
    .with_single_cert(vec![der], key)
    .ok()?;
    Some(TlsAcceptor::from(Arc::new(config)))
}

/// Answer one connection over TLS, where its handshake completes.
async fn answering(tcp: TcpStream, acceptor: Option<TlsAcceptor>, app: Router) {
    let accepted = OptionFuture::from(acceptor.map(|acceptor| acceptor.accept(tcp))).await;
    if let Some(Ok(tls)) = accepted {
        let _ = hyper::server::conn::http1::Builder::new()
            .serve_connection(TokioIo::new(tls), TowerToHyperService::new(app))
            .await;
    }
}
