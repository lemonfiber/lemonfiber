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
//! and a source is refused where any address it stands for is loopback, private,
//! link-local, unique-local or unspecified, an IPv4 address carried inside an IPv6 one
//! included.
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
use crate::error::{Problem, Remedy, Severity, State};
use crate::plugin::SPOKEN;

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
    let Some((host, port)) = host_of(url) else {
        return Err(Box::new(super::fetching::unfetched(
            url,
            "it names no host",
        )));
    };
    if let Ok(address) = host.parse::<IpAddr>() {
        return refusing(url, &host, &[address]).map(|()| Reached { pin: None });
    }
    let addresses = ctx
        .seams
        .resolver
        .addresses(&host, port)
        .await
        .map_err(|why| Box::new(super::fetching::unfetched(url, &why)))?;
    if addresses.is_empty() {
        let why = format!("{host} stands for no address");
        return Err(Box::new(super::fetching::unfetched(url, &why)));
    }
    refusing(url, &host, &addresses)?;
    Ok(Reached {
        pin: Some(pinned(&host, port, &addresses)),
    })
}

/// Refuse where any of these addresses is one a source may not stand for.
fn refusing(url: &str, host: &str, addresses: &[IpAddr]) -> Result<(), Box<Problem>> {
    match addresses.iter().find(|address| refused(**address)) {
        Some(address) => Err(Box::new(address_refused(url, host, *address))),
        None => Ok(()),
    }
}

/// Whether an address is this machine, a network of its own, or no address at all.
fn refused(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(v4) => {
            v4.is_loopback() || v4.is_private() || v4.is_link_local() || v4.is_unspecified()
        }
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map_or_else(
            || {
                let first = v6.segments()[0];
                v6.is_loopback()
                    || v6.is_unspecified()
                    // fc00::/7, an address a network gives itself.
                    || first & 0xfe00 == 0xfc00
                    // fe80::/10, an address that reaches no further than the link.
                    || first & 0xffc0 == 0xfe80
            },
            |v4| refused(IpAddr::V4(v4)),
        ),
    }
}

/// The host an https address names and the port it is reached on.
///
/// Read off what follows the scheme up to the first `/`, `?` or `#`, past anything a
/// `user@` puts in front of it, with an IPv6 address in its brackets.
fn host_of(url: &str) -> Option<(String, u16)> {
    let rest = url.strip_prefix(SPOKEN)?;
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let authority = authority
        .rsplit_once('@')
        .map_or(authority, |(_, after)| after);
    let (host, port) = match authority.strip_prefix('[') {
        Some(bracketed) => {
            let (host, after) = bracketed.split_once(']')?;
            (host, after.strip_prefix(':'))
        }
        None => match authority.rsplit_once(':') {
            Some((host, port)) => (host, Some(port)),
            None => (authority, None),
        },
    };
    let port = match port {
        Some(written) => written.parse().ok()?,
        None => crate::schemes::SECURE,
    };
    (!host.is_empty()).then(|| (host.to_owned(), port))
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
        Severity::Error,
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
fn address_refused(url: &str, host: &str, address: IpAddr) -> Problem {
    Problem::new(
        ADDRESS_REFUSED,
        Severity::Error,
        format!("{url} is not out on the internet"),
        format!(
            "Nothing was fetched and nothing was installed. {host} stands for {address}, \
             which is this machine or a network of its own, and a plugin is fetched only \
             from somewhere every machine could reach."
        ),
        Remedy::new("Install it from a directory on this machine, or from a public https address"),
    )
    .in_state(State::Guided)
}

#[cfg(test)]
mod tests;
