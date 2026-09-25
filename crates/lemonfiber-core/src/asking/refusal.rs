//! What is said where what a household may ask for could not be settled.
//!
//! **A limit that refuses out loud is the whole design.** A request quietly dropped and
//! a request refused with the figure beside it are the same outcome for the disk and
//! opposite outcomes for the person who asked, so every refusal here names the thing it
//! turned on — the limit, the word, the request, or the person — and never merely
//! reports that something did not work.
//!
//! **The disk is not one of these.** A machine with no room left refuses an acquisition
//! in [`crate::app`]'s own words, from the one reading of the volumes every command that
//! brings content onto the disk shares. That refusal names the disk and this file names
//! limits, which is exactly the distinction that matters: an operator who read a full
//! disk as somebody's quota would go and raise a quota and watch it happen again.

use crate::error::codes::quota::{
    NEVER_HERE, NOBODY, NOTHING_AGREED, NOT_WAITING, NO_LIMIT, NO_REASON, TOO_SOON, UNREACHABLE,
};
use crate::error::{Amiss, Problem, Remedy, Severity};

/// Said where the request service could not be asked or would not answer.
///
/// Named as nothing having been changed rather than as a failure, because those are
/// different states to be left in: an operator told only that something went wrong does
/// not know whether to set the policy again or to go and check what it now says.
#[must_use]
pub fn unreachable(doing: &str) -> Problem {
    Problem::new(
        UNREACHABLE,
        Severity::Error,
        format!("the request service would not answer, so {doing}"),
        "What the household may ask for is that service's to hold, so nothing here \
         could be changed and nothing was — what it had before is what it still has",
        Remedy::new("Check the request service is running, then run this again"),
    )
}

/// Said where a policy that only means something with a limit was chosen without one.
///
/// Refused rather than given a number of this product's choosing. A limit is the whole
/// of what that policy is, and one invented here would be a household held to a figure
/// nobody in it agreed to.
#[must_use]
pub(crate) fn no_limit_named() -> Problem {
    Problem::new(
        NO_LIMIT,
        Severity::Error,
        "living within a limit needs a limit, and none was named",
        "This policy lets everything through until somebody has used up their share of \
         a period, so without a share it would be the same as trusting everybody — \
         which is a policy of its own and would be chosen by name",
        Remedy::new("Say how many requests a period allows, and how long the period is"),
    )
    .lies_in(Amiss::Asking)
}

/// Said where the request named is not one anybody is waiting on.
///
/// The number is the request service's own, and one already ruled on is not a mistake to
/// correct silently: an operator approving something a second time has misread a list,
/// and being told so is worth more than a second approval that changes nothing.
#[must_use]
pub(crate) fn nothing_to_decide(request: i64) -> Problem {
    Problem::new(
        NOT_WAITING,
        Severity::Error,
        format!("request {request} is not one that is waiting on anybody"),
        "Only a request nobody has ruled on can be approved or turned down — one \
         already decided keeps the answer it was given",
        Remedy::new("Ask what the household has asked for, to see what is still waiting")
            .with_detail("lemonfiber household"),
    )
    .lies_in(Amiss::Naming)
}

/// Said where a decline carried a reason that says nothing.
///
/// A blank reason is the silent decline this is here to prevent, arriving through the
/// field meant to prevent it.
#[must_use]
pub(crate) fn no_reason_given() -> Problem {
    Problem::new(
        NO_REASON,
        Severity::Error,
        "turning a request down needs a reason, and the one given was blank",
        "Somebody asked for this and will see that it was refused; a refusal with \
         nothing beside it is indistinguishable from being ignored, which is the \
         conversation this is here to save you",
        Remedy::new("Say why in a few words, and pass them on to whoever asked"),
    )
    .lies_in(Amiss::Asking)
}

/// Said where nobody in the household goes by the name that was given.
#[must_use]
pub(crate) fn nobody_called(named: &str, household: &[String]) -> Problem {
    Problem::new(
        NOBODY,
        Severity::Error,
        format!("nobody in this household goes by {named}, so nothing was changed"),
        "A limit is set on somebody the media server holds an account for, matched the \
         way you would say their name rather than exactly",
        Remedy::new("Name somebody who is here").with_detail(household.join(", ")),
    )
    .lies_in(Amiss::Naming)
}

/// Said where somebody has an account here and none on the request service.
///
/// Not a fault and not a member who is missing. The request service learns of somebody
/// when they first sign in to it, so this is an invitation nobody has used yet — and a
/// limit written against nobody would read as a limit that had been applied.
#[must_use]
pub(crate) fn never_asked_here(name: &str) -> Problem {
    Problem::new(
        NEVER_HERE,
        Severity::Warning,
        format!(
            "{name} has never signed in to the request service, so there is nobody \
                 there to hold to a limit"
        ),
        "The request service learns of somebody the first time they sign in to it, and \
         until then it holds no account of theirs for a limit to sit on — what the \
         household is held to applies to them in the meantime",
        Remedy::new("Ask them to open the request service once, then set this again"),
    )
    .lies_in(Amiss::Naming)
}

/// Said where a run was asked to close old requests and nobody has said what old means.
///
/// **Refused rather than given a period of this program's choosing**, and it is the whole
/// of what keeps an expiry from being a policy nobody consented to. A household that never
/// arranged this loses nothing by being asked again; one held to a figure it never named
/// would have somebody's request closed on this program's authority.
#[must_use]
pub(crate) fn nothing_agreed() -> Problem {
    Problem::new(
        NOTHING_AGREED,
        Severity::Error,
        "nothing is closed for waiting here, because no period has been agreed to",
        "Requests wait until somebody rules on them unless this household has said how \
         long is too long, and there is no figure this could choose on your behalf — a \
         request closed against a period nobody named is one nobody agreed to close",
        Remedy::new("Say how many days a request may wait, then start this again")
            .with_detail("lemonfiber household expiring --after 30"),
    )
    .lies_in(Amiss::Asking)
}

/// Said where the period named is shorter than the reminder that comes before it.
///
/// The two are one arrangement rather than two settings. A request closed sooner than the
/// operator is reminded of it is one they never saw waiting: what they would watch is
/// requests disappearing, and the reminder — which exists so somebody can answer before it
/// comes to this — would never once be reached.
#[must_use]
pub(crate) fn sooner_than_the_reminder(after: u32) -> Problem {
    Problem::new(
        TOO_SOON,
        Severity::Error,
        format!(
            "{after} days is sooner than you are reminded that anything is waiting, so \
             nothing was arranged"
        ),
        "You are told a request is waiting once it has waited a week; a period shorter \
         than that closes it before any reading could put it in front of you, which \
         leaves you watching requests vanish rather than answering them",
        Remedy::new("Name a period of a week or more").with_detail(format!(
            "the reminder arrives after {} days",
            super::REMINDING_AFTER
        )),
    )
    .lies_in(Amiss::Asking)
}

#[cfg(test)]
mod tests;
