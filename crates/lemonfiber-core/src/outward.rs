//! Whether a host stands for somewhere out on the internet, and nowhere closer.
//!
//! Anything lemonfiber reaches on the strength of a name somebody else wrote — a git
//! source a plugin is fetched from, a host a recipe calls — is held to one rule. A name
//! can stand for this machine, the network it sits on, or a cloud's metadata service,
//! and reaching one of those is reaching something nobody meant to expose by asking
//! lemonfiber to do it. So the host is resolved once, before anything is sent, and
//! refused where any address it stands for is not out on the internet, as [`internal`]
//! classes addresses.
//!
//! What passed is what the caller connects to. [`checked`] puts the addresses on the
//! request, the transport connects to them and to nothing else, and a name that stands
//! for somewhere else a moment later is not asked again.
//!
//! An address written as the host is read as the address it is however it is spelled
//! ([`literal`]), because a resolver or a URL parser handed `127.1` or `0x7f000001`
//! reads it as an address too, and a spelling this did not recognise would pass as a
//! name.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use url::Url;

use crate::ports::http::Request;
use crate::ports::resolve::Resolver;
use crate::schemes::{PLAIN, SECURE};

mod classes;

pub use classes::internal;

/// Why a host is not one to reach.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Inward {
    /// The address names no host to check.
    Nameless,
    /// The resolver could not say what the name stands for, in its own words.
    Unresolved(String),
    /// The name stands for no address at all.
    Nowhere,
    /// The host stands for this address, which is not out on the internet.
    Internal(IpAddr),
}

/// Every address `host` stands for when reached on `port`, each out on the internet.
///
/// An address written as the host is judged as it is written, and the resolver is not
/// asked.
///
/// # Errors
///
/// Where the name stands for nothing, the resolver could not answer, or any address it
/// stands for is one this refuses — even beside an address out there.
pub async fn outward(
    resolver: &dyn Resolver,
    host: &str,
    port: u16,
) -> Result<Vec<IpAddr>, Inward> {
    let addresses = match literal(host) {
        Some(address) => vec![address],
        None => resolver
            .addresses(host, port)
            .await
            .map_err(Inward::Unresolved)?,
    };
    if addresses.is_empty() {
        return Err(Inward::Nowhere);
    }
    match addresses.iter().find(|address| internal(**address)) {
        Some(address) => Err(Inward::Internal(*address)),
        None => Ok(addresses),
    }
}

/// The request, held to the addresses its host was just checked against.
///
/// Asked once per call, so every call checks the name anew and connects only to what
/// that check passed. The host is the one the address names as the transport reads it.
///
/// # Errors
///
/// Where the address names no host, or [`outward`] refuses the one it names.
pub async fn checked(resolver: &dyn Resolver, request: Request) -> Result<Request, Inward> {
    // Read with the parser the transport reads it with, so the name checked is the name
    // the call connects as: two readers of one address that disagree about its host are
    // a check of one host and a call to another.
    let url = Url::parse(&request.url).map_err(|_| Inward::Nameless)?;
    let (Some(host), Some(port)) = (url.host_str(), url.port_or_known_default()) else {
        return Err(Inward::Nameless);
    };
    let addresses = outward(resolver, host, port).await?;
    Ok(Request {
        pinned: Some(addresses),
        ..request
    })
}

/// The host an http or https address names, and every address it stands for, each out
/// on the internet.
///
/// # Errors
///
/// Where the address names no host, or [`outward`] refuses the one it names.
pub async fn located(
    resolver: &dyn Resolver,
    url: &str,
) -> Result<((String, u16), Vec<IpAddr>), Inward> {
    let (host, port) = host_of(url).ok_or(Inward::Nameless)?;
    let addresses = outward(resolver, &host, port).await?;
    Ok(((host, port), addresses))
}

/// The host an http or https address names and the port it is reached on.
///
/// Read off what follows the scheme up to the first `/`, `?` or `#`, past anything a
/// `user@` puts in front of it, with an IPv6 address in its brackets.
#[must_use]
pub fn host_of(url: &str) -> Option<(String, u16)> {
    let (rest, default) = url
        .strip_prefix("https://")
        .map(|rest| (rest, SECURE))
        .or_else(|| url.strip_prefix("http://").map(|rest| (rest, PLAIN)))?;
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
        None => default,
    };
    (!host.is_empty()).then(|| (host.to_owned(), port))
}

/// The address `host` is, however it is written, or nothing where it is a name.
///
/// IPv6 as the standard text form writes it, brackets or not; IPv4 as dotted decimal,
/// and as every older form a resolver still reads as an address: one to four parts,
/// each decimal, octal after a leading `0` or hexadecimal after `0x`, the last filling
/// whatever bytes the parts before it leave.
#[must_use]
pub fn literal(host: &str) -> Option<IpAddr> {
    let bare = host
        .strip_prefix('[')
        .and_then(|inner| inner.strip_suffix(']'))
        .unwrap_or(host);
    bare.parse::<Ipv6Addr>()
        .map(IpAddr::V6)
        .ok()
        .or_else(|| older(bare).map(IpAddr::V4))
}

/// An IPv4 address in any of the forms a resolver reads as one.
fn older(host: &str) -> Option<Ipv4Addr> {
    let parts: Vec<u32> = host.split('.').map(number).collect::<Option<_>>()?;
    let (last, leading) = parts.split_last()?;
    if leading.len() > 3 || leading.iter().any(|part| *part > 0xff) {
        return None;
    }
    // What the last part fills: every byte the parts before it leave, so `127.1` is
    // `127.0.0.1` and a single number is the whole address.
    let room = 8 * (4 - u32::try_from(leading.len()).ok()?);
    if room < 32 && *last >> room != 0 {
        return None;
    }
    let high = leading
        .iter()
        .zip([24, 16, 8])
        .fold(0_u32, |all, (part, shift)| all | part << shift);
    Some(Ipv4Addr::from(high | last))
}

/// One part of an older IPv4 form: decimal, octal after a leading `0`, or hexadecimal
/// after `0x`.
fn number(part: &str) -> Option<u32> {
    let (digits, radix) = match part.as_bytes() {
        [b'0', b'x' | b'X', ..] => (&part[2..], 16),
        [b'0', _, ..] => (&part[1..], 8),
        _ => (part, 10),
    };
    if digits.is_empty() || !digits.chars().all(|one| one.is_digit(radix)) {
        return None;
    }
    u32::from_str_radix(digits, radix).ok()
}

#[cfg(test)]
mod tests;
