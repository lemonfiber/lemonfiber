//! Opening a real NNTP connection to a Usenet provider, for real.
//!
//! The whole of this adapter is transport: connect, wrap in TLS where asked, read
//! the greeting, send each command and read its reply, hand the lines back. No
//! decisions — reading the reply codes into an outcome belongs above the port,
//! where a fake stands in for this. `rustls` and the socket are confined here.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use lemonfiber_core::ports::nntp::{Endpoint, Nntp, Unreachable};
use rustls::ClientConfig;
use rustls_pki_types::ServerName;
use tokio::io::{AsyncBufReadExt as _, AsyncRead, AsyncWrite, AsyncWriteExt as _, BufReader};
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;

/// How long a whole exchange may take before nothing more is waited on — a
/// provider that has not answered a login within this has not answered.
const BUDGET: Duration = Duration::from_secs(15);

/// Dials Usenet providers over NNTP, TLS-wrapped where the endpoint asks.
pub struct Dialer {
    /// The TLS client configuration, or nothing where it could not be built — a
    /// process-wide impossibility with a bundled provider, handled rather than
    /// panicked on so a secure dial reports it instead.
    config: Option<Arc<ClientConfig>>,
}

impl Dialer {
    /// A dialer trusting the bundled Mozilla roots, so a static binary carries its
    /// own trust anchors rather than depending on a system store.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: build_config(),
        }
    }
}

impl Default for Dialer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Nntp for Dialer {
    async fn converse(
        &self,
        endpoint: &Endpoint,
        commands: &[String],
    ) -> Result<Vec<String>, Unreachable> {
        let fail = |reason: String| Unreachable {
            host: endpoint.host.clone(),
            reason,
        };
        // Bounded, so a provider that never answers is reported unreachable rather
        // than waited on forever.
        match tokio::time::timeout(BUDGET, self.dial(endpoint, commands, &fail)).await {
            Ok(result) => result,
            Err(_) => Err(fail(format!("no reply within {}s", BUDGET.as_secs()))),
        }
    }
}

impl Dialer {
    /// Connect, wrap in TLS where asked, and run the exchange.
    async fn dial<F>(
        &self,
        endpoint: &Endpoint,
        commands: &[String],
        fail: &F,
    ) -> Result<Vec<String>, Unreachable>
    where
        F: Fn(String) -> Unreachable,
    {
        let tcp = TcpStream::connect((endpoint.host.as_str(), endpoint.port))
            .await
            .map_err(|error| fail(error.to_string()))?;
        if !endpoint.secure {
            return exchange(tcp, commands, fail).await;
        }
        let Some(config) = &self.config else {
            return Err(fail("the TLS backend could not be initialised".to_owned()));
        };
        let name = ServerName::try_from(endpoint.host.clone())
            .map_err(|_| fail("the provider's hostname is not a valid TLS name".to_owned()))?;
        let stream = TlsConnector::from(config.clone())
            .connect(name, tcp)
            .await
            .map_err(|error| fail(error.to_string()))?;
        exchange(stream, commands, fail).await
    }
}

/// Read the greeting, then send each command and read its reply, returning the
/// greeting followed by each reply in order.
async fn exchange<S, F>(
    stream: S,
    commands: &[String],
    fail: &F,
) -> Result<Vec<String>, Unreachable>
where
    S: AsyncRead + AsyncWrite + Unpin,
    F: Fn(String) -> Unreachable,
{
    let mut connection = BufReader::new(stream);
    let mut replies = Vec::with_capacity(commands.len() + 1);
    replies.push(read_reply(&mut connection, fail).await?);
    for command in commands {
        connection
            .get_mut()
            .write_all(format!("{command}\r\n").as_bytes())
            .await
            .map_err(|error| fail(error.to_string()))?;
        replies.push(read_reply(&mut connection, fail).await?);
    }
    // Close politely; the provider has answered all that was asked.
    let _ = connection.get_mut().write_all(b"QUIT\r\n").await;
    Ok(replies)
}

/// Read one CRLF-terminated reply line, its trailing newline trimmed. A closed
/// connection where a line was expected is nothing usable answering.
async fn read_reply<S, F>(connection: &mut BufReader<S>, fail: &F) -> Result<String, Unreachable>
where
    S: AsyncRead + AsyncWrite + Unpin,
    F: Fn(String) -> Unreachable,
{
    let mut line = String::new();
    let read = connection
        .read_line(&mut line)
        .await
        .map_err(|error| fail(error.to_string()))?;
    if read == 0 {
        return Err(fail("the provider closed the connection".to_owned()));
    }
    Ok(line.trim_end().to_owned())
}

/// Build the TLS client configuration over the bundled roots, or nothing where
/// the provider could not supply the safe defaults — which it always can.
fn build_config() -> Option<Arc<ClientConfig>> {
    let mut roots = rustls::RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let config =
        ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_safe_default_protocol_versions()
            .ok()?
            .with_root_certificates(roots)
            .with_no_client_auth();
    Some(Arc::new(config))
}
