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
pub fn reaches_the_household(recorded: &str) -> bool {
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
/// A name carrying a dot is left as it is: it is already qualified, and putting a
/// second suffix on the end of one would ask for a machine that does not exist.
fn reachable(name: &str) -> String {
    if name.contains('.') {
        return name.to_owned();
    }
    format!("{name}{LOCAL}")
}

#[cfg(test)]
mod tests {
    use super::{address, publishes_a_name, reaches_the_household, Address, MAY_CHANGE};
    use crate::platform::Environment;

    #[test]
    fn a_name_is_preferred_over_the_address_somebody_wrote_down() {
        assert_eq!(
            address(
                Some("kitchen-nas"),
                Some("192.168.1.10"),
                Environment::MacOs,
                5055
            ),
            Some(Address {
                url: "http://kitchen-nas.local:5055".to_owned(),
                caution: None,
            })
        );
    }

    #[test]
    fn an_already_qualified_name_is_asked_for_as_it_stands() {
        // A second suffix on the end of one would ask for a machine nobody has.
        assert_eq!(
            address(Some("nas.lan"), None, Environment::Windows, 8096).map(|address| address.url),
            Some("http://nas.lan:8096".to_owned())
        );
    }

    #[test]
    fn a_machine_with_no_responder_is_not_offered_by_a_name_that_resolves_nowhere() {
        // A name that fails does not look wrong, which is what makes it worse than
        // a number that says it may change.
        assert!(!publishes_a_name(Environment::LinuxNative));
        assert!(!publishes_a_name(Environment::LinuxDesktop));
        assert!(!publishes_a_name(Environment::Unsupported));
        assert!(publishes_a_name(Environment::MacOs));
        assert!(publishes_a_name(Environment::Windows));
    }

    #[test]
    fn a_number_is_given_with_the_note_that_it_may_change() {
        assert_eq!(
            address(
                Some("kitchen-nas"),
                Some("192.168.1.10"),
                Environment::LinuxNative,
                5055
            ),
            Some(Address {
                url: "http://192.168.1.10:5055".to_owned(),
                caution: Some(MAY_CHANGE.to_owned()),
            })
        );
    }

    #[test]
    fn a_name_the_operator_wrote_down_is_taken_at_its_word() {
        // Their network resolves it, which this cannot check and should not overrule.
        assert_eq!(
            address(None, Some("nas.home.arpa"), Environment::LinuxNative, 5055),
            Some(Address {
                url: "http://nas.home.arpa:5055".to_owned(),
                caution: None,
            })
        );
    }

    #[test]
    fn the_address_this_stack_ships_with_reaches_nobody_and_is_not_offered() {
        // What it ships with means this machine and nowhere else, which is right for
        // a machine nobody has told where it is and wrong to hand anybody.
        assert!(!reaches_the_household("localhost"));
        assert!(!reaches_the_household("LOCALHOST"));
        assert!(!reaches_the_household("127.0.0.1"));
        assert!(!reaches_the_household("::1"));
        assert!(!reaches_the_household("   "));
        assert!(reaches_the_household("192.168.1.10"));
        assert!(reaches_the_household("nas.home.arpa"));
    }

    #[test]
    fn an_address_published_on_every_interface_names_none_of_them() {
        // What the stack's own binding setting defaults to says which interfaces a
        // service is published on, and nothing at all about where to reach it.
        let every = std::net::Ipv4Addr::UNSPECIFIED.to_string();
        assert!(!reaches_the_household(&every));
        assert_eq!(
            address(None, Some(&every), Environment::LinuxNative, 5055),
            None
        );
    }

    #[test]
    fn a_machine_that_says_nothing_and_a_file_that_says_nothing_give_no_address() {
        // Rather than one built from a default, which is an address somebody sends
        // on and nobody can reach.
        assert_eq!(address(None, None, Environment::MacOs, 5055), None);
        assert_eq!(address(Some("  "), None, Environment::MacOs, 5055), None);
        assert_eq!(
            address(Some("kitchen-nas"), None, Environment::LinuxNative, 5055),
            None
        );
    }
}
