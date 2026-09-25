//! What one household member would be told, written to them rather than about them.
//!
//! **Every figure this composes already existed and none of it was in front of the
//! person it is about.** What a period allows and what it has left, when it next makes
//! room, roughly what a thing will take before anybody asks for it, and why a request was
//! refused — all of it is gathered beside the household list, which is a list the
//! household never sees. So the requirements it serves read as unmet by anybody standing
//! where they are written from.
//!
//! **This program cannot put it where they ask.** The request service has no notion of
//! size, its over-quota refusal names neither the limit nor the reset, and its decline
//! carries no reason at all — and nobody in the house has an account here or a way in, on
//! purpose. What is left is to write the answer in the second person and hand it to the
//! one person who can pass it on, which is what this does: a message ready to send as it
//! stands, rather than four figures an operator has to compose into one.
//!
//! **The three answers are kept apart.** Waiting on a decision, refused, and refused by
//! the disk are different things to be told, and a surface that merged any two of them
//! would hand somebody the wrong next move — wait, ask for something else, or free some
//! room. So each is its own line and none of them borrows another's words.

use crate::household::State;
use crate::model::{HouseholdMember, MemberRequest};
use crate::quality::Selection;
use crate::recyclarr::Kind;

/// What to tell one member, in the order they would want it.
///
/// Nothing at all for somebody with no standing to report and nothing waiting. What a
/// thing costs and what the disk is doing are true of everybody in the house alike, so a
/// message carrying only those is a notice rather than an answer — and one handed over as
/// though it were addressed to somebody teaches a household to ignore the next.
#[must_use]
pub(super) fn to_hand_over(
    member: &HouseholdMember,
    quality: &Selection,
    expiring: Option<u32>,
    no_room: bool,
) -> Vec<String> {
    let unanswered = waiting(&member.requests, expiring);
    let refused = turned_down(&member.requests);
    if member.asking.is_none() && unanswered.is_empty() && refused.is_empty() {
        return Vec::new();
    }
    let mut said = Vec::new();
    if let Some(asking) = &member.asking {
        said.push(format!("What you may ask for: {}.", asking.policy.means()));
        said.push(asking.sentence().map_or_else(
            || "Nothing limits how much you may ask for.".to_owned(),
            |sentence| format!("Your limit: {sentence}."),
        ));
    }
    said.push(before_you_ask(quality));
    said.extend(unanswered);
    said.extend(refused);
    if no_room {
        said.push(NO_ROOM.to_owned());
    }
    said
}

/// What is said where the disk has no room left.
///
/// The disk's own answer, said as the disk and never as a limit. Somebody who read a
/// full disk as their own quota would wait for a period to roll over and watch the same
/// refusal happen again, having done the one thing that could not help.
const NO_ROOM: &str = "There is no room left on the disk, so nothing new is being fetched \
                       at the moment — that is the disk rather than anything of yours, and \
                       waiting for your limit to roll over will not change it.";

/// Roughly what the two kinds of thing cost, before anybody has chosen one.
///
/// The line the requirement is actually about. A household member is shown no cost where
/// they ask, and most excessive requests are innocent — nobody means to ask for four
/// hundred gigabytes, they did not know a whole series at that quality was that much. The
/// figure is a guess and says so, because a number without the word in front of it is one
/// somebody will hold this house to.
fn before_you_ask(quality: &Selection) -> String {
    format!(
        "Before you ask, roughly what things take at the quality this house is set to: a \
         film {}, a season of television {} — a guess from the quality and how long a \
         thing of that kind usually runs, not a measurement.",
        super::allowance::for_kind(Kind::Radarr, quality).reading(),
        super::allowance::for_kind(Kind::Sonarr, quality).reading(),
    )
}

/// What is still waiting on somebody, where anything is.
///
/// Named as waiting on a person rather than as being worked on: a member who read
/// "waiting" as "in progress" would go on waiting, and one who assumed something
/// eventually clears it would never ask again.
///
/// **What it says about the end of the wait is whichever is true of this house.** Where
/// nothing is arranged it says nothing expires it, which is what it has always said and
/// what is still true. Where a period is arranged it says the wait can end unanswered and
/// that they will be told — and it names no figure, because the period is the operator's
/// to change and nothing runs it on its own, so a date said here is one this could not
/// keep.
fn waiting(requests: &[MemberRequest], expiring: Option<u32>) -> Vec<String> {
    let ends = if expiring.is_some() {
        "It waits until somebody rules on it, or until it is closed for having waited too \
         long — you will be told which"
    } else {
        "Nothing expires it — it waits until somebody rules on it"
    };
    let mut said = Vec::new();
    for request in requests
        .iter()
        .filter(|request| request.state == Some(State::WaitingForApproval))
    {
        said.push(format!(
            "Waiting on an answer: {}{}{}. {ends}.",
            named(request),
            request
                .estimate
                .map_or_else(String::new, |estimate| format!(", {}", estimate.reading())),
            request
                .waiting_days
                .map_or_else(String::new, |days| format!(
                    ", asked {days} day{} ago",
                    crate::plural::s(usize::try_from(days).unwrap_or(2))
                )),
        ));
    }
    said
}

/// What was refused and why, where this machine refused it.
///
/// The honesty line is said once and only where there is a reason to explain: repeated
/// under every refusal it is a caveat nobody reads, and said where nothing was refused it
/// is an apology for something that did not happen.
///
/// **A request nobody ruled on is not one somebody turned down**, and it does not say so.
/// Being refused and having run out are different things to have happened to a person, and
/// the second is the one where asking again is the sensible next move — which nobody reads
/// off a line beginning with the word for the first.
fn turned_down(requests: &[MemberRequest]) -> Vec<String> {
    let mut said: Vec<String> = requests
        .iter()
        .filter(|request| request.state == Some(State::Declined))
        .filter_map(|request| {
            request.refused.as_ref().map(|refused| {
                format!(
                    "{}{}: {} — {}.",
                    if refused.expired {
                        "Closed unanswered"
                    } else {
                        "Turned down"
                    },
                    refused.at.as_deref().map_or_else(String::new, |at| format!(
                        " on {}",
                        at.split('T').next().unwrap_or(at)
                    )),
                    named(request),
                    refused.reason
                )
            })
        })
        .collect();
    if !said.is_empty() {
        said.push(
            "The request service tells you a request was declined and carries no reason \
             with it, so those words are lemonfiber's own record of what was said here."
                .to_owned(),
        );
    }
    said
}

/// What to call one request, by its title where anything knows one.
///
/// A request nobody has approved has been handed to no service, so there is nothing
/// holding a title for it — which is exactly the state most of these lines are about. The
/// kind is the fallback rather than the number, because a member reading "request 41"
/// would have to go and look it up in the one place this message exists to save them
/// opening.
fn named(request: &MemberRequest) -> String {
    request.title.clone().unwrap_or_else(|| {
        request.media.clone().map_or_else(
            || format!("request {}", request.id),
            |media| format!("the {media} you asked for"),
        )
    })
}

#[cfg(test)]
mod tests;
