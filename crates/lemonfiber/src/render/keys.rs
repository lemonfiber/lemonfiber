//! A key minted, and the keys listed, on a terminal.
//!
//! The secret comes first and once, because it is the one thing on the screen that is
//! never shown again: everything under it is how to use it and what to know.

use lemonfiber_core::keys::{Listed, Listing, Minted, State};

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
    let used = key.used.as_deref().map_or_else(
        || "never used".to_owned(),
        |used| format!("last used {used}"),
    );
    lines.put(format!("    minted {}, {used}", key.minted));
    if let Some(revoked) = key.revoked.as_deref() {
        lines.put(format!("    revoked {revoked}"));
    }
    lines
}

/// Where a key stands, in the words a listing uses.
const fn state(state: State) -> &'static str {
    match state {
        State::Active => "active",
        State::Revoked => "revoked",
        State::Orphaned => "orphaned: the account it is for has left the household",
        State::Unconfirmed => "the media server could not be asked whether its account remains",
    }
}

#[cfg(test)]
mod tests;
