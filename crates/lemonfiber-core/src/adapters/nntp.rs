//! Opening a real NNTP connection to a Usenet provider, for real.
//!
//! The whole of this adapter is transport: connect, wrap in TLS where asked, read
//! the greeting, send each command and read its reply, hand the lines back. No
//! decisions — reading the reply codes into an outcome belongs above the port,
//! where a fake stands in for this. `rustls` and the socket are confined here.

use std::sync::Arc;
use std::time::Duration;

use crate::ports::nntp::{Endpoint, Nntp, Unreachable};
use async_trait::async_trait;
use rustls::ClientConfig;
use rustls_pki_types::ServerName;
use tokio::io::{
    AsyncBufReadExt as _, AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _, BufReader,
};
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;

/// How long a whole exchange may take before nothing more is waited on — a
/// provider that has not answered a login within this has not answered.
const BUDGET: Duration = Duration::from_secs(15);

/// Dials Usenet providers over NNTP, TLS-wrapped where the endpoint asks.
pub struct Dialer {
    /// How long a whole exchange may take before nothing more is waited on.
    budget: Duration,
}

impl Dialer {
    /// A dialer trusting the bundled Mozilla roots, so a static binary carries its
    /// own trust anchors rather than depending on a system store.
    #[must_use]
    pub fn new() -> Self {
        Self { budget: BUDGET }
    }

    /// A dialer that waits no longer than `budget` for a whole exchange.
    ///
    /// The default is generous, because a provider answering slowly is still a
    /// provider; a caller that cannot wait that long says so here.
    #[must_use]
    pub const fn with_budget(budget: Duration) -> Self {
        Self { budget }
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
        // Bounded, so a provider that never answers is reported unreachable rather
        // than waited on forever.
        match tokio::time::timeout(self.budget, self.dial(endpoint, commands)).await {
            Ok(result) => result,
            Err(_) => Err(unreachable(
                &endpoint.host,
                format!("no reply within {}s", self.budget.as_secs()),
            )),
        }
    }
}

impl Dialer {
    /// Connect, wrap in TLS where asked, and run the exchange.
    async fn dial(
        &self,
        endpoint: &Endpoint,
        commands: &[String],
    ) -> Result<Vec<String>, Unreachable> {
        // A name that cannot be verified against is settled before a socket is
        // opened: there is no point connecting somewhere the handshake could never
        // be held with, and a password must never ride an unwrapped connection.
        let name = endpoint
            .secure
            .then(|| {
                ServerName::try_from(endpoint.host.clone()).map_err(|_| {
                    unreachable(
                        &endpoint.host,
                        "the provider's hostname is not a valid TLS name".to_owned(),
                    )
                })
            })
            .transpose()?;
        let tcp = TcpStream::connect((endpoint.host.as_str(), endpoint.port))
            .await
            .map_err(|error| unreachable(&endpoint.host, error.to_string()))?;
        // Wrapped or not, the exchange is the same and happens in one place: a
        // second call site for it would be a line only a live TLS provider could
        // reach, and the difference between the two is the wrapping, not the talking.
        let wire: Box<dyn Wire> = match name {
            Some(name) => Box::new(self.wrap(endpoint, name, tcp).await?),
            None => Box::new(tcp),
        };
        exchange(wire, &endpoint.host, commands).await
    }

    /// Wrap a connected socket in TLS, as a connection carrying a password must be.
    async fn wrap(
        &self,
        endpoint: &Endpoint,
        name: ServerName<'static>,
        tcp: TcpStream,
    ) -> Result<tokio_rustls::client::TlsStream<TcpStream>, Unreachable> {
        // Built per dial rather than held: a configuration that could not be built
        // would be an impossible state to carry around, and the one place that
        // cares is here, where it becomes an ordinary unreachable-provider report.
        let config = tls_config().map_err(|reason| unreachable(&endpoint.host, reason))?;
        TlsConnector::from(config)
            .connect(name, tcp)
            .await
            .map_err(|error| unreachable(&endpoint.host, error.to_string()))
    }
}

/// The provider could not be reached, in the transport's own words.
///
/// A plain function rather than a captured closure: a closure makes every caller
/// generic over its type, and a generic is instantiated once per build of this
/// crate — the copy the tests do not run then counts as uncovered forever.
fn unreachable(host: &str, reason: String) -> Unreachable {
    Unreachable {
        host: host.to_owned(),
        reason,
    }
}

/// Something an NNTP exchange can be held over — a plain socket or a TLS-wrapped
/// one. Named so the two can share one exchange rather than one each.
trait Wire: AsyncRead + AsyncWrite + Unpin + Send {}

impl<T: AsyncRead + AsyncWrite + Unpin + Send> Wire for T {}

/// Read the greeting, then send each command and read its reply, returning the
/// greeting followed by each reply in order.
/// Not generic over the stream, deliberately. A generic here is instantiated once
/// for the binary and again for its tests, over different types; the two are
/// different symbols, so the binary's copy is never executed and counts as
/// uncovered forever. One boxed wire is one symbol both builds share.
async fn exchange(
    stream: Box<dyn Wire>,
    host: &str,
    commands: &[String],
) -> Result<Vec<String>, Unreachable> {
    let mut connection = BufReader::new(stream);
    let mut replies = Vec::with_capacity(commands.len() + 1);
    replies.push(read_reply(&mut connection, host).await?);
    for command in commands {
        connection
            .get_mut()
            .write_all(format!("{command}\r\n").as_bytes())
            .await
            .map_err(|error| unreachable(host, error.to_string()))?;
        replies.push(read_reply(&mut connection, host).await?);
    }
    // Close politely; the provider has answered all that was asked.
    let _ = connection.get_mut().write_all(b"QUIT\r\n").await;
    Ok(replies)
}

/// Read one CRLF-terminated reply line, its trailing newline trimmed. A closed
/// connection where a line was expected is nothing usable answering.
///
/// Bounded in length: a status line is tens of bytes, so a peer streaming a
/// newline-less blob — hostile or broken, and reached before the login is even
/// proven — is cut off rather than allowed to grow the buffer without limit.
async fn read_reply(
    connection: &mut BufReader<Box<dyn Wire>>,
    host: &str,
) -> Result<String, Unreachable> {
    /// Far above any real NNTP status line, far below a memory concern.
    const MAX_LINE: u64 = 8 * 1024;

    let mut line = String::new();
    let read = (&mut *connection)
        .take(MAX_LINE)
        .read_line(&mut line)
        .await
        .map_err(|error| unreachable(host, error.to_string()))?;
    if read == 0 {
        return Err(unreachable(
            host,
            "the provider closed the connection".to_owned(),
        ));
    }
    Ok(line.trim_end().to_owned())
}

/// Build the TLS client configuration over the bundled roots, or say why it could
/// not be — which, with the bundled provider, it always can.
fn tls_config() -> Result<Arc<ClientConfig>, String> {
    let mut roots = rustls::RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let config =
        ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_safe_default_protocol_versions()
            .map_err(|error| format!("the TLS backend could not be initialised: {error}"))?
            .with_root_certificates(roots)
            .with_no_client_auth();
    Ok(Arc::new(config))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{tls_config, unreachable, Dialer};
    use crate::ports::nntp::{Endpoint, Nntp};

    /// The behaviour of this adapter is proven from `tests/nntp.rs`, against real
    /// sockets — it is the outside world, and that is where the outside world is
    /// reached. What is here is what a library test can settle without one, and it
    /// is here at all because this crate is compiled twice: once for its own tests
    /// and once for the integration suite to link, and a copy nothing runs counts
    /// against the coverage gate whichever copy it is.
    #[tokio::test]
    async fn a_dialer_reports_a_provider_it_cannot_reach() {
        // Port zero listens nowhere, so this settles the whole path — construct,
        // dial, fail — without needing anything to answer.
        let nowhere = Endpoint {
            host: "127.0.0.1".to_owned(),
            port: 0,
            secure: false,
        };
        assert!(Dialer::new().converse(&nowhere, &[]).await.is_err());
        assert!(Dialer::default().converse(&nowhere, &[]).await.is_err());
        assert!(Dialer::with_budget(Duration::from_millis(50))
            .converse(&nowhere, &[])
            .await
            .is_err());
    }

    #[tokio::test]
    async fn a_secure_dial_settles_an_unusable_name_before_it_connects() {
        let unnameable = Endpoint {
            host: "not a hostname".to_owned(),
            port: 563,
            secure: true,
        };
        let refused = Dialer::new().converse(&unnameable, &[]).await;
        assert!(refused.is_err_and(|error| error.reason.contains("not a valid TLS name")));
    }

    #[tokio::test]
    async fn a_dialer_holds_an_exchange_with_something_that_answers() {
        // A provider that answers, so the exchange and its reply reading are held
        // here as well as from the integration suite: this crate is compiled twice,
        // and each copy has to be run by the tests that share its build.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.ok();
        let port = listener
            .as_ref()
            .and_then(|listener| listener.local_addr().ok())
            .map_or(0, |addr| addr.port());
        let server = tokio::spawn(async move {
            use tokio::io::AsyncWriteExt as _;
            let (mut socket, _) = listener?.accept().await.ok()?;
            socket.write_all(b"200 welcome\r\n").await.ok()?;
            tokio::task::yield_now().await;
            socket.write_all(b"281 authenticated\r\n").await.ok()?;
            let mut sent = Vec::new();
            let _ = tokio::time::timeout(
                Duration::from_secs(1),
                tokio::io::AsyncReadExt::read_to_end(&mut socket, &mut sent),
            )
            .await;
            Some(())
        });

        let answering = Endpoint {
            host: "127.0.0.1".to_owned(),
            port,
            secure: false,
        };
        let replies = Dialer::new()
            .converse(&answering, &["AUTHINFO USER me".to_owned()])
            .await;
        assert_eq!(
            replies.ok(),
            Some(vec![
                "200 welcome".to_owned(),
                "281 authenticated".to_owned()
            ])
        );
        let _ = tokio::time::timeout(Duration::from_secs(3), server).await;
    }

    #[tokio::test]
    async fn a_secure_dial_against_something_that_is_not_tls_does_not_complete() {
        // Reaches the wrapping, which a plain dial never does.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.ok();
        let port = listener
            .as_ref()
            .and_then(|listener| listener.local_addr().ok())
            .map_or(0, |addr| addr.port());
        let server = tokio::spawn(async move {
            use tokio::io::AsyncWriteExt as _;
            let (mut socket, _) = listener?.accept().await.ok()?;
            let _ = socket.write_all(b"200 welcome\r\n").await;
            Some(())
        });
        let secure = Endpoint {
            host: "127.0.0.1".to_owned(),
            port,
            secure: true,
        };
        assert!(Dialer::new().converse(&secure, &[]).await.is_err());
        let _ = tokio::time::timeout(Duration::from_secs(3), server).await;
    }

    #[test]
    fn the_bundled_roots_build_a_configuration() {
        // The trust anchors a static binary carries rather than reading from a
        // system store, so a release with no system TLS still verifies a provider.
        assert!(tls_config().is_ok());
    }

    #[test]
    fn an_unreachable_provider_carries_the_host_and_the_reason() {
        let reported = unreachable("news.example", "refused".to_owned());
        assert_eq!(reported.host, "news.example");
        assert_eq!(reported.reason, "refused");
    }
}
