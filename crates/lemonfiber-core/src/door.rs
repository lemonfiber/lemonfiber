//! Which one service the household is sent to, and the honest answer where there
//! is none.
//!
//! The stack publishes several things to the local network, and left to improvise an
//! operator sends three links and a paragraph explaining which is which — the exact
//! complexity this product exists to remove. So the question has one answer, derived
//! here rather than decided again by each surface that shows it.
//!
//! The two that can be a door are told apart by the shape of their API rather than
//! by their name, the way every other service resolution here is, so a stack that
//! ships a different request service under the same shape resolves the same way. The
//! rest speak no API lemonfiber knows, so each is written down below with what it is
//! to the household — and one nobody has written down is not offered, for the reason
//! [`crate::config::display`] withholds a setting nobody vouched for.
//!
//! All of which is a default rather than a decree. An operator who disagrees about
//! their own stack names the service they want, and [`chosen`] is where that name
//! meets the one bound on it: a setting may pick between the places this stack
//! already publishes to the household, and may not turn into a way past the tiers.

mod address;
mod chosen;
pub(crate) mod run;

use serde::Serialize;

use lemonfiber_manifest::{ApiKind, Bind, Service};

pub use address::{address, publishes_a_name, Address};
pub use chosen::{chosen, Chosen, Refusal, KEPT};

/// What a service published to the local network is to the people in the house.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Facing {
    /// Where asking for something begins, and where what was asked for is followed.
    /// It links onward to the library, which is what makes it a place to start
    /// rather than one more address to be sent.
    Asking,
    /// The library, where what arrived is watched. A place to start only where there
    /// is nothing to ask for, because somebody sent there has no way to ask.
    Watching,
    /// One kind of media and no other, reached from the library rather than instead
    /// of it.
    Shelf,
    /// An index over every service, including the ones nobody in the house should
    /// learn exist.
    Operators,
    /// How the others are reached, rather than one of them.
    Carriage,
    /// Published to the household, and nothing here says what it is to them.
    Unstated,
}

impl Facing {
    /// Why this is, or is not, somewhere for the household to begin.
    #[must_use]
    pub const fn because(self) -> &'static str {
        match self {
            Self::Asking => {
                "where a request begins, and where what was asked for is followed — and \
                 it links onward to the library"
            }
            Self::Watching => {
                "where the library is watched, and no way to ask for anything that is \
                 not in it yet"
            }
            Self::Shelf => "one kind of media, reached from the library rather than instead of it",
            Self::Operators => {
                "an index over every service, including the ones nobody in the house \
                 should learn exist — the operator's convenience, never a way in"
            }
            Self::Carriage => "how the others are reached, rather than one of them",
            Self::Unstated => {
                "nothing here says what this is to the household, so it is not offered \
                 as somewhere to begin"
            }
        }
    }

    /// Whether the household can be sent here as a place to start.
    #[must_use]
    pub const fn begins(self) -> bool {
        matches!(self, Self::Asking | Self::Watching)
    }

    /// Whether somebody in the house may be handed the way there at all.
    ///
    /// Wider than [`Self::begins`] and narrower than every service there is. A shelf
    /// is not somewhere to *start* — it holds one kind of media and is reached from
    /// the library — but a member sent to it arrives somewhere meant for them, so an
    /// address is a thing they can be given.
    ///
    /// **The other three are named in order to be refused by name.** The index over
    /// every service exists for the operator and includes the ones nobody in the
    /// house should learn exist; the carriage is how the others are reached rather
    /// than one of them; and a service nothing has said anything about is offered to
    /// nobody, which is the register's own default and the safe direction to be
    /// wrong in. Naming one of those and handing over the way to it are different
    /// acts, and only the first is what standing beside the door means.
    #[must_use]
    pub const fn handed_over(self) -> bool {
        matches!(self, Self::Asking | Self::Watching | Self::Shelf)
    }
}

/// The services published to the household that speak no API lemonfiber knows.
///
/// A register rather than a rule read off a name, for the reason
/// [`crate::config::display::SHOWN`] is one: a guess is wrong in whichever direction
/// nobody chose, and the service that ends up wrongly offered to a household is by
/// definition the one nobody thought about. So an entry is a decision somebody made
/// and can be reviewed on; anything else is [`Facing::Unstated`] and is offered to
/// nobody.
const NAMED: &[(&str, Facing)] = &[
    ("calibre-web-automated", Facing::Shelf),
    ("audiobookshelf", Facing::Shelf),
    ("navidrome", Facing::Shelf),
    ("homepage", Facing::Operators),
    ("caddy", Facing::Carriage),
];

/// What this service is to the household, or nothing where it is not published to
/// them at all.
///
/// Publication is the manifest's own [`Bind`], so a service reachable only from this
/// machine is not a candidate for anything here — which is the same fact the admin
/// tier's loopback binding rests on, read from the same field rather than from a
/// second list that could disagree with it.
#[must_use]
pub fn facing(service: &Service) -> Option<Facing> {
    if service.bind != Some(Bind::Lan) {
        return None;
    }
    match service.api.as_ref().map(|api| api.kind) {
        Some(ApiKind::Seerr) => Some(Facing::Asking),
        Some(ApiKind::Jellyfin) => Some(Facing::Watching),
        _ => Some(named(&service.id)),
    }
}

/// What the register says this service is, or that nobody has said.
fn named(id: &str) -> Facing {
    NAMED
        .iter()
        .find(|(named, _)| *named == id)
        .map_or(Facing::Unstated, |(_, facing)| *facing)
}

/// The one service the household begins at, from everything the stack declares.
///
/// Whichever request surface the stack has, and the library where it has none. Read
/// from what the stack *declares* rather than from what is up at this moment,
/// deliberately: a request service that is not running is a front door that is down,
/// and answering "the library, then" would hand the household somewhere they cannot
/// ask for anything without ever saying that is what happened.
///
/// Nothing where the stack publishes neither. That is an answer — an operator-only
/// configuration has no household front door — and it is said as one rather than
/// filled in with the nearest thing that would open.
#[must_use]
pub(crate) fn begins_at(services: &[Service]) -> Option<(Facing, &Service)> {
    let mut best: Option<(Facing, &Service)> = None;
    for service in services {
        let Some(facing) = facing(service) else {
            continue;
        };
        if !facing.begins() {
            continue;
        }
        if facing == Facing::Asking {
            return Some((facing, service));
        }
        if best.is_none() {
            best = Some((facing, service));
        }
    }
    best
}

/// A manifest service built for a test, for the two modules that need one.
///
/// Beside the rule rather than inside either test module: what a stack declares is
/// this file's vocabulary, and a second copy of a literal that names every field
/// the schema declares is a second thing to keep agreeing with it.
#[cfg(test)]
pub(crate) mod fixtures;

#[cfg(test)]
mod tests;
