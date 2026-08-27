//! How far this surface may be reached, and what has to be true first.
//!
//! This surface can start, stop and reconfigure the whole stack and reaches every
//! credential the system holds. It is the most privileged thing in the product, so
//! it answers this machine and nothing else unless somebody asks otherwise — and
//! asking otherwise is **refused** rather than warned about, unless a password has
//! been set. A warning that can be clicked past is how unauthenticated control
//! surfaces end up on networks.
//!
//! **Refusing to be offered and giving up an offer already made are one rule read
//! at two moments.** [`permitted`] is that rule: what may be reached is a function
//! of what was asked for and whether a password is set, and nothing else. Before a
//! socket exists there is nothing to fall back to, so a request that fails it is
//! refused; after one exists, refusing outright would take the surface away from
//! the operator too, so it falls back to the address it would have been given. Two
//! answers, one predicate, and neither of them is written twice.
//!
//! **Both families or neither.** A tier names an address on each of IPv4 and IPv6,
//! and every one that can be taken is taken — a policy enforced on one family and
//! silently absent on the other is worse than none, because it reads as enforced.
//! Which ones were actually taken is what gets printed, rather than which ones were
//! meant: on a machine whose IPv6 wildcard already answers for IPv4 there is one
//! socket and the operator is told about one socket.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use lemonfiber_core::error::{Code, Problem, Remedy, Severity, State};
use lemonfiber_core::PRODUCT;
use tokio::net::TcpListener;

/// Raised when the address the surface was asked to serve on cannot be taken.
const ADDRESS_TAKEN: Code = Code::new("SERVE-1");

/// Raised when the network was asked for and nothing here can say who is knocking.
const NO_PASSWORD: Code = Code::new("SERVE-3");

/// How far this surface was asked to be reachable.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Reach {
    /// This machine, and nowhere else. What is asked for where nothing says
    /// otherwise, because the alternative is the most privileged surface in the
    /// product arriving on a network nobody decided to put it on.
    #[default]
    Machine,
    /// Every interface this machine has, which is how a household reaches it from a
    /// phone or a television.
    Network,
}

/// What the policy allows, having read what is configured.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Offered {
    /// This machine. Nothing has to be true first.
    Machine,
    /// The network, which needed a password and found one.
    Network,
    /// The network was asked for and there is no password to ask anybody for.
    Refused,
}

/// What may be reached, given what was asked for and whether a password is set.
///
/// The whole of the policy, in one place, so the moment before a socket exists and
/// the moment after it does are reading the same rule rather than two that agree
/// today.
pub(crate) const fn permitted(asked: Reach, password: bool) -> Offered {
    match (asked, password) {
        (Reach::Machine, _) => Offered::Machine,
        (Reach::Network, true) => Offered::Network,
        (Reach::Network, false) => Offered::Refused,
    }
}

/// The addresses a tier names, one on each family.
///
/// There is no default port. A port this product chose would be the same port on
/// every machine running it, and a port nobody chose is one something else may
/// already hold — so zero is asked for, which means any free one, and whatever was
/// given is printed in full. Where no port was named the first address taken settles
/// it and the rest are asked for the same one, so what an operator is told is one
/// port on however many families answered.
///
/// IPv6 first, deliberately. A wildcard on that family answers for IPv4 as well on
/// some machines and not on others, and asking for it first means the machines where
/// it does are covered by one socket rather than by a second bind that then has to be
/// explained away.
pub(crate) const fn wanted(offered: Offered, port: Option<u16>) -> [SocketAddr; 2] {
    let port = match port {
        Some(port) => port,
        None => 0,
    };
    match offered {
        Offered::Network => [
            SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), port),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), port),
        ],
        // A refusal never reaches a socket; naming this machine's own addresses for
        // it keeps the shape of this function one thing rather than two.
        Offered::Machine | Offered::Refused => [
            SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), port),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port),
        ],
    }
}

/// Take every address of a tier that can be taken, and say which they were.
///
/// A family this machine has no stack for, and a family already answered by the
/// socket before it, come to the same thing: one fewer socket, and nothing to
/// report. What is a failure is taking **none** of them, which is the case where
/// there is nowhere for a browser to connect.
///
/// # Errors
///
/// Returns the [`Problem`] to report where not one address could be taken. Boxed
/// because a problem carries what happened, what it means and what to do about it,
/// and a result that carries all of that inline on the way that succeeds is paying
/// for the failure on every call.
pub(crate) async fn held(
    offered: Offered,
    port: Option<u16>,
) -> Result<Vec<(TcpListener, SocketAddr)>, Box<Problem>> {
    let asked = wanted(offered, port);
    let mut taken: Vec<(TcpListener, SocketAddr)> = Vec::new();
    let mut refused = String::new();
    for address in asked {
        // Whatever the first one settled, since a port nobody named is a port only
        // the operating system knows until something holds it.
        let at = taken.first().map_or(address, |(_, bound)| {
            SocketAddr::new(address.ip(), bound.port())
        });
        // Bound and named in one step, so there is one way for this to fail rather
        // than two, only one of which anything could provoke — and so the way it
        // cannot fail leaves behind no arm a test would have to reach.
        let bound = TcpListener::bind(at)
            .await
            .and_then(|listener| listener.local_addr().map(|bound| (listener, bound)));
        match bound {
            Ok(held) => taken.push(held),
            Err(err) => refused = err.to_string(),
        }
    }
    if taken.is_empty() {
        return Err(Box::new(unavailable(asked, &refused)));
    }
    Ok(taken)
}

/// The address as it is printed, and as it is typed into a browser.
pub(crate) fn address(bound: SocketAddr) -> String {
    format!("http://{bound}")
}

/// Not one of the addresses could be taken.
fn unavailable(asked: [SocketAddr; 2], reason: &str) -> Problem {
    let named = asked
        .iter()
        .map(SocketAddr::to_string)
        .collect::<Vec<String>>()
        .join(" nor ");
    let asked = match asked.first().map(SocketAddr::port) {
        Some(0) | None => "no free port could be taken on this machine".to_owned(),
        Some(_) => format!("neither {named} could be taken"),
    };
    Problem::new(
        ADDRESS_TAKEN,
        Severity::Error,
        format!("{PRODUCT} could not start serving: {asked}"),
        "Usually something else on this machine is already listening there. Whatever the \
         reason, there is nowhere for a browser to connect, and the words below are the \
         operating system's own.",
        Remedy::new("Ask for a different port").with_detail(format!("{PRODUCT} ui --port 7171")),
    )
    .or_try(Remedy::new(
        "Or name no port and be given whichever one is free",
    ))
    .in_state(State::Guided)
    .with_detail(reason.to_owned())
}

/// The network was asked for and nothing here can say who is knocking.
///
/// Refused rather than warned about, and refused rather than quietly served on this
/// machine instead: an operator who asked for the network and was given loopback
/// would find out from a device that could not connect, which is a worse way to learn
/// it than being told now.
pub(crate) fn unauthenticated() -> Problem {
    Problem::new(
        NO_PASSWORD,
        Severity::Error,
        format!("{PRODUCT} will not offer this to your network without a password"),
        "This surface can start, stop and reconfigure everything and reaches every password \
         the system holds. Offered to a network with nothing in front of it, anything on that \
         network can do all of that.",
        Remedy::new("Set a password, then ask again")
            .with_detail(format!("{PRODUCT} ui --set-password --lan")),
    )
    .or_try(Remedy::new(
        "Or leave it as it is, and reach it from this machine",
    ))
    .in_state(State::Guided)
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use super::{address, held, permitted, unauthenticated, unavailable, wanted, Offered, Reach};

    /// Every reading of the policy, written out.
    ///
    /// As the whole table rather than as the two cases that are interesting, because
    /// what makes this a policy rather than a condition is that there is an answer
    /// for each of them and only one of them is a refusal.
    #[test]
    fn the_policy_refuses_one_of_its_four_readings_and_no_other() {
        assert_eq!(permitted(Reach::Machine, false), Offered::Machine);
        assert_eq!(permitted(Reach::Machine, true), Offered::Machine);
        assert_eq!(permitted(Reach::Network, true), Offered::Network);
        assert_eq!(permitted(Reach::Network, false), Offered::Refused);
    }

    /// Asking for nothing in particular is this machine.
    #[test]
    fn what_is_asked_for_where_nothing_says_otherwise_is_this_machine() {
        assert_eq!(Reach::default(), Reach::Machine);
    }

    /// A tier names one address on each family, so a policy cannot be enforced on
    /// one of them and be silently absent on the other.
    #[test]
    fn each_tier_names_an_address_on_both_families() {
        for offered in [Offered::Machine, Offered::Network] {
            let named = wanted(offered, None);
            assert!(named.iter().any(SocketAddr::is_ipv6), "{offered:?}");
            assert!(named.iter().any(SocketAddr::is_ipv4), "{offered:?}");
        }
    }

    /// This machine's tier names only addresses this machine answers, and the
    /// network's names only ones that are not.
    #[test]
    fn the_addresses_a_tier_names_are_the_ones_that_tier_means() {
        assert!(wanted(Offered::Machine, None)
            .iter()
            .all(|address| address.ip().is_loopback()));
        assert!(wanted(Offered::Refused, None)
            .iter()
            .all(|address| address.ip().is_loopback()));
        assert!(wanted(Offered::Network, None)
            .iter()
            .all(|address| address.ip().is_unspecified()));
    }

    #[test]
    fn naming_no_port_asks_for_whichever_one_is_free() {
        assert!(wanted(Offered::Machine, None)
            .iter()
            .all(|address| address.port() == 0));
    }

    #[test]
    fn naming_a_port_asks_for_that_one_on_every_family() {
        assert!(wanted(Offered::Machine, Some(7171))
            .iter()
            .all(|address| address.port() == 7171));
    }

    #[tokio::test]
    async fn every_address_taken_is_this_machine_and_they_share_one_port() {
        // Asserted as one value rather than through a branch on the way in: this
        // module's tests are under the same coverage gate as the code, and an arm
        // for a bind that never fails is a line nothing could ever run.
        let taken = held(Offered::Machine, None).await.ok();
        let at: Vec<SocketAddr> = taken.iter().flatten().map(|(_, bound)| *bound).collect();
        let first = at.first().map_or(0, SocketAddr::port);
        assert!(!at.is_empty(), "not one address could be taken");
        assert!(at.iter().all(|bound| bound.ip().is_loopback()), "{at:?}");
        assert!(at.iter().all(|bound| bound.port() == first), "{at:?}");
        assert_ne!(first, 0, "a port nobody named is one the machine settled");
    }

    #[tokio::test]
    async fn an_address_no_family_can_take_is_reported_rather_than_swapped() {
        // A port this run already holds, asked for again by name. Both families are
        // asked for it and neither can have it, which is the one case that is a
        // failure rather than one fewer socket.
        let taken = held(Offered::Machine, None).await.ok();
        let port = taken.iter().flatten().next().map(|(_, bound)| bound.port());
        let again = held(Offered::Machine, port).await;
        assert!(
            again
                .err()
                .is_some_and(|problem| problem.summary.contains("could not start serving")),
            "a port this run is holding is not one it can take again"
        );
        drop(taken);
    }

    #[test]
    fn a_machine_with_no_free_port_at_all_says_that_instead() {
        // The other half of the same fault: asking for any port and being given none
        // says something different from being refused a named one.
        let any = unavailable(wanted(Offered::Machine, None), "denied").summary;
        let named = unavailable(wanted(Offered::Machine, Some(7171)), "denied").summary;
        assert!(any.contains("no free port"), "{any}");
        assert!(named.contains("127.0.0.1:7171"), "{named}");
        assert!(named.contains("[::1]:7171"), "{named}");
    }

    #[test]
    fn a_refusal_to_take_an_address_offers_both_ways_out() {
        // Ask for another port, or stop asking for one in particular.
        let problem = unavailable(wanted(Offered::Machine, Some(7171)), "address in use");
        assert_eq!(problem.remedies.len(), 2);
        assert_eq!(problem.detail.as_deref(), Some("address in use"));
    }

    /// The network without a password is refused, and the refusal says how.
    ///
    /// Both ways out, because there are two: set one, or stop asking for the network.
    /// A refusal naming neither is a refusal somebody has to go and look things up
    /// after.
    #[test]
    fn refusing_the_network_says_how_to_be_allowed_it() {
        let problem = unauthenticated();
        assert_eq!(problem.remedies.len(), 2);
        assert!(
            problem.remedies.iter().any(|remedy| remedy
                .detail
                .as_deref()
                .is_some_and(|said| said.contains("--set-password"))),
            "{problem:?}"
        );
    }

    #[test]
    fn the_address_is_printed_whole() {
        assert_eq!(
            address(SocketAddr::from(([127, 0, 0, 1], 8471))),
            "http://127.0.0.1:8471"
        );
    }
}
