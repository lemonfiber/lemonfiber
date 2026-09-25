//! What removing somebody costs, said before it is done.
//!
//! The shape is the cost first and the confirmation last, because the operator reading
//! this is deciding, not being informed. A summary that leads with "removed" and buries
//! what went is one somebody skims past.
//!
//! **Every figure here is knowable without removing anybody**, which is the whole reason
//! the unconfirmed run is worth having: what it says is not an estimate of what would
//! happen, it is the same reading the confirmed run acts on.

use lemonfiber_core::model::{HouseholdRemoval, Revoked};

use super::Lines;

/// What an operator is told when removing somebody, or asking what it would cost.
pub(super) fn removal(report: &HouseholdRemoval) -> Lines {
    let mut lines = Lines::default();
    lines.put(if report.confirmed {
        format!("{} is no longer in this household.", report.name)
    } else {
        format!("Removing {} would take all of this:", report.name)
    });

    // Watch history is not a choice this program makes. The media server's own removal
    // takes only an account identifier — there is no option to keep anything — so this
    // is stated as the fact it is rather than as something that could be arranged.
    lines.spaced(if report.confirmed {
        "  Their watch history went with the account. It cannot be got back."
    } else {
        "  Their watch history, which goes with the account. It cannot be got back."
    });

    lines.put(asked_for(report));

    if !report.asks_through_the_request_service {
        // Said rather than left silent: an operator who expected two revocations and
        // saw one has to be told which, and that nothing failed.
        lines.put(
            "  They never signed in to the request service, so there was no account \
             there to take."
                .to_owned(),
        );
    }

    // Read off what was revoked rather than off the flag beside it. Saying it twice —
    // once from `confirmed`, once from `revoked` — is two accounts of one fact that can
    // disagree, and it left a sentence for a state that cannot arise: a confirmed run
    // that revoked nothing. Taken from `revoked` alone, every ending is one that happens.
    lines.spaced(match report.revoked {
        Revoked::Everywhere => "Both accounts are gone.".to_owned(),
        Revoked::MediaServerOnly => {
            "The media server's account is gone. The request service's is not — see below."
                .to_owned()
        }
        Revoked::Nothing => format!(
            "Nothing has been removed. Run `lemonfiber remove {} --confirm` to go ahead.",
            report.name
        ),
    });

    for finding in &report.findings {
        lines.put(format!("  ! {finding}"));
    }
    lines
}

/// The line about what they asked for, in the number the operator is deciding about.
///
/// **Destroyed, not transferred.** The request service removes them by hand so a title
/// still waiting goes back to being unrequested; saying "removed" alone would let
/// somebody read it as the requests surviving under another name.
fn asked_for(report: &HouseholdRemoval) -> String {
    match (report.requests, report.confirmed) {
        (0, _) => "  Nothing they asked for — they had no requests outstanding.".to_owned(),
        (1, true) => "  The one thing they asked for, which no longer exists.".to_owned(),
        (1, false) => "  The one thing they asked for, which stops existing.".to_owned(),
        (many, true) => format!("  The {many} things they asked for, which no longer exist."),
        (many, false) => format!("  The {many} things they asked for, which stop existing."),
    }
}

#[cfg(test)]
mod tests;
