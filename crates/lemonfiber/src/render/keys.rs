//! A key minted, the keys listed, and the device keys a phone proves the operator with,
//! on a terminal.
//!
//! The secret comes first and once, because it is the one thing on the screen that is
//! never shown again: everything under it is how to use it and what to know.

use lemonfiber_core::device_keys;
use lemonfiber_core::keys::{Claimed, Listed, Listing, Minted, State};
use lemonfiber_core::push;

use super::{qr, Lines};

/// A key minted, with its secret, the address and the pin a client elsewhere needs.
pub(super) fn minted(report: &Minted) -> Lines {
    let mut lines = Lines::default();
    lines.put(format!(
        "Minted the key {}, with the scope {}, for {}.",
        report.name,
        report.scope,
        report.purpose.written()
    ));
    lines.spaced("Its secret, which is not shown again:");
    lines.put(format!("  {}", report.secret.as_str()));
    qr::labelled(
        &mut lines,
        "The same secret, for a camera:",
        report.secret.as_str(),
    );
    lines.spaced(format!(
        "A program sends it in the {} header.",
        lemonfiber_api::guard::TOKEN_HEADER
    ));
    if let Some(address) = report.address.as_deref() {
        lines.put(format!("  address  {address}"));
    }
    if let Some(pin) = report.pin.as_deref() {
        lines.put(format!("  pin      {pin}"));
    }
    if let Some(caution) = report.caution.as_deref() {
        lines.spaced(caution);
    }
    lines
}

/// A key minted as a claim: the link and its code, which collect the key once.
pub(super) fn claimed(report: &Claimed) -> Lines {
    let mut lines = Lines::default();
    lines.put(format!(
        "Minted the key {} as a claim, with the scope {}, for {}.",
        report.name,
        report.scope,
        report.purpose.written()
    ));
    lines.spaced("Its link, which is not shown again:");
    lines.put(format!("  {}", report.link));
    qr::labelled(&mut lines, "The same link, for a camera:", &report.link);
    lines.spaced(format!(
        "The program collects the key with it once, until {}. A key nobody collects by then \
         is revoked, and collecting it is told to you.",
        report.lapses.said()
    ));
    lines
}

/// Every key, without its secret, and what became of a revoke.
pub(super) fn listing(report: &Listing) -> Lines {
    let mut lines = Lines::default();
    if let Some(name) = report.revoked.as_deref() {
        lines.put(if report.rehearsed {
            format!("Revoking would refuse the key {name} from its next request.")
        } else {
            format!("Revoked the key {name}. It is refused from its next request.")
        });
        lines.put(String::new());
    }
    if report.keys.is_empty() {
        lines.put("No key has been minted on this machine.");
        return lines;
    }
    lines.put("Keys:");
    for key in &report.keys {
        lines.extend(one(key));
    }
    lines.spaced(report.purposes.as_str());
    lines
}

/// One key, as the listing shows it.
fn one(key: &Listed) -> Lines {
    let mut lines = Lines::default();
    let minted_by = if key.member_minted {
        ", minted by the member"
    } else {
        ""
    };
    lines.put(format!(
        "  {}  {}  {}  {}{minted_by}",
        key.name,
        key.scope,
        key.purpose.written(),
        state(key.state)
    ));
    let used = key.used.map_or_else(
        || "never used".to_owned(),
        |used| format!("last used {}", used.said()),
    );
    lines.put(format!("    minted {}, {used}", key.minted.said()));
    if let Some(device) = key.proved_by.as_deref() {
        lines.put(format!("    proved by the device key {device}"));
    }
    if let Some(revoked) = key.revoked {
        lines.put(format!("    revoked {}", revoked.said()));
    }
    lines
}

/// Where a key stands, in the words a listing uses.
const fn state(state: State) -> &'static str {
    match state {
        State::Active => "active",
        State::Waiting => "waiting to be collected",
        State::Revoked => "revoked",
        State::Orphaned => "orphaned: the account it is for has left the household",
        State::Unconfirmed => "the media server could not be asked whether its account remains",
    }
}

/// A device key registered.
pub(super) fn registered(report: &device_keys::Registered) -> Lines {
    let mut lines = Lines::default();
    lines.put(format!(
        "Registered the device key {}, held in {}, made by {}.",
        report.name,
        report.backing.written(),
        report.app
    ));
    lines.put(format!("  fingerprint  {}", report.fingerprint));
    lines
}

/// Every device key, and what became of a revoke.
pub(super) fn devices(report: &device_keys::Listing) -> Lines {
    let mut lines = Lines::default();
    if let Some(name) = report.revoked.as_deref() {
        lines.put(if report.rehearsed {
            format!("Revoking would refuse the device key {name} at its next challenge.")
        } else {
            format!("Revoked the device key {name}. It proves nothing from now on.")
        });
        lines.put(String::new());
    }
    if report.keys.is_empty() {
        lines.put("No device key has been registered on this machine.");
        return lines;
    }
    lines.put("Device keys:");
    for key in &report.keys {
        let used = key.used.map_or_else(
            || "never used".to_owned(),
            |used| format!("last used {}", used.said()),
        );
        let standing = key.revoked.map_or_else(
            || "active".to_owned(),
            |at| format!("revoked {}", at.said()),
        );
        lines.put(format!(
            "  {}  {}  {}  registered {}  {used}  {standing}",
            key.name,
            key.backing.written(),
            key.app,
            key.registered.said()
        ));
        lines.put(format!("    fingerprint  {}", key.fingerprint));
    }
    lines
}

/// A push endpoint registered.
pub(super) fn pushed(report: &push::Registered) -> Lines {
    let mut lines = Lines::default();
    lines.put(if report.rehearsed {
        format!("Registering would wake the push endpoint {}.", report.name)
    } else {
        format!(
            "Registered the push endpoint {}. It is woken when this stack has something to say.",
            report.name
        )
    });
    lines
}

/// Every push endpoint, and what became of a removal.
pub(super) fn pushes(report: &push::Listing) -> Lines {
    let mut lines = Lines::default();
    if let Some(name) = report.removed.as_deref() {
        lines.put(if report.rehearsed {
            format!("Removing would stop waking the push endpoint {name}.")
        } else {
            format!("Removed the push endpoint {name}. Its phone is not woken from now on.")
        });
        lines.put(String::new());
    }
    if report.endpoints.is_empty() {
        lines.put("No push endpoint has been registered on this machine.");
        return lines;
    }
    lines.put("Push endpoints:");
    for one in &report.endpoints {
        lines.put(format!(
            "  {}  {}  registered {}",
            one.name,
            one.endpoint,
            one.registered.said()
        ));
    }
    lines
}

#[cfg(test)]
mod tests;
