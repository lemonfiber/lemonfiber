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

/// Where this surface is listening, as much of it as a request has to name.
///
/// The port rather than an address, because a surface offered to a network takes an
/// address on each family it can and a request names whichever one the device it is
/// on reached — so there is no single address to hold a request to, and the port is
/// the part every one of them shares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Binding {
    /// The port every address it took is on.
    pub port: u16,
    /// Whether it is offered past this machine.
    pub beyond: bool,
}

impl Binding {
    /// Listening on this machine and nowhere else, at this port.
    #[must_use]
    pub const fn here(port: u16) -> Self {
        Self {
            port,
            beyond: false,
        }
    }
}

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
    ///
    /// A source that answers with fewer bytes than it was asked for is treated as
    /// one that would not say. The width is what makes guessing hopeless, and a
    /// short answer is invisible in the result: a narrower secret is a secret,
    /// and looks like one right up until somebody guesses it.
    pub fn mint(random: &dyn Random) -> Option<Self> {
        minted(random, WIDTH).map(Self)
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

/// The width asked for, written out as one word, or nothing.
///
/// A source that answers with fewer bytes than it was asked for is treated as one
/// that would not say. The width is what makes guessing hopeless, and a short
/// answer is invisible in the result: a narrower secret is a secret, and looks
/// like one right up until somebody guesses it.
///
/// Asked for here rather than by each caller, because the token checked and the
/// names given to work did not — and a job's name is a capability on the same
/// terms, being the whole of what a caller redeems work by. The doc below already
/// said the two were the same act; only one of them was treated that way.
pub(crate) fn minted(random: &dyn Random, width: usize) -> Option<String> {
    let bytes = random.bytes(width)?;
    if bytes.len() != width {
        return None;
    }
    Some(hex(&bytes))
}

/// Bytes written out as one word.
///
/// Two hex digits per byte, so anything minted here is a word an operator can
/// copy and a stream can carry without it wrapping or needing quoting. Shared
/// between the token and the names given to work, because they are the same act:
/// bytes the operating system chose, written down once.
pub(crate) fn hex(bytes: &[u8]) -> String {
    let mut written = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        written.push(digit(byte >> 4));
        written.push(digit(byte & 0x0f));
    }
    written
}

/// One hex digit, low nibble.
const fn digit(nibble: u8) -> char {
    match nibble {
        0..=9 => (b'0' + nibble) as char,
        _ => (b'a' + nibble - 10) as char,
    }
}

/// Whether a request's `Host` names where this server is listening.
///
/// A request without one is refused: `Host` is not optional in the version of
/// HTTP a browser speaks, and its absence is not something to be lenient about.
#[must_use]
pub fn host_is_here(host: Option<&str>, at: Binding) -> bool {
    host.is_some_and(|host| names_here(host, at))
}

/// Whether a request's `Origin` names the address this server is listening on.
///
/// A request without one is allowed. `Origin` is a browser's word about itself,
/// and a script or a command-line client is entitled to say nothing — it is the
/// browser this check exists to catch, and a browser always speaks.
#[must_use]
pub fn origin_is_here(origin: Option<&str>, at: Binding) -> bool {
    origin.is_none_or(|origin| {
        let stated = origin.strip_prefix("http://").unwrap_or(origin);
        names_here(stated, at)
    })
}

/// Whether `stated` carries this server's port and names somewhere it answers.
fn names_here(stated: &str, at: Binding) -> bool {
    let Some((name, port)) = stated.rsplit_once(':') else {
        return false;
    };
    port.parse::<u16>().is_ok_and(|port| port == at.port) && reaches_here(name, at.beyond)
}

/// Whether a host name is one this surface answers to.
///
/// On this machine, loopback's own names and nothing else — a rebound name that
/// resolves to `127.0.0.1` is refused here even though the connection arrived, which
/// is the whole of what this check is for.
///
/// Offered past this machine there is no such list to hold a request to: the
/// addresses this machine answers on vary by machine, change with the network, and
/// are different again on a laptop that moves. What is held instead is the shape of
/// the word. **A rebinding attack needs a name**, because what it rebinds is a name;
/// an address typed into a browser is not one, and cannot be made to resolve
/// anywhere. So a literal address is let through and a name is not — which costs an
/// operator who reaches this by a name their network hands out, and that is the half
/// of the trade worth saying out loud rather than the half worth hiding.
fn reaches_here(name: &str, beyond: bool) -> bool {
    is_loopback_name(name) || (beyond && is_address(name))
}

/// Whether a host name is one of loopback's own.
fn is_loopback_name(name: &str) -> bool {
    LOOPBACK.contains(&name) || name.ends_with(".localhost")
}

/// Whether a host is written as an address rather than as a name.
///
/// A bracketed form is IPv6 as a `Host` header writes one, and what is inside the
/// brackets is the address.
fn is_address(name: &str) -> bool {
    let bare = name
        .strip_prefix('[')
        .and_then(|inside| inside.strip_suffix(']'))
        .unwrap_or(name);
    bare.parse::<std::net::IpAddr>().is_ok()
}

#[cfg(test)]
mod tests {
    use super::{host_is_here, origin_is_here, Binding, Token, TOKEN_HEADER, WIDTH};
    use lemonfiber_fixtures::ports::Chance;

    /// The byte every test mints from, chosen because its hex is two of one digit,
    /// which makes the token it is written as a repeat rather than a literal.
    const EVERY_BYTE: u8 = 0xab;
    const AS_HEX: &str = "ab";

    /// The token those bytes are written as, built rather than spelled out.
    fn written() -> String {
        AS_HEX.repeat(WIDTH)
    }

    /// Serving this machine and nowhere else.
    fn bound() -> Binding {
        Binding::here(8471)
    }

    /// The same port, offered past this machine.
    fn beyond() -> Binding {
        Binding {
            port: 8471,
            beyond: true,
        }
    }

    /// The source every token test is minted from, answering in full.
    fn given() -> Chance {
        Chance::exactly(Some(vec![EVERY_BYTE; WIDTH]))
    }

    #[test]
    fn the_header_is_the_one_both_sides_agreed_on() {
        assert_eq!(TOKEN_HEADER, "X-Lemonfiber-Token");
    }

    #[test]
    fn a_token_is_the_bytes_written_as_hex() {
        assert_eq!(
            Token::mint(&given()).map(|token| token.as_str().to_owned()),
            Some(written())
        );
    }

    #[test]
    fn a_token_is_as_wide_as_the_secret_is_meant_to_be() {
        // Two hex digits a byte. Asserted rather than assumed, because nothing
        // about a short token looks wrong.
        assert_eq!(
            Token::mint(&given()).map(|token| token.as_str().len()),
            Some(WIDTH * 2)
        );
    }

    #[test]
    fn there_is_no_token_when_the_system_will_not_say() {
        assert!(Token::mint(&Chance::exactly(None)).is_none());
    }

    #[test]
    fn there_is_no_token_when_the_system_says_less_than_it_was_asked() {
        let short = Chance::exactly(Some(vec![EVERY_BYTE; WIDTH - 1]));
        assert!(Token::mint(&short).is_none());
    }

    #[test]
    fn a_request_carrying_the_token_is_recognised() {
        assert!(Token::mint(&given()).is_some_and(|token| token.carried_by(Some(&written()))));
    }

    #[test]
    fn a_request_carrying_something_else_is_not() {
        let wrong = written().replace('a', "b");
        assert!(!Token::mint(&given()).is_some_and(|token| token.carried_by(Some(&wrong))));
    }

    #[test]
    fn a_request_carrying_a_prefix_is_not() {
        // One byte's worth of the token, which every longer one begins with.
        assert!(!Token::mint(&given()).is_some_and(|token| token.carried_by(Some(AS_HEX))));
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

    /// Offered past this machine, an address is let through and a name is not.
    ///
    /// There is no list of the addresses this machine answers on — they vary by
    /// machine, change with the network, and are different again on a laptop that
    /// moves — so what is held is the shape of the word. A rebinding attack needs a
    /// name, because what it rebinds is a name; an address typed into a browser is
    /// not one and cannot be made to resolve anywhere.
    #[test]
    fn offered_to_a_network_an_address_reaches_it_and_a_name_still_does_not() {
        assert!(host_is_here(Some("192.168.1.10:8471"), beyond()));
        assert!(host_is_here(Some("[fe80::1]:8471"), beyond()));
        assert!(host_is_here(Some("localhost:8471"), beyond()));
        assert!(!host_is_here(Some("lemonfiber.local:8471"), beyond()));
        assert!(!host_is_here(Some("evil.example:8471"), beyond()));
        // And the port is still held, whichever address named it.
        assert!(!host_is_here(Some("192.168.1.10:9000"), beyond()));
        // On this machine, an address off loopback reaches nothing at all.
        assert!(!host_is_here(Some("192.168.1.10:8471"), bound()));
    }
}
