//! What a phone is handed, and what replacing the certificate it pins costs.
//!
//! The code comes first, because the operator is holding the phone: everything under it
//! is what to check and what to know, and none of it has to be read to pair.

use lemonfiber_core::app::standing::StackStanding;
use lemonfiber_core::app::surfacing::Answer;
use lemonfiber_core::companion::served::ServedReport;
use lemonfiber_core::companion::{Pairing, Replacement, LASTS};
use lemonfiber_core::notify::events::Heard;
use lemonfiber_core::screening::{Renewed, Untuned};
use lemonfiber_core::PRODUCT;
use lemonfiber_error::plural::s;

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
    lines.put(format!(
        "  A phone the line is typed into shows {}; pair it only if it does.",
        report.compare
    ));
    if let Some(caution) = &report.caution {
        lines.put(format!("  {caution}"));
    }
    lines.put(format!(
        "It stops being good at {}, {} minutes from now.",
        report.until.said(),
        LASTS.as_secs() / 60
    ));
    lines.spaced(format!(
        "It holds no password, and scanning it lets nobody in: the phone still signs in \
         with the password this surface asks for. {}",
        report.replacing
    ));
    lines.put(format!(
        "Replacing it is `{PRODUCT} companion certificate --confirm`."
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

/// The name the web surface is served under, and the certificate it presents where it
/// has one.
pub(super) fn served(report: &ServedReport) -> Lines {
    let mut lines = Lines::default();
    lines.put(report.named.as_ref().map_or_else(
        || "Served under no name.".to_owned(),
        |name| format!("Served as {name}."),
    ));
    if let Some(certificate) = &report.certificate {
        lines.put(format!(
            "Certificate fingerprint: {}",
            certificate.fingerprint
        ));
        lines.put(format!(
            "Made out to {}{}, for {}{}.",
            certificate.subject,
            cut(certificate.subject_cut),
            certificate.names.join(", "),
            left_out(certificate.names_more)
        ));
        lines.put(if certificate.self_signed {
            "Signed by itself.".to_owned()
        } else {
            format!(
                "Signed by {}{}.",
                certificate.issuer,
                cut(certificate.issuer_cut)
            )
        });
        lines.put(format!(
            "Valid from {} until {}.",
            certificate.valid_from.said(),
            certificate.valid_until.said()
        ));
    }
    lines
}

/// What follows a text that was cut, being too long to show: an ellipsis, or nothing.
const fn cut(was: bool) -> &'static str {
    if was {
        "…"
    } else {
        ""
    }
}

/// What follows a list of names some of which were left out: how many, or nothing.
fn left_out(more: usize) -> String {
    if more == 0 {
        String::new()
    } else {
        format!(" and {more} more")
    }
}

/// What the web surface asked of the core for itself.
pub(super) fn surfaced(answer: &Answer) -> Lines {
    match answer {
        Answer::Standing(report) => standing(report),
        Answer::Heard(report) => heard(report),
        Answer::Renewed(report) => renewed(report),
        Answer::Untuned(report) => untuned(*report),
        Answer::Exported(report) => super::logs::exported(report),
        Answer::Dashboard(snapshot) => crate::dashboard::listed(snapshot),
        Answer::Octets(octets) => super::plugin::document(&String::from_utf8_lossy(&octets.bytes)),
    }
}

/// The stack's identifier, the settings that are off, and whether a way to be told,
/// a waiting push and members' own keys stand.
fn standing(report: &StackStanding) -> Lines {
    let mut lines = Lines::default();
    lines.put(report.stack.as_ref().map_or_else(
        || "No stack identifier is kept here.".to_owned(),
        |stack| format!("Stack {stack}."),
    ));
    for setting in &report.switched_off {
        lines.put(format!("{setting} is off."));
    }
    lines.put(format!("A way to be told: {}.", said(report.told)));
    lines.put(format!("A push waiting: {}.", said(report.waiting)));
    lines.put(format!(
        "Members mint their own keys: {}.",
        said(report.members_mint)
    ));
    lines
}

/// Which service an event was handed on from.
fn heard(report: &Heard) -> Lines {
    let mut lines = Lines::default();
    lines.put(format!("An event from {} was handed on.", report.service));
    lines
}

/// Until when a member's grant to play holds.
fn renewed(report: &Renewed) -> Lines {
    let mut lines = Lines::default();
    let holds = format!(
        "{}'s grant to play holds until {}.",
        report.member, report.lasts_until
    );
    lines.put(if report.rehearsed {
        format!("A rehearsal renews nothing: {holds}")
    } else {
        holds
    });
    lines
}

/// How many live streams were closed because their passes lapsed.
fn untuned(report: Untuned) -> Lines {
    let mut lines = Lines::default();
    lines.put(format!(
        "{} live stream{} closed, the pass of each lapsed unrenewed.",
        report.closed,
        s(report.closed)
    ));
    lines
}

/// Yes or no, as a line says it.
const fn said(holds: bool) -> &'static str {
    if holds {
        "yes"
    } else {
        "no"
    }
}

#[cfg(test)]
mod tests;
