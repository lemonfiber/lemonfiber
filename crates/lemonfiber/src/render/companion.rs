//! What a phone is handed, and what replacing the certificate it pins costs.
//!
//! The code comes first, because the operator is holding the phone: everything under it
//! is what to check and what to know, and none of it has to be read to pair.

use lemonfiber_core::companion::{Pairing, Replacement, LASTS};
use lemonfiber_core::PRODUCT;

use super::{qr, Lines};
use crate::say;

/// What an operator is shown to pair a phone.
pub(super) fn pairing(report: &Pairing) -> Lines {
    let mut lines = Lines::default();
    lines.put(
        "Point the phone's camera at this, or type the line under it where it has none:".to_owned(),
    );
    for row in qr::rows(&report.written, say::folding())
        .into_iter()
        .flatten()
    {
        lines.put(format!("  {row}"));
    }
    lines.put(format!("  {}", report.written));
    lines.spaced(format!(
        "It reaches this stack at {}, which presents the certificate",
        report.material.address
    ));
    lines.put(format!("  {}", report.material.fingerprint));
    if let Some(caution) = &report.caution {
        lines.put(format!("  {caution}"));
    }
    lines.put(format!(
        "It stops being good at {} UTC, {} minutes from now.",
        report.until,
        LASTS.as_secs() / 60
    ));
    lines.spaced(format!(
        "It holds no password, and scanning it lets nobody in: the phone still signs in \
         with the password this surface asks for. {}",
        report.replacing
    ));
    lines
}

/// What replacing the certificate came to, or what it would cost.
pub(super) fn certificate(report: &Replacement) -> Lines {
    let mut lines = Lines::default();
    if report.replaced {
        lines.put("The certificate was replaced.".to_owned());
        lines.put(report.consequence.clone());
    } else {
        lines.put("Nothing was replaced.".to_owned());
        lines.put(format!("Replacing it: {}", report.consequence));
    }
    match (&report.fingerprint, report.replaced) {
        (Some(fingerprint), true) => {
            lines.spaced("A phone paired from now on pins".to_owned());
            lines.put(format!("  {fingerprint}"));
        }
        (Some(fingerprint), false) => {
            lines.spaced("Paired phones pin".to_owned());
            lines.put(format!("  {fingerprint}"));
            lines.put(format!(
                "Replace it with `{PRODUCT} companion certificate --confirm`."
            ));
        }
        (None, _) => lines.spaced(format!(
            "None has been made yet. One is made the first time the web interface is served \
             with `{PRODUCT} ui --lan --tls --port <port>`."
        )),
    }
    lines
}

#[cfg(test)]
mod tests;
