//! Where a git source may be fetched from: over https, from a host that stands for an
//! address out on the internet and nowhere closer.
//!
//! **Over https, or not at all.** Every other transport git speaks can be read or
//! rewritten on the way, and a branch or a tag fetched over one is whatever somebody in
//! between chose. An address written with another scheme is refused by name before
//! anything is asked of it.
//!
//! **Out there, not in here.** A source is something the operator, or a client of the
//! web API, names, and a name can stand for this machine, the network it sits on, or a
//! cloud's metadata service. Fetching from one of those is reaching something nobody
//! meant to expose by asking git to do it. So the host is resolved before git is run,
//! and held to the rule [`crate::outward`] holds every such name to.
//!
//! **Git connects to what was checked.** The addresses that passed are handed to git as
//! the only answer for that host, so a name that stands for somewhere else a moment
//! later is not asked again. Git older than 2.37 does not read that setting and asks
//! the resolver itself, so on such a git the check and the fetch are two lookups. Git
//! follows no redirect from any source, so the host checked is the host fetched from.
//!
//! Asked when a source is fetched for an install or an update, and never when what is
//! installed is listed: a listing asks each recorded source whether it still answers,
//! and asks nothing more of the network to say so.

use std::net::IpAddr;

use crate::app::Ctx;
use crate::error::codes::plugin::{ADDRESS_REFUSED, SCHEME_REFUSED};
use crate::error::{Problem, Remedy, State};
use crate::outward::{literal, located, Inward};

/// The git setting the checked addresses are handed over in.
const RESOLVE: &str = "http.curloptResolve";

/// A source that may be fetched, and the setting that holds git to what was checked.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Reached {
    /// `http.curloptResolve=…`, where the host is a name; nothing where it is an
    /// address, which git connects to as it is written.
    pin: Option<String>,
}

impl Reached {
    /// The setting git is run with for this source, where there is one.
    pub(super) fn pin(&self) -> Option<&str> {
        self.pin.as_deref()
    }
}

/// Hold an https source to a host out on the internet.
///
/// Its scheme is [`scheme_refused`]'s question, asked before this one, because a source
/// written with another is refused before anything is asked, a setting included.
///
/// # Errors
///
/// Where its host stands for an address this refuses, and where it names no host or
/// the host stands for nothing.
pub(super) async fn reached(ctx: &Ctx, url: &str) -> Result<Reached, Box<Problem>> {
    let ((host, port), addresses) = located(ctx.seams.resolver.as_ref(), url)
        .await
        .map_err(|inward| Box::new(refused(url, inward)))?;
    // An address written as the host is what git connects to as it is written.
    let pin = literal(&host)
        .is_none()
        .then(|| pinned(&host, port, &addresses));
    Ok(Reached { pin })
}

/// What a host that is not out on the internet comes to for a source.
fn refused(url: &str, inward: Inward) -> Problem {
    match inward {
        Inward::Nameless => super::fetching::unfetched(url, "it names no host"),
        Inward::Unresolved(why) => super::fetching::unfetched(url, &why),
        Inward::Nowhere => super::fetching::unfetched(url, "it stands for no address"),
        Inward::Internal(address) => address_refused(url, address),
    }
}

/// The setting that hands git these addresses as the only answer for `host`.
fn pinned(host: &str, port: u16, addresses: &[IpAddr]) -> String {
    let written: Vec<String> = addresses
        .iter()
        .map(|address| match address {
            IpAddr::V4(v4) => v4.to_string(),
            IpAddr::V6(v6) => format!("[{v6}]"),
        })
        .collect();
    format!("{RESOLVE}={host}:{port}:{}", written.join(","))
}

/// Said where a source is written with a scheme other than https.
pub(super) fn scheme_refused(url: &str, scheme: &str) -> Problem {
    Problem::new(
        SCHEME_REFUSED,
        format!("{url} is not fetched over https"),
        format!(
            "Nothing was asked of it and nothing was installed. It begins with {scheme}, and \
             a source fetched over anything but https can be read or rewritten on its way \
             here."
        ),
        Remedy::new("Name the same repository by its https address"),
    )
    .in_state(State::Guided)
}

/// Said where a source's host stands for an address this refuses.
fn address_refused(url: &str, address: IpAddr) -> Problem {
    Problem::new(
        ADDRESS_REFUSED,
        format!("{url} is not out on the internet"),
        format!(
            "Nothing was fetched and nothing was installed. Its host stands for {address}, \
             which is this machine, a network of its own or a range nothing out on the \
             internet answers on, and a plugin is fetched only from somewhere every machine \
             could reach."
        ),
        Remedy::new("Install it from a directory on this machine, or from a public https address"),
    )
    .in_state(State::Guided)
}

#[cfg(test)]
mod tests;
