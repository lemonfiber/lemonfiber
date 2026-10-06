//! The breaks accepted without moving the wire version, each declared by name.
//!
//! A break is refused unless the wire version moves, and moving it is the right
//! answer for a change every consumer has to absorb. Some breaks are narrower than
//! that: a field that stops being sent in a case where it never meant anything. Such
//! a break lands under the version it was found under only where it is written here
//! — the version, the one name, how it moved and why — so the generator and the suite
//! accept that change and still refuse every other, including another change to the
//! same name. A declaration names the version it was accepted under, so moving the
//! wire version leaves it accepting nothing.

use super::{Break, Moved};

/// One break accepted under one wire version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Declared {
    /// The wire version the break was accepted under.
    pub under: u32,
    /// What moved, spelled as [`Break::what`] spells it.
    pub what: &'static str,
    /// How it moved.
    pub moved: Moved,
    /// Why a consumer is not left reading missing data.
    pub because: &'static str,
}

/// Every break accepted without moving the wire version.
pub const DECLARED: &[Declared] = &[Declared {
    under: 1,
    what: "PluginPair.approval",
    moved: Moved::MayBeAbsent,
    because: "a pair to a service in the stack carries nothing out of the machine, so it \
              asks for no approval and carries none; a pair to a host outside the stack \
              carries one, as it always has",
}];

/// The breaks no declaration accepts, of those found between two surfaces of
/// `version`.
pub(super) fn undeclared(breaks: Vec<Break>, version: u32, declared: &[Declared]) -> Vec<Break> {
    breaks
        .into_iter()
        .filter(|one| {
            !declared
                .iter()
                .any(|accepted| accepts(accepted, one, version))
        })
        .collect()
}

/// Whether one declaration accepts one break found under `version`.
fn accepts(accepted: &Declared, one: &Break, version: u32) -> bool {
    accepted.under == version && accepted.what == one.what && accepted.moved == one.moved
}

#[cfg(test)]
mod tests;
