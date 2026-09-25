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

use lemonfiber_core::error::{Problem, Remedy, Severity, State};
use lemonfiber_core::PRODUCT;
use tokio::net::TcpListener;

use lemonfiber_core::error::codes::serve::ADDRESS_TAKEN;

use lemonfiber_core::error::codes::serve::NO_PASSWORD;

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

/// What may be reached, given what was asked for and whether a password guards it.
///
/// The whole of the policy, in one place, so the moment before a socket exists and
/// the moment after it does are reading the same rule rather than two that agree
/// today.
pub(crate) const fn permitted(asked: Reach, guarded: bool) -> Offered {
    match (asked, guarded) {
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
mod tests;
