//! Every credential the stack holds, named rather than shown.
//!
//! The stack accumulates secrets from three different directions and they are not
//! the same kind of thing. The operator supplies some — a VPN private key, a Usenet
//! password, an indexer key. The services mint most of them for themselves and write
//! them into their own configuration. And a few lemonfiber has to mint, because the
//! service offers nothing durable to read.
//!
//! What every one of them shares is that an operator needs to know it exists, what
//! uses it, and where it lives, and needs that answer without the value crossing a
//! terminal, a screenshot or a support bundle. So [`Held`] carries no value field at
//! all. Withholding a value you are holding is a rule somebody has to keep; having
//! nowhere to put it is a property of the shape, and the difference is the whole
//! reason this type is written this way.
//!
//! The one place a value is carried is [`Revealed`], which exists only because
//! refusing an operator their own secret outright is paternalism rather than
//! security. It is built nowhere except by an explicit, confirmed ask.
//!
//! See `.docs/architecture/module-layout.md`.

mod held;
mod protection;
mod rotation;

pub use held::{catalogue, Consumer, Entry, Needed, Origin, Reached, CATALOGUE, VPN_PRIVATE_KEY};
pub use protection::Protection;
pub use rotation::{Propagation, Reach, Rotation, Settled};

use serde::Serialize;

/// Where a credential stands, as far as lemonfiber can tell without spending it.
///
/// Six states rather than a boolean because the operator's next move differs for
/// each: an absent credential is one to supply, a stale one is one to prove, an
/// invalid one is one to replace, and the two rotation states exist so a run
/// interrupted half-way through a replacement is legible rather than mysterious.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "CredentialState")]
pub enum State {
    /// Required by what this stack runs, and not supplied.
    Absent,
    /// Present, and proven against the service it authenticates to.
    Active,
    /// Present, and never proven — or not proven since it was last written.
    Stale,
    /// Present, and refused by the service the last time it was offered.
    Invalid,
    /// A replacement is being proven; the existing value is still the one in force.
    Rotating,
    /// Replaced. What is left is the old value, waiting to be destroyed.
    Superseded,
}

impl State {
    /// The state as it is written on a surface.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Absent => "absent",
            Self::Active => "active",
            Self::Stale => "stale",
            Self::Invalid => "invalid",
            Self::Rotating => "rotating",
            Self::Superseded => "superseded",
        }
    }

    /// Whether this state is one to say something about rather than pass over.
    ///
    /// An advisory, never an expiry: a credential that has gone stale is still a
    /// working credential, and a product that retired it on a schedule would break a
    /// stack on a day its owner did not choose.
    #[must_use]
    pub(crate) const fn worth_advising(self) -> bool {
        matches!(self, Self::Stale | Self::Invalid | Self::Absent)
    }
}

/// One credential, described without being disclosed.
///
/// There is deliberately no value here, and no field a value could be put in later
/// without the change being visible in review.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[schemars(rename = "CredentialHeld")]
pub struct Held {
    /// What it is, in the operator's words — `qBittorrent web UI password`.
    pub name: String,
    /// The setting it is recorded under, which is a name and never a value.
    pub setting: String,
    /// Everything that authenticates with it. Named individually, because a
    /// consumer left off this list is a consumer a rotation would silently strand.
    pub consumers: Vec<String>,
    /// Where the value lives, as a path or a description of one.
    pub location: String,
    /// Who produced it.
    pub origin: Origin,
    /// Whose line it is: the stack's own, or an installed plugin's, named.
    ///
    /// A different question from who produced it. A plugin's secret is minted by the
    /// service it belongs to like any other, and what the operator needs to know as well
    /// is that the service is one a plugin brought.
    pub from: crate::origin::Origin,
    /// Where it stands.
    pub state: State,
    /// A short likeness of the value, for telling two copies apart in a report.
    ///
    /// Absent where there is no value to take one of. Never reversible and never a
    /// proof — see [`fingerprint`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<String>,
    /// What is worth saying about this one, where anything is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub advisory: Option<String>,
}

impl Held {
    /// The installed plugin this line is the secret of, where it is one.
    #[must_use]
    pub fn plugins(&self) -> Option<&str> {
        match &self.from {
            crate::origin::Origin::Plugin { named } => Some(named),
            _ => None,
        }
    }

    /// What is said of a plugin's secret wherever its value is asked for.
    ///
    /// One sentence for a reveal and a rotation both, because the answer to each is the
    /// same fact: nothing holds it, so there is nothing to print and nothing to replace.
    #[must_use]
    pub fn unheld(&self, plugin: &str) -> String {
        format!(
            "{} is the plugin {plugin}'s, and nothing holds it yet: lemonfiber holds a \
             plugin's secret only once the plugin's recipes capture it, and this build runs \
             none. There is no value to show or to replace.",
            self.name
        )
    }
}

/// One stored value, handed back because the operator asked for it and said so.
///
/// Separate from [`Held`] rather than a field on it, so that the inventory cannot
/// carry a value by accident: a surface that renders the inventory has nothing to
/// render, whatever it does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Revealed {
    /// Which credential this is.
    pub name: String,
    /// The value, present only where the ask was confirmed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// What the operator is told before it appears, whether or not it appears.
    pub warning: String,
}

/// What an unconfirmed reveal says instead of the value.
pub const SHOULDER: &str = "This prints a credential in the clear. It will sit in your \
terminal's scrollback, in any recording of this session, and in view of anyone looking over \
your shoulder. Ask again with the confirmation to print it.";

/// What a confirmed reveal says beside the value.
pub const REVEALED: &str = "This is a credential in the clear. Clear your scrollback when \
you are done with it, and do not paste it into a support request.";

/// The whole answer to a question about credentials.
///
/// Every ask answers with the inventory as it now stands, and adds what became of
/// whatever else was asked for. An operator who has just rotated something wants to
/// see the inventory that rotation produced, not a receipt they have to go and check
/// against one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Inventory {
    /// Every credential, whether or not it is present.
    pub held: Vec<Held>,
    /// What keeping them in files does and does not protect against.
    pub protection: Protection,
    /// What became of a rotation, where one was asked for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rotated: Option<Rotation>,
    /// One value, where one was asked for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revealed: Option<Revealed>,
}

impl Inventory {
    /// An inventory of what is held, with nothing else asked of it.
    #[must_use]
    pub fn of(held: Vec<Held>) -> Self {
        Self {
            held,
            protection: Protection::stated(),
            rotated: None,
            revealed: None,
        }
    }

    /// The same inventory, carrying what became of a rotation.
    #[must_use]
    pub fn after(mut self, rotated: Rotation) -> Self {
        self.rotated = Some(rotated);
        self
    }

    /// The same inventory, carrying one revealed value.
    #[must_use]
    pub fn showing(mut self, revealed: Revealed) -> Self {
        self.revealed = Some(revealed);
        self
    }

    /// Everything worth telling the operator about, in the order it is held.
    ///
    /// Advisories rather than actions: what is here is a sentence, and nothing in this
    /// product acts on one by itself. The state decides which credentials may produce
    /// one, so a sentence attached to a working credential is not shown by accident —
    /// the two would otherwise be one rule written twice, in two files.
    #[must_use]
    pub fn advisories(&self) -> Vec<&str> {
        self.held
            .iter()
            .filter(|held| held.state.worth_advising())
            .filter_map(|held| held.advisory.as_deref())
            .collect()
    }
}

/// How many hex characters a fingerprint is written with.
const FINGERPRINT_WIDTH: usize = 4;

/// A short, stable likeness of a value, for saying that two copies of a credential
/// disagree without printing either of them.
///
/// Sixteen bits, which is enough to notice a difference and far too few to work back
/// to the value: a fingerprint names about one in sixty-five thousand possible
/// values, so it identifies nothing on its own. It is a label, not a proof, and the
/// narrowness is the point — a longer one would start to be worth attacking.
///
/// Stable across runs and across versions, which a hasher from the standard library
/// is not: `DefaultHasher`'s output is explicitly allowed to change between
/// releases, and a fingerprint that moved under an upgrade would report every
/// credential in the stack as newly regenerated.
#[must_use]
pub fn fingerprint(value: &str) -> String {
    // FNV-1a over the bytes, folded to its low sixteen bits.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("~{:0FINGERPRINT_WIDTH$x}", hash & 0xffff)
}

#[cfg(test)]
mod tests;
