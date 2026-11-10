//! The back-ends the stack delivers its alerts to, as the operator reads them.

use lemonfiber_core::model::{
    BackendProof, BackendReport, ChosenBy, Delivery, DeliveryOutcome, NotifyMineReport,
    NotifyReport, NotifyState,
};

use super::news::moment;
use super::Lines;

/// Every back-end, where it reaches, whether it is proven and what became of its newest
/// delivery.
pub(super) fn backends(report: &NotifyReport) -> Lines {
    let mut lines = Lines::default();
    if report.rehearsed {
        lines.put("Rehearsed: nothing was changed and nothing was sent.");
        lines.put(String::new());
    }
    lines.put(standing(report.state));
    for one in &report.backends {
        lines.put(String::new());
        backend(&mut lines, one);
    }
    for unread in &report.unread {
        lines.put(String::new());
        lines.put(format!(
            "{} could not be read: {}",
            unread.backend, unread.because
        ));
    }
    lines
}

/// The ways a member may be told that something they asked for is ready, and which they
/// chose.
pub(super) fn mine(report: &NotifyMineReport) -> Lines {
    let mut lines = Lines::default();
    if report.offered.is_empty() {
        lines.put("Nothing is offered to be told on yet.");
    }
    for one in &report.offered {
        let chosen = if one.receiving {
            "told here"
        } else {
            "not chosen"
        };
        lines.put(format!("{}  ({})  {chosen}", one.name, one.id));
    }
    lines.put(if report.phone {
        "Your phone is registered to be woken."
    } else {
        "No phone of yours is registered to be woken."
    });
    lines
}

/// Where delivery as a whole stands.
const fn standing(state: NotifyState) -> &'static str {
    match state {
        NotifyState::Unconfigured => {
            "No back-end is configured, so alerts reach only the dashboard and the phones."
        }
        NotifyState::Ready => "Alerts are delivered, and every back-end is answering.",
        NotifyState::Degraded => "Alerts are delivered, but not to every back-end.",
        NotifyState::Unreachable => "No back-end is taking alerts.",
    }
}

/// One back-end, as the listing draws it.
fn backend(lines: &mut Lines, one: &BackendReport) {
    lines.put(format!("{}  {}  {}", one.id, one.kind.word(), one.reaches));
    if let ChosenBy::Member(member) = &one.chosen_by {
        lines.put(format!("  chosen by {member}"));
    }
    if let Some(hosted) = &one.hosted {
        lines.put(format!(
            "  hosted at {}, which leaves the self-hosted posture; what leaves for it: {}",
            hosted.host,
            hosted.leaves.join("; ")
        ));
    }
    lines.put(match &one.proof {
        BackendProof::Unproven => "  unproven: run notify test to see a message arrive".to_owned(),
        BackendProof::Proven(at) => format!("  proven {}", moment(*at)),
        BackendProof::Held(why) => format!("  held, delivering nothing: {why}"),
    });
    if let Some(last) = &one.last {
        lines.put(format!("  newest delivery {}", delivered(last)));
    }
    for failed in &one.failures {
        lines.put(format!("  failed {}", delivered(failed)));
    }
    if let Some(retry) = &one.retry {
        lines.put(format!(
            "  attempt {} of {} at {}",
            retry.attempt,
            retry.of,
            moment(retry.next)
        ));
    }
}

/// One delivery, when it was and what became of it.
fn delivered(delivery: &Delivery) -> String {
    let outcome = match &delivery.outcome {
        DeliveryOutcome::Confirmed => "confirmed arriving".to_owned(),
        DeliveryOutcome::Sent => "taken, its arrival not seen".to_owned(),
        DeliveryOutcome::Failed(why) => format!("failed: {why}"),
    };
    format!(
        "{} ({}) {outcome}",
        moment(delivery.at),
        delivery.kind.word()
    )
}

#[cfg(test)]
mod tests;
