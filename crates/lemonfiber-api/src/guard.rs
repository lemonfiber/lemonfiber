//! What the web surface refuses, and why it can.
//!
//! A writable API on loopback is reachable from any page the operator happens to
//! visit. Such a page cannot *read* a cross-origin response, but it can *send* a
//! request the server acts on, and a naive origin check is defeated by DNS
//! rebinding. The command line never had this exposure: nothing a web page does
//! reaches `argv`.
//!
//! Two things stand in the way, and both must hold. A secret the caller carries,
//! which a cross-site request cannot read and therefore cannot send. And a check
//! that the request says it came from where this server is listening, which
//! closes the window a rebound name would open.

use std::net::SocketAddr;

use lemonfiber_core::ports::random::Random;

/// The header the per-run token travels in.
///
/// Never a query parameter. URLs reach logs, browser history and referrers, and
/// a credential that reaches any of those has left.
pub const TOKEN_HEADER: &str = "X-Lemonfiber-Token";

/// Bytes of secret. Wide enough that guessing is not a strategy.
const WIDTH: usize = 32;

/// The names a loopback address answers to.
///
/// A printed address may say `localhost`, and it is what an operator types.
/// Refusing the word outright is the wrong trade; refusing a name that resolves
/// somewhere else is the protection that matters, and the client does that
/// before it connects.
const LOOPBACK: [&str; 3] = ["127.0.0.1", "localhost", "[::1]"];

/// A secret minted once per run, held in memory, never written down.
///
/// It is printed when the server starts and given to a client by whoever read
/// it. There is no discovery, no file, and no default.
pub struct Token(String);

impl Token {
    /// Mints one, or nothing when the operating system will not say.
    ///
    /// Randomness arrives through the port rather than being taken directly, so
    /// a test can hand over bytes it chose and this can be exercised without
    /// depending on what the machine happens to produce.
    pub fn mint(random: &dyn Random) -> Option<Self> {
        let bytes = random.bytes(WIDTH)?;
        let mut held = String::with_capacity(WIDTH * 2);
        for byte in bytes {
            // Two hex digits per byte, so the printed token is one word an
            // operator can copy without it wrapping or needing quoting.
            held.push(digit(byte >> 4));
            held.push(digit(byte & 0x0f));
        }
        Some(Self(held))
    }

    /// The token as it is printed and as a caller must send it back.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether the request carried this token.
    ///
    /// The comparison looks at every byte whatever it finds, so how long it
    /// takes says nothing about how much of a guess was right.
    #[must_use]
    pub fn carried_by(&self, offered: Option<&str>) -> bool {
        let Some(offered) = offered else {
            return false;
        };
        let (mine, theirs) = (self.0.as_bytes(), offered.as_bytes());
        mine.len() == theirs.len()
            && mine
                .iter()
                .zip(theirs)
                .fold(0u8, |seen, (a, b)| seen | (a ^ b))
                == 0
    }
}

/// One hex digit, low nibble.
const fn digit(nibble: u8) -> char {
    match nibble {
        0..=9 => (b'0' + nibble) as char,
        _ => (b'a' + nibble - 10) as char,
    }
}

/// Whether a request's `Host` names the address this server is listening on.
///
/// A request without one is refused: `Host` is not optional in the version of
/// HTTP a browser speaks, and its absence is not something to be lenient about.
#[must_use]
pub fn host_is_here(host: Option<&str>, bound: SocketAddr) -> bool {
    host.is_some_and(|host| names_here(host, bound))
}

/// Whether a request's `Origin` names the address this server is listening on.
///
/// A request without one is allowed. `Origin` is a browser's word about itself,
/// and a script or a command-line client is entitled to say nothing — it is the
/// browser this check exists to catch, and a browser always speaks.
#[must_use]
pub fn origin_is_here(origin: Option<&str>, bound: SocketAddr) -> bool {
    origin.is_none_or(|origin| {
        let stated = origin.strip_prefix("http://").unwrap_or(origin);
        names_here(stated, bound)
    })
}

/// Whether `stated` is a loopback name carrying this server's port.
fn names_here(stated: &str, bound: SocketAddr) -> bool {
    let Some((name, port)) = stated.rsplit_once(':') else {
        return false;
    };
    port.parse::<u16>().is_ok_and(|port| port == bound.port()) && is_loopback_name(name)
}

/// Whether a host name is one of loopback's own.
fn is_loopback_name(name: &str) -> bool {
    LOOPBACK.contains(&name) || name.ends_with(".localhost")
}

#[cfg(test)]
mod tests {
    use super::{host_is_here, origin_is_here, Token, TOKEN_HEADER};
    use lemonfiber_core::ports::random::Random;
    use std::net::SocketAddr;

    /// Hands back bytes chosen by the test, or nothing.
    struct Given(Option<Vec<u8>>);

    impl Random for Given {
        fn bytes(&self, _: usize) -> Option<Vec<u8>> {
            self.0.clone()
        }
    }

    /// Built rather than parsed: an address made of numbers cannot fail to be one.
    fn bound() -> SocketAddr {
        SocketAddr::from(([127, 0, 0, 1], 8471))
    }

    /// The bytes every token test is minted from.
    fn given() -> Given {
        Given(Some(vec![0x00, 0x0f, 0xa5, 0xff]))
    }

    #[test]
    fn the_header_is_the_one_both_sides_agreed_on() {
        assert_eq!(TOKEN_HEADER, "X-Lemonfiber-Token");
    }

    #[test]
    fn a_token_is_the_bytes_written_as_hex() {
        assert_eq!(
            Token::mint(&given()).map(|token| token.as_str().to_owned()),
            Some("000fa5ff".to_owned())
        );
    }

    #[test]
    fn there_is_no_token_when_the_system_will_not_say() {
        assert!(Token::mint(&Given(None)).is_none());
    }

    #[test]
    fn a_request_carrying_the_token_is_recognised() {
        assert!(Token::mint(&given()).is_some_and(|token| token.carried_by(Some("000fa5ff"))));
    }

    #[test]
    fn a_request_carrying_something_else_is_not() {
        assert!(!Token::mint(&given()).is_some_and(|token| token.carried_by(Some("000fa5fe"))));
    }

    #[test]
    fn a_request_carrying_a_prefix_is_not() {
        assert!(!Token::mint(&given()).is_some_and(|token| token.carried_by(Some("000fa5"))));
    }

    #[test]
    fn a_request_carrying_nothing_is_not() {
        assert!(!Token::mint(&given()).is_some_and(|token| token.carried_by(None)));
    }

    #[test]
    fn a_host_naming_this_address_is_here() {
        assert!(host_is_here(Some("127.0.0.1:8471"), bound()));
        assert!(host_is_here(Some("localhost:8471"), bound()));
        assert!(host_is_here(Some("[::1]:8471"), bound()));
        assert!(host_is_here(Some("stack.localhost:8471"), bound()));
    }

    #[test]
    fn a_host_naming_another_port_is_not() {
        assert!(!host_is_here(Some("localhost:9000"), bound()));
    }

    #[test]
    fn a_host_naming_somewhere_else_is_not() {
        assert!(!host_is_here(Some("example.com:8471"), bound()));
    }

    #[test]
    fn a_host_carrying_no_port_is_not() {
        assert!(!host_is_here(Some("localhost"), bound()));
    }

    #[test]
    fn a_host_carrying_something_that_is_not_a_port_is_not() {
        assert!(!host_is_here(Some("localhost:doorway"), bound()));
    }

    #[test]
    fn a_request_without_a_host_is_refused() {
        assert!(!host_is_here(None, bound()));
    }

    #[test]
    fn an_origin_naming_this_address_is_here() {
        assert!(origin_is_here(Some("http://localhost:8471"), bound()));
        assert!(origin_is_here(Some("127.0.0.1:8471"), bound()));
    }

    #[test]
    fn an_origin_naming_somewhere_else_is_not() {
        assert!(!origin_is_here(Some("http://evil.example:8471"), bound()));
    }

    #[test]
    fn a_request_stating_no_origin_is_allowed() {
        assert!(origin_is_here(None, bound()));
    }
}
