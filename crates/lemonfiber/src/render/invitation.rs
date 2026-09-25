//! What to send somebody, and the one thing they should know before they accept.
//!
//! The shape is the answer first: who it is for, then the single address to pass
//! on. An operator reading only the first two lines has everything they need to
//! send the message.
//!
//! The code beneath it is the same address again, for a camera. Somebody being
//! invited is usually holding the phone they will watch on, and typing an address
//! and then a password on a phone keyboard is exactly the friction that makes
//! people give up before they start.

use lemonfiber_core::model::{Invitation, InvitationStanding, Linked};

use super::{qr, Lines};
use crate::say;

/// What an operator is told after offering somebody an account.
pub(super) fn invitation(report: &Invitation) -> Lines {
    let mut lines = Lines::default();
    if report.rehearsed {
        // First, because everything under it reads as an account that exists.
        lines.put("Nothing was made. This is what the invitation would say:".to_owned());
    }
    lines.put(match report.standing {
        InvitationStanding::Made => format!(
            "{} can sign in — unclaimed until they set a password, and it lapses in {} hours",
            report.name, report.hours
        ),
        InvitationStanding::Waiting => format!(
            "{} was already invited and it still stands, so here is that message again",
            report.name
        ),
        InvitationStanding::Joined => format!("{} is already in the house", report.name),
        // Said as what stopped working and what is at stake, because both are things
        // the operator is about to be asked. The window is the same one an offer has,
        // counted from the reset — but what is withdrawn at the end of it is an account
        // somebody has watched on, so this is the one place the consequence is spelled
        // out rather than left to the word "lapses".
        InvitationStanding::Reset => format!(
            "{}'s old password no longer works — they set a new one next time they sign \
             in, within {} hours or the account is removed",
            report.name, report.hours
        ),
    });
    lines.put(format!("  {}", report.address));
    lines.put(HOME_ONLY.to_owned());
    if let Some(caution) = &report.caution {
        lines.put(format!("  {caution}"));
    }

    // Nothing to claim, so nothing to point a camera at. The address stands because
    // it is still where they sign in, but a code and an instruction about setting a
    // first password would be telling somebody to do again what they have done.
    if report.standing == InvitationStanding::Joined {
        return footer(lines, report);
    }

    if let Some(drawn) = qr::rows(&report.address, say::folding()) {
        lines.spaced("Or point their phone's camera at this:");
        for row in drawn {
            lines.put(format!("  {row}"));
        }
    }

    lines.spaced(format!(
        "Tell them to sign in as `{}`. They will be asked to set a password.",
        report.name
    ));

    // Said here because here is where somebody is being asked to join. Telling them
    // afterwards is telling them once they have already put their watching on a
    // machine somebody else administers.
    lines.put(WATCHED.to_owned());

    footer(lines, report)
}

/// What is left to say once the invitation itself has been said.
///
/// Both paths through this view end here — the one for somebody already in the house
/// as much as the one for a new account — so anything owed at the end is owed on
/// both, and there is one place it can be forgotten from rather than two.
fn footer(mut lines: Lines, report: &Invitation) -> Lines {
    // Said only where it is not the ordinary case. A member the request service
    // already knows about needs no line about it; one it does not is the partial
    // state, and the operator's next question is whether they must do anything.
    if report.linked == Linked::NotYet {
        lines.spaced(NOT_ASKING_YET.to_owned());
    }
    withdrawals(lines, report)
}

/// What the sweep took back on the way past, where it took anything.
fn withdrawals(mut lines: Lines, report: &Invitation) -> Lines {
    if !report.withdrawn.is_empty() {
        lines.spaced(if report.rehearsed {
            "Nobody claimed these in time, so they would be withdrawn:"
        } else {
            "Nobody claimed these in time, so they have been withdrawn:"
        });
        for name in &report.withdrawn {
            lines.put(format!("  {name}"));
        }
    }
    lines
}

/// Said where the account was made but the request service could not be told of it.
///
/// **The invitation is not held back for this.** The account somebody watches with is
/// on the media server and stands from the moment it is made — nothing has to be
/// running for them to claim it. Being able to *ask* for something is a second account
/// on a second service, and that one being down is worth a sentence rather than a
/// refusal.
///
/// It says there is nothing to undo, because the obvious fear on reading this is that
/// half a thing was made and somebody must go and tidy it up.
const NOT_ASKING_YET: &str = "The request service could not be reached, so they can \
    watch but cannot ask for anything yet. Nothing is half-made and nothing needs \
    undoing — invite them again once it is up and they will be able to ask.";

/// Where the address works, said beside the address rather than under it.
///
/// The stack is published to the home network and nowhere else. An address that
/// opens nothing from a phone on mobile data is not broken — it is being asked
/// from the wrong place, and somebody who was not told that reads a working
/// invitation as a dead link and gives up without saying so.
const HOME_ONLY: &str = "  It opens on the home network only, not from outside the house.";

/// What the household is owed before they accept.
///
/// The operator of this stack can see what everybody watches — the media server
/// keeps that and shows it to whoever administers it. Said plainly and without
/// softening: somebody deciding whether to accept an account is entitled to know
/// what accepting it means, and a sentence they have to go looking for is one they
/// will not find.
const WATCHED: &str = "  Tell them too: whoever runs this server can see what they watch and when.";

#[cfg(test)]
mod tests;
