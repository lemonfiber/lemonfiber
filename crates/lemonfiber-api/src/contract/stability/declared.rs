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
pub const DECLARED: &[Declared] = &[
    Declared {
        under: 1,
        what: "PluginPair.approval",
        moved: Moved::MayBeAbsent,
        because: "a pair to a service in the stack carries nothing out of the machine, so it \
                  asks for no approval and carries none; a pair to a host outside the stack \
                  carries one, as it always has",
    },
    Declared {
        under: 1,
        what: "PluginPlaced.api",
        moved: Moved::Gone,
        because: NO_ADAPTER,
    },
    Declared {
        under: 1,
        what: "PluginInstalled.adapters",
        moved: Moved::Gone,
        because: NO_ADAPTER,
    },
    Declared {
        under: 1,
        what: "SeedReport.unsupported",
        moved: Moved::Gone,
        because: NOTHING_UNASKED,
    },
    Declared {
        under: 1,
        what: "StatusReport.unsupported",
        moved: Moved::Gone,
        because: NOTHING_UNASKED,
    },
    Declared {
        under: 1,
        what: "StuckReport.unsupported",
        moved: Moved::Gone,
        because: NOTHING_UNASKED,
    },
    Declared {
        under: 1,
        what: "Api",
        moved: Moved::Gone,
        because: "it was the type of PluginPlaced.api alone, which is gone, so nothing names it",
    },
    Declared {
        under: 1,
        what: "KeySource",
        moved: Moved::Gone,
        because: "it was a field of Api alone, which is gone, so nothing names it",
    },
    Declared {
        under: 1,
        what: "PluginServiceAdapter",
        moved: Moved::Gone,
        because: "it was the type of PluginInstalled.adapters alone, which is gone, so nothing \
                  names it",
    },
    Declared {
        under: 1,
        what: "SettingChange.setting",
        moved: Moved::Gone,
        because: ONE_ACT,
    },
    Declared {
        under: 1,
        what: "SettingChange.was",
        moved: Moved::Gone,
        because: ONE_ACT,
    },
    Declared {
        under: 1,
        what: "SettingChange.value",
        moved: Moved::Gone,
        because: ONE_ACT,
    },
    Declared {
        under: 1,
        what: "SettingChange.consequence",
        moved: Moved::Gone,
        because: ONE_ACT,
    },
    Declared {
        under: 1,
        what: r#"PluginPuts = "region""#,
        moved: Moved::Gone,
        because: "an install writes into no file the stack already has: a plugin's route is \
                  the proxy's routing table, applied over its contract, so no change it states \
                  is a region",
    },
    Declared {
        under: 1,
        what: "ServiceNetwork.via",
        moved: Moved::Retyped,
        because: FENCED,
    },
    Declared {
        under: 1,
        what: "ServiceNetwork.via",
        moved: Moved::MayBeAbsent,
        because: FENCED,
    },
];

/// Why a network may name no capability: a fenced service runs inside its fence, whose
/// network is no filler's, and a service taking `via` still names it as it always has.
const FENCED: &str = "a fenced service runs inside its fence rather than a filler's network, so \
                      its network says fenced and names no capability; a service running \
                      inside a filler's network names it in via as it always has";

/// Why a change's one setting is gone: a change moves one or more settings in one act, and
/// each is carried in its `settings`.
const ONE_ACT: &str = "a change moves one or more settings in one act, each carried with what \
                       it held, what it is to hold and what moving it costs in its settings; \
                       no release carried the one setting a change used to name";

/// Why what named an in-process adapter is gone: no service names one.
const NO_ADAPTER: &str = "no service names an in-process adapter, so there is none to carry";

/// Why a list of services no adapter could ask is gone: no service names an API through
/// an in-process adapter, so nothing is left unasked for that reason.
const NOTHING_UNASKED: &str = "it listed each stack service declaring an API through an \
                               in-process adapter lemonfiber could not ask, and no service \
                               names one";

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
