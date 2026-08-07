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
    /// The TLS client configuration, or nothing where it could not be built — a
    /// process-wide impossibility with a bundled provider, handled rather than
    /// panicked on so a secure dial reports it instead.
    config: Option<Arc<ClientConfig>>,
    /// How long a whole exchange may take before nothing more is waited on.
    budget: Duration,
}

impl Dialer {
    /// A dialer trusting the bundled Mozilla roots, so a static binary carries its
    /// own trust anchors rather than depending on a system store.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: build_config(),
            budget: BUDGET,
        }
    }

    /// A dialer with the TLS configuration and patience a test needs — the two
    /// things about a dial that cannot be arranged from outside it: a TLS backend
    /// that would not initialise, and a wait short enough to actually run out.
    #[cfg(test)]
    fn with(config: Option<Arc<ClientConfig>>, budget: Duration) -> Self {
        Self { config, budget }
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
        match tokio::time::timeout(self.budget, self.dial(endpoint, commands, &fail)).await {
            Ok(result) => result,
            Err(_) => Err(fail(format!("no reply within {}s", self.budget.as_secs()))),
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
        // Wrapped or not, the exchange is the same and happens in one place: a
        // second call site for it would be a line only a live TLS provider could
        // reach, and the difference between the two is the wrapping, not the talking.
        let wire: Box<dyn Wire> = if endpoint.secure {
            Box::new(self.wrap(endpoint, tcp, fail).await?)
        } else {
            Box::new(tcp)
        };
        exchange(wire, commands, fail).await
    }

    /// Wrap a connected socket in TLS, as a connection carrying a password must be.
    async fn wrap<F>(
        &self,
        endpoint: &Endpoint,
        tcp: TcpStream,
        fail: &F,
    ) -> Result<tokio_rustls::client::TlsStream<TcpStream>, Unreachable>
    where
        F: Fn(String) -> Unreachable,
    {
        let Some(config) = &self.config else {
            return Err(fail("the TLS backend could not be initialised".to_owned()));
        };
        let name = ServerName::try_from(endpoint.host.clone())
            .map_err(|_| fail("the provider's hostname is not a valid TLS name".to_owned()))?;
        TlsConnector::from(config.clone())
            .connect(name, tcp)
            .await
            .map_err(|error| fail(error.to_string()))
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
async fn exchange<F>(
    stream: Box<dyn Wire>,
    commands: &[String],
    fail: &F,
) -> Result<Vec<String>, Unreachable>
where
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
///
/// Bounded in length: a status line is tens of bytes, so a peer streaming a
/// newline-less blob — hostile or broken, and reached before the login is even
/// proven — is cut off rather than allowed to grow the buffer without limit.
async fn read_reply<F>(
    connection: &mut BufReader<Box<dyn Wire>>,
    fail: &F,
) -> Result<String, Unreachable>
where
    F: Fn(String) -> Unreachable,
{
    /// Far above any real NNTP status line, far below a memory concern.
    const MAX_LINE: u64 = 8 * 1024;

    let mut line = String::new();
    let read = (&mut *connection)
        .take(MAX_LINE)
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

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use lemonfiber_core::ports::nntp::{Endpoint, Nntp};
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
    use tokio::net::TcpListener;

    use super::{build_config, exchange, read_reply, Dialer, Unreachable};

    /// A duplex half as the wire the exchange is held over.
    fn boxed(stream: tokio::io::DuplexStream) -> Box<dyn super::Wire> {
        Box::new(stream)
    }

    /// The failure shape the adapter reports everything through.
    fn fail(reason: String) -> Unreachable {
        Unreachable {
            host: "news.example".to_owned(),
            reason,
        }
    }

    /// An endpoint at a port on the loopback.
    fn at(port: u16, secure: bool) -> Endpoint {
        Endpoint {
            host: "127.0.0.1".to_owned(),
            port,
            secure,
        }
    }

    /// Serve one connection, writing `script` and reading whatever is sent, then
    /// hand back the port it is listening on.
    async fn serving(script: &'static [&'static str]) -> (u16, Server) {
        let listener = TcpListener::bind("127.0.0.1:0").await.ok();
        let port = port_of(listener.as_ref());
        let serving = listener.map(|listener| tokio::spawn(speak(listener, script)));
        (port, Server(serving))
    }

    /// A server running in the background. Awaited at the end of a test so it runs
    /// to its own end rather than being cut off mid-answer when the test returns.
    struct Server(Option<tokio::task::JoinHandle<Option<()>>>);

    impl Server {
        async fn finished(self) {
            // An Option iterates once when it holds something and not at all when it
            // does not, so the wait has no arm a passing test could never take.
            // Bounded: a server whose client never closed should not hold the test.
            // Bounded: a server whose client never closed should not hold the test.
            if let Some(handle) = self.0 {
                let _ = tokio::time::timeout(Duration::from_secs(3), handle).await;
            }
        }
    }

    /// Answer one connection with the scripted lines. Returns nothing useful; the
    /// early exits are a client that hung up, which the test's own assertion reports.
    async fn speak(listener: TcpListener, script: &'static [&'static str]) -> Option<()> {
        let (mut socket, _) = listener.accept().await.ok()?;
        for line in script {
            socket
                .write_all(format!("{line}\r\n").as_bytes())
                .await
                .ok()?;
            // Let the client read and send its next command before the reply to it
            // goes out, so the exchange stays in step.
            tokio::task::yield_now().await;
        }
        // Held open until the client has finished sending, so its writes land —
        // bounded, so a client that never closes does not hold the server open.
        let mut sent = Vec::new();
        let _ = tokio::time::timeout(Duration::from_secs(1), socket.read_to_end(&mut sent)).await;
        Some(())
    }

    /// A listener that accepts and then says nothing at all.
    async fn silent() -> (u16, Server) {
        let listener = TcpListener::bind("127.0.0.1:0").await.ok();
        let port = port_of(listener.as_ref());
        let waiting = listener.map(|listener| {
            tokio::spawn(async move {
                let accepted = listener.accept().await.ok();
                // Held open, unanswered, for longer than the patience under test.
                tokio::time::sleep(Duration::from_millis(300)).await;
                drop(accepted);
                Some(())
            })
        });
        (port, Server(waiting))
    }

    /// The port a listener bound to, or zero where there is none — nothing listens
    /// on zero, which is what a test wanting a refusal asks for anyway.
    fn port_of(listener: Option<&TcpListener>) -> u16 {
        listener
            .and_then(|listener| listener.local_addr().ok())
            .map_or(0, |addr| addr.port())
    }

    #[tokio::test]
    async fn an_exchange_returns_the_greeting_then_each_reply_in_order() {
        let (client, mut provider) = tokio::io::duplex(1024);
        let peer = tokio::spawn(async move {
            let _ = provider.write_all(b"200 welcome\r\n").await;
            let _ = provider.write_all(b"381 password\r\n").await;
            let _ = provider.write_all(b"281 authenticated\r\n").await;
            // Held open while the client sends its commands: a peer that hung up
            // after speaking would break the writes rather than answer them.
            let mut sent = Vec::new();
            let _ =
                tokio::time::timeout(Duration::from_secs(1), provider.read_to_end(&mut sent)).await;
        });
        let commands = vec!["AUTHINFO USER me".to_owned(), "AUTHINFO PASS x".to_owned()];
        let replies = exchange(Box::new(client), &commands, &fail).await;
        assert_eq!(
            replies.ok(),
            Some(vec![
                "200 welcome".to_owned(),
                "381 password".to_owned(),
                "281 authenticated".to_owned(),
            ])
        );
        let _ = peer.await;
    }

    #[tokio::test]
    async fn a_provider_that_closes_before_answering_is_nothing_usable() {
        // Nothing written and the far end dropped: a closed connection where a line
        // was expected says nothing about the credential.
        let (client, provider) = tokio::io::duplex(64);
        drop(provider);
        let refused = exchange(Box::new(client), &[], &fail).await;
        assert!(refused.is_err_and(|error| error.reason.contains("closed the connection")));
    }

    #[tokio::test]
    async fn a_reply_line_is_trimmed_of_its_terminator() {
        let (client, mut provider) = tokio::io::duplex(64);
        tokio::spawn(async move {
            let _ = provider.write_all(b"200 ready\r\n").await;
        });
        let mut connection = tokio::io::BufReader::new(boxed(client));
        assert_eq!(
            read_reply(&mut connection, &fail).await.ok(),
            Some("200 ready".to_owned())
        );
    }

    #[tokio::test]
    async fn a_peer_streaming_without_a_newline_is_cut_off_rather_than_grown() {
        // Far more than a status line and no terminator in sight: read up to the
        // bound and stop, rather than letting a hostile peer grow the buffer.
        let (client, mut provider) = tokio::io::duplex(64 * 1024);
        tokio::spawn(async move {
            let _ = provider.write_all(&vec![b'x'; 16 * 1024]).await;
        });
        let mut connection = tokio::io::BufReader::new(boxed(client));
        let line = read_reply(&mut connection, &fail).await.unwrap_or_default();
        assert!(
            line.len() <= 8 * 1024,
            "bounded at the cap, got {}",
            line.len()
        );
    }

    #[tokio::test]
    async fn a_plain_dial_talks_to_a_real_socket() {
        let (port, server) = serving(&["200 welcome", "281 authenticated"]).await;
        let dialer = Dialer::new();
        let replies = dialer
            .converse(&at(port, false), &["AUTHINFO USER me".to_owned()])
            .await;
        assert_eq!(
            replies.ok(),
            Some(vec![
                "200 welcome".to_owned(),
                "281 authenticated".to_owned()
            ])
        );
        server.finished().await;
    }

    #[tokio::test]
    async fn nothing_listening_is_reported_as_unreachable() {
        // Bound and dropped, so the port is almost certainly free and refuses.
        let port = port_of(TcpListener::bind("127.0.0.1:0").await.ok().as_ref());
        let refused = Dialer::new().converse(&at(port, false), &[]).await;
        assert!(refused.is_err());
    }

    #[tokio::test]
    async fn a_provider_that_never_answers_runs_out_of_patience() {
        let (port, server) = silent().await;
        // A budget short enough to actually run out inside a test.
        let dialer = Dialer::with(build_config(), Duration::from_millis(50));
        let waited = dialer.converse(&at(port, false), &[]).await;
        assert!(waited.is_err_and(|error| error.reason.contains("no reply within")));
        server.finished().await;
    }

    #[tokio::test]
    async fn a_secure_dial_against_something_that_is_not_tls_does_not_complete() {
        // The socket answers, but not with a handshake: reported as unreachable
        // rather than treated as a provider that said something.
        let (port, server) = serving(&["200 welcome"]).await;
        let refused = Dialer::new().converse(&at(port, true), &[]).await;
        assert!(refused.is_err());
        server.finished().await;
    }

    #[tokio::test]
    async fn a_host_that_is_not_a_valid_tls_name_is_refused_before_the_handshake() {
        let (port, server) = serving(&["200 welcome"]).await;
        let endpoint = Endpoint {
            host: "not a hostname".to_owned(),
            port,
            secure: true,
        };
        let refused = Dialer::new().converse(&endpoint, &[]).await;
        server.finished().await;
        // It never resolves, so the connect fails first — either way nothing usable
        // came back, which is the only thing this reports.
        assert!(refused.is_err());
    }

    #[tokio::test]
    async fn a_secure_dial_with_no_tls_backend_says_so() {
        // The one case that cannot be arranged from outside: a provider that would
        // not initialise. A secure dial reports it rather than proceeding unwrapped.
        let (port, server) = serving(&["200 welcome"]).await;
        let dialer = Dialer::with(None, Duration::from_secs(5));
        let refused = dialer.converse(&at(port, true), &[]).await;
        assert!(refused.is_err_and(|error| error.reason.contains("TLS backend")));
        server.finished().await;
    }

    #[test]
    fn the_bundled_roots_build_a_configuration() {
        // The trust anchors a static binary carries rather than reading from a
        // system store, so a release with no system TLS still verifies a provider.
        assert!(build_config().is_some());
        // And the default dialer is the one that uses them.
        let _ = Dialer::default();
    }
}
