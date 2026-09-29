//! Where handing somebody's device over stands, and what to hold up to it.
//!
//! The shape is the answer first: whose device, and where it stands. Beneath that is
//! whatever the state needs — a code and the steps while the device has yet to sign in,
//! the devices signed in once one has, and the reason where nothing could be handed
//! over.
//!
//! The code is the address again, for a camera. It signs nobody in, so it is drawn
//! without a warning about who might see it.

use lemonfiber_core::model::{Handoff, HandoffState};

use super::super::{qr, Lines};
use crate::say;

/// What an operator is told about handing one person's device over.
pub(crate) fn handoff(report: &Handoff) -> Lines {
    let mut lines = Lines::default();
    if report.rehearsed && report.state == HandoffState::Ready {
        // First, because everything under it reads as a code that was handed over.
        lines.put("Nothing was written down. This is what would be handed over:".to_owned());
    }
    lines.put(headline(report));
    if let Some(reason) = &report.reason {
        lines.put(format!("  {reason}"));
    }
    match (report.state, &report.address) {
        (HandoffState::Ready | HandoffState::Pending, Some(address)) => {
            handing(&mut lines, report, address);
        }
        (HandoffState::Connected, _) => signed_in(&mut lines, report),
        _ => {}
    }
    lines
}

/// The sentence the answer opens on.
fn headline(report: &Handoff) -> String {
    let name = &report.name;
    match report.state {
        HandoffState::Unprovisioned => format!("{name} has no account yet"),
        HandoffState::Ready => {
            format!("{name}'s code is ready — scan it on the device they will watch on")
        }
        HandoffState::Pending => {
            format!("{name} has the code and has not signed in on a device yet")
        }
        HandoffState::Connected => match report.sessions.len() {
            1 => format!("{name} is signed in on one device"),
            many => format!("{name} is signed in on {many} devices"),
        },
        HandoffState::Failed => format!("{name}'s device could not be handed over"),
    }
}

/// The address, its code, the apps to point at it, and the steps.
fn handing(lines: &mut Lines, report: &Handoff, address: &str) {
    lines.spaced(format!("  {address}"));
    if let Some(caution) = &report.caution {
        lines.put(format!("  {caution}"));
    }
    if let Some(drawn) = qr::rows(address, say::folding()) {
        lines.spaced("Point the device's camera, or the app's scanner, at this:");
        for row in drawn {
            lines.put(format!("  {row}"));
        }
    }
    lines.spaced("Then:");
    for (number, step) in report.steps.iter().enumerate() {
        lines.put(format!("  {}. {step}", number + 1));
    }
    lines.spaced("Which app, on which device:");
    for client in &report.clients {
        let closed = if client.open_source {
            ""
        } else {
            " (not open source)"
        };
        lines.put(format!("  {}   {}{closed}", client.device, client.client));
    }
}

/// The devices the media server lists as signed in.
fn signed_in(lines: &mut Lines, report: &Handoff) {
    lines.spaced("Signed in now, as the media server lists them:");
    for session in &report.sessions {
        let seen = session
            .last_seen
            .as_deref()
            .map(|at| format!(", last heard from {at}"))
            .unwrap_or_default();
        lines.put(format!("  {} — {}{seen}", session.device, session.client));
    }
}

#[cfg(test)]
mod tests;
