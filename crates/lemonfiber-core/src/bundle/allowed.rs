//! What may be shown as it is, and what stands in for the rest.
//!
//! One question asked of named fields: is this one of the few whose value is safe to read
//! in a bundle somebody may attach to a public thread? Everything else gets a stand-in that
//! reads the same wherever the same value appeared, and means nothing anywhere else.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::ports::random::Random;

use super::Terms;

/// How many bytes of salt a bundle's stand-ins are derived with.
///
/// Sixteen is far past guessing, and the salt is the whole of what makes a stand-in
/// meaningless outside its own bundle — the derivation carries no secret of its own.
pub const SALT_BYTES: usize = 16;

/// The settings whose values may be shown as they are.
///
/// Named one at a time, deliberately. Everything absent from this list is replaced,
/// including settings that do not exist yet — which is the property being bought: a
/// service that adds a field tomorrow is redacted tomorrow, with nobody having to notice.
///
/// What is here is what a person helping actually needs: which provider, which mode, which
/// ports, which identities the containers run as, and whether a credential was ever
/// validated — never the credential. The two host-shaped values, the data root and the
/// Usenet host, are here because a stack cannot be diagnosed without knowing where its
/// files are and who it talks to; both are offered for redaction to an operator who would
/// rather not say. An indexer's address is here for the same reason, and is the case that
/// makes the query-string rule below necessary rather than tidy: the address is worth
/// sharing and the key riding in its query is not, and they arrive as one string.
const SHOWN: [&str; 15] = [
    "DATA_ROOT",
    "PUID",
    "PGID",
    "TZ",
    "VPN_PROVIDER",
    "VPN_PORT_FORWARDING",
    "JELLYFIN_MODE",
    "LEMONFIBER_USENET",
    "LEMONFIBER_TORRENT",
    "LEMONFIBER_IP_ECHO",
    "INDEXER_URL",
    "USENET_HOST",
    "USENET_PORT",
    "USENET_TLS",
    "USENET_VALIDATED",
];

/// Settings whose *names* end in something a service uses for a validated flag rather
/// than for a secret — `INDEXER_VALIDATED` is safe where `INDEXER_APIKEY` is not, and both
/// begin the same way.
const SHOWN_SUFFIX: &str = "_VALIDATED";

/// Whether a setting's value may be shown as it is.
#[must_use]
pub fn shown(name: &str) -> bool {
    let name = name.trim();
    SHOWN.contains(&name) || name.ends_with(SHOWN_SUFFIX)
}

/// The stand-ins one bundle uses for the values it will not show.
///
/// Salted per bundle so the likeness a reader can see — these two services point at the
/// same account — holds only inside the bundle it was read from. Without that, the same
/// key would produce the same mark in every bundle ever posted, which is a fingerprint
/// that outlives the redaction it was meant to be.
pub struct Marks {
    salt: Vec<u8>,
}

impl Marks {
    /// Stand-ins for one bundle, or nothing where the machine could not provide the
    /// randomness — which is not something to paper over with a fixed salt, because a
    /// predictable stand-in is a way back to the value it stands for.
    #[must_use]
    pub fn new(random: &dyn Random) -> Option<Self> {
        Some(Self {
            salt: random.bytes(SALT_BYTES)?,
        })
    }

    /// What `value` is shown as instead of itself.
    ///
    /// Four characters, which is what makes it readable in a config file an operator is
    /// scanning; it carries at most sixteen bits about a value nobody can invert without
    /// the salt, and nothing is ever checked against it — it is a label, not a proof.
    #[must_use]
    pub fn of(&self, value: &str) -> String {
        let mut hasher = DefaultHasher::new();
        self.salt.hash(&mut hasher);
        value.hash(&mut hasher);
        format!("<redacted:{:04x}>", hasher.finish() & 0xffff)
    }
}

/// A settings file as it may be shared: every value not named safe replaced by its mark.
///
/// The names stay, always. A reader helping needs to know that an indexer key is set at
/// all — an empty setting and a set one are different faults — and the name of a setting
/// is not a secret in any stack anyone has ever run.
#[must_use]
pub fn settings(body: &str, marks: &Marks, terms: &Terms) -> String {
    body.lines()
        .map(|line| setting(line, marks, terms))
        .collect::<Vec<_>>()
        .join("\n")
}

/// One settings line, shown or marked.
fn setting(line: &str, marks: &Marks, terms: &Terms) -> String {
    let Some((name, value)) = line.split_once('=') else {
        return line.to_owned();
    };
    if value.trim().is_empty() {
        return line.to_owned();
    }
    // Named by its operator, one field at a time and confirmed on the same breath. The
    // allow-list decides what is safe to share; this is somebody deciding, for one field,
    // that they would rather be understood than careful — which the first page then says
    // out loud, because they will not be there when it is read.
    if terms.reveals(name) {
        return line.to_owned();
    }
    if shown(name) {
        return format!("{name}={}", url(value.trim(), marks));
    }
    format!("{name}={}", marks.of(value.trim()))
}

/// A URL with everything after its question mark marked.
///
/// The shape that catches people out: an indexer's address is worth sharing and the key
/// riding in its query string is not, and the two arrive as one string. So a value that is
/// allowed through keeps its address and loses its parameters wholesale — a query nobody
/// reads is a smaller loss than a key everybody can.
fn url(value: &str, marks: &Marks) -> String {
    match value.split_once('?') {
        None => value.to_owned(),
        Some((address, query)) => format!("{address}?{}", marks.of(query)),
    }
}
