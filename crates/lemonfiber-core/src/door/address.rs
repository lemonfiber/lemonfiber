//! The address the household is handed, and what that address is worth.
//!
//! Two kinds of address, and the difference between them is what a bookmark is
//! worth six months later. A name this machine answers to survives the router
//! handing out a different lease; the number the router handed out does not, and a
//! household member whose bookmark has quietly stopped working does not debug it —
//! they ask the operator, or they stop using the thing.
//!
//! So the name is preferred wherever there is one to prefer, the operator's own
//! recorded address is used where there is not, and where there is neither there is
//! no address rather than a guess. An address invented from a default is one
//! somebody sends on, and the whole of this feature is that what gets sent on is
//! true.
//!
//! Nothing here is remembered. The address is built from what the machine says
//! about itself at the moment of asking, so one that has been renamed or moved
//! answers as it is now rather than as it was when something last looked.

use std::net::IpAddr;

use serde::Serialize;

use crate::platform::Environment;

/// The suffix a machine's own name is reached under on the local network.
const LOCAL: &str = ".local";

/// The recorded address that means this machine and nowhere else.
const ONLY_HERE: &str = "localhost";

/// What is said about an address that was written down as a number.
const MAY_CHANGE: &str = "That address is a number, and routers hand out different ones — so it \
                          can stop working without anything here having changed.";

/// Where the front door is reached, and what is worth knowing about the address.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Address {
    /// The whole address, as it would be typed or followed.
    pub url: String,
    /// What is worth knowing about the address itself, where anything is. Absent
    /// for one that keeps working on its own.
    pub caution: Option<String>,
}

/// Whether this machine answers to its own name on the local network without
/// anything having been installed for it.
///
/// macOS and Windows each ship a responder and answer to `name.local` out of a
/// clean install. On Linux the responder is a separate package that many hosts do
/// not run, so a name offered there is one that may resolve nowhere — which is a
/// worse address than a number, because it fails without looking wrong.
#[must_use]
pub const fn publishes_a_name(environment: Environment) -> bool {
    match environment {
        Environment::MacOs | Environment::Windows => true,
        Environment::LinuxNative | Environment::LinuxDesktop | Environment::Unsupported => false,
    }
}

/// Whether a recorded address is one another device could reach.
///
/// The default this stack ships is this machine and nowhere else, which is the
/// right default for a machine nobody has told where it is and the wrong address to
/// hand anybody. An address that is not a number is taken at its word: a name the
/// operator wrote down is one their network resolves, which is not something this
/// can check and not something it should overrule.
#[must_use]
pub(crate) fn reaches_the_household(recorded: &str) -> bool {
    let written = recorded.trim();
    if written.is_empty() {
        return false;
    }
    written.parse::<IpAddr>().map_or_else(
        |_| !written.eq_ignore_ascii_case(ONLY_HERE),
        |address| !(address.is_loopback() || address.is_unspecified()),
    )
}

/// The address to hand the household for a service on this machine's `port`.
///
/// `name` is what the machine calls itself and `recorded` is what the operator
/// wrote down as the address their household links point at. Nothing where neither
/// answers.
#[must_use]
pub fn address(
    name: Option<&str>,
    recorded: Option<&str>,
    environment: Environment,
    port: u16,
) -> Option<Address> {
    if publishes_a_name(environment) {
        if let Some(name) = name.map(str::trim).filter(|name| !name.is_empty()) {
            return Some(Address {
                url: format!("http://{}:{port}", reachable(name)),
                caution: None,
            });
        }
    }
    let written = recorded
        .map(str::trim)
        .filter(|recorded| reaches_the_household(recorded))?;
    Some(Address {
        url: format!("http://{written}:{port}"),
        caution: written
            .parse::<IpAddr>()
            .is_ok()
            .then(|| MAY_CHANGE.to_owned()),
    })
}

/// A machine's name as another device on the network asks for it.
///
/// Always the `.local` form, because this is only reached where a responder answers
/// to it — that is what [`publishes_a_name`] establishes, and it is the one name
/// this can promise resolves.
///
/// **A domain already on the name is dropped rather than kept.** What the machine
/// reports about itself carries whatever suffix the router handed out with the
/// lease, and a router that hands out a domain does not necessarily answer for it:
/// `hostname` on a Mac behind a common home router reads
/// `machine.fritz.box`, which resolves nowhere, while `machine.local` answers. A
/// name that fails is worse than a number, because it fails without looking wrong.
///
/// An operator whose network really does resolve a qualified name is not losing it
/// — they write it down, and [`address`] takes a recorded name at its word.
fn reachable(name: &str) -> String {
    if name.ends_with(LOCAL) {
        return name.to_owned();
    }
    let itself = name.split_once('.').map_or(name, |(before, _)| before);
    format!("{itself}{LOCAL}")
}

#[cfg(test)]
mod tests;
