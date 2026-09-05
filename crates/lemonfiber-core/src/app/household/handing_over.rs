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
    no_room: bool,
) -> Vec<String> {
    let unanswered = waiting(&member.requests);
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
/// Named as waiting on a person rather than as being worked on, and said to expire of
/// nothing: a member who read "waiting" as "in progress" would go on waiting, and one who
/// assumed something eventually clears it would never ask again.
fn waiting(requests: &[MemberRequest]) -> Vec<String> {
    let mut said = Vec::new();
    for request in requests
        .iter()
        .filter(|request| request.state == Some(State::WaitingForApproval))
    {
        said.push(format!(
            "Waiting on an answer: {}{}{}. Nothing expires it — it waits until somebody \
             rules on it.",
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
fn turned_down(requests: &[MemberRequest]) -> Vec<String> {
    let mut said: Vec<String> = requests
        .iter()
        .filter(|request| request.state == Some(State::Declined))
        .filter_map(|request| {
            request.refused.as_ref().map(|refused| {
                format!(
                    "Turned down{}: {} — {}.",
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
mod tests {
    use super::to_hand_over;
    use crate::asking::{Estimate, Policy, Refused, Standing};
    use crate::household::State;
    use crate::model::{Counted, HouseholdMember, MemberAsking, MemberRequest};
    use crate::quality::{Preset, Selection};

    /// The quality these fixtures are read at.
    fn quality() -> Selection {
        Selection::everywhere(Preset::Balanced)
    }

    /// One member holding a limit they have nearly used, or holding none at all.
    fn asking(limit: Option<u32>) -> MemberAsking {
        MemberAsking {
            policy: if limit.is_some() {
                Policy::WithinALimit
            } else {
                Policy::Trusted
            },
            standing: Standing::NearQuota,
            films: Counted {
                limit,
                used: 4,
                remaining: limit.map(|limit| limit.saturating_sub(4)),
                period: limit.map(|_| "a week".to_owned()),
            },
            television: Counted {
                limit: None,
                used: 0,
                remaining: None,
                period: None,
            },
            frees_up: limit.map(|_| "2026-08-24T21:04:09".to_owned()),
        }
    }

    /// One request in the state named.
    fn request(id: i64, state: State, refused: Option<&str>) -> MemberRequest {
        MemberRequest {
            id,
            title: Some("The Thing".to_owned()),
            media: Some("film".to_owned()),
            state: Some(state),
            waiting_days: (state == State::WaitingForApproval).then_some(9),
            estimate: Some(Estimate::film(Preset::Balanced)),
            refused: refused.map(|reason| Refused {
                reason: reason.to_owned(),
                at: (id != 2).then(|| "2026-08-17T21:04:09".to_owned()),
            }),
        }
    }

    /// A member with what is given, and nothing else.
    fn member(asking: Option<MemberAsking>, requests: Vec<MemberRequest>) -> HouseholdMember {
        HouseholdMember {
            name: "Ana".to_owned(),
            requests,
            asking,
            ..HouseholdMember::default()
        }
    }

    /// Somebody close to their limit is told the limit, the spend and the reset.
    ///
    /// The three the requirement asks for, in front of the person who hit it rather than
    /// in front of the person who set it.
    #[test]
    fn somebody_close_to_their_limit_is_told_all_three() {
        let said = to_hand_over(
            &member(Some(asking(Some(5))), Vec::new()),
            &quality(),
            false,
        );
        let whole = said.join("\n");

        assert!(whole.contains("4 of 5"), "{whole}");
        assert!(whole.contains("a week"), "{whole}");
        assert!(whole.contains("2026-08-24"), "{whole}");
    }

    /// Somebody nothing limits is told that, rather than told nothing.
    ///
    /// An absent limit and an unread one look identical to a member left with silence,
    /// and only one of them means they may ask for whatever they like.
    #[test]
    fn somebody_nothing_limits_is_told_so() {
        let said = to_hand_over(&member(Some(asking(None)), Vec::new()), &quality(), false);

        assert!(
            said.iter().any(|line| line.contains("Nothing limits")),
            "{said:?}"
        );
    }

    /// Roughly what a thing costs is said before anybody has asked for one.
    ///
    /// The whole of what the requirement wants: the figure that changes a mind arrives
    /// before the choice rather than beside the approval.
    #[test]
    fn what_a_thing_costs_is_said_before_anybody_asks() {
        let said = to_hand_over(&member(Some(asking(None)), Vec::new()), &quality(), false);
        let whole = said.join("\n");

        assert!(whole.contains("Before you ask"), "{whole}");
        assert!(whole.contains("a film about "), "{whole}");
        assert!(whole.contains("a season of television about "), "{whole}");
        assert!(whole.contains("not a measurement"), "{whole}");
    }

    /// Waiting and refused are two answers, and neither borrows the other's words.
    #[test]
    fn waiting_and_refused_are_never_the_same_answer() {
        let said = to_hand_over(
            &member(
                Some(asking(Some(5))),
                vec![
                    request(1, State::WaitingForApproval, None),
                    request(2, State::Declined, Some("we already have it dubbed")),
                ],
            ),
            &quality(),
            false,
        );

        let waiting = said
            .iter()
            .filter(|line| line.starts_with("Waiting"))
            .count();
        let refused = said
            .iter()
            .filter(|line| line.starts_with("Turned down"))
            .count();
        assert_eq!((waiting, refused), (1, 1), "{said:?}");
        assert!(
            said.iter().any(|line| line.contains("Nothing expires it")),
            "{said:?}"
        );
        assert!(
            said.iter()
                .any(|line| line.contains("we already have it dubbed")),
            "{said:?}"
        );
    }

    /// A reason is said to be this program's record and never the service's.
    ///
    /// Said once rather than under every refusal, and not at all where nothing was
    /// refused — a caveat repeated is one nobody reads, and one offered for something
    /// that did not happen is an apology for nothing.
    #[test]
    fn a_reason_is_said_to_be_this_programs_own_record() {
        let refused = to_hand_over(
            &member(
                None,
                vec![
                    request(1, State::Declined, Some("too new")),
                    request(2, State::Declined, Some("no room")),
                ],
            ),
            &quality(),
            false,
        );
        let nothing_refused = to_hand_over(
            &member(Some(asking(Some(5))), Vec::new()),
            &quality(),
            false,
        );

        assert_eq!(
            refused
                .iter()
                .filter(|line| line.contains("lemonfiber's own record"))
                .count(),
            1,
            "{refused:?}"
        );
        // The day it was answered where the clock could be written, and the words alone
        // where it could not — a refusal is worth reporting either way.
        assert!(
            refused
                .iter()
                .any(|line| line.starts_with("Turned down on 2026-08-17: ")),
            "{refused:?}"
        );
        assert!(
            refused.iter().any(|line| line.starts_with("Turned down: ")),
            "{refused:?}"
        );
        assert!(
            !nothing_refused
                .iter()
                .any(|line| line.contains("lemonfiber's own record")),
            "{nothing_refused:?}"
        );
    }

    /// A request refused where this program never saw it says nothing about why.
    ///
    /// Somebody using the request service directly leaves no reason here, and inventing
    /// one would put words in their mouth.
    #[test]
    fn a_refusal_made_elsewhere_carries_no_words() {
        let said = to_hand_over(
            &member(
                Some(asking(Some(5))),
                vec![request(1, State::Declined, None)],
            ),
            &quality(),
            false,
        );

        assert!(!said.is_empty(), "there was no message to look in");
        assert!(
            !said.iter().any(|line| line.starts_with("Turned down")),
            "{said:?}"
        );
    }

    /// A full disk is said as the disk, and never as somebody's limit.
    #[test]
    fn a_full_disk_is_said_as_the_disk() {
        let said = to_hand_over(&member(Some(asking(Some(5))), Vec::new()), &quality(), true);
        let line = said
            .iter()
            .find(|line| line.contains("no room left on the disk"))
            .cloned()
            .unwrap_or_default();

        assert!(
            line.contains("that is the disk rather than anything of yours"),
            "{line}"
        );
        assert!(!line.contains("limit is"), "{line}");
    }

    /// A member with nothing to say is handed nothing rather than a bare notice.
    ///
    /// The cost line alone is the same sentence for everybody in the house, which is a
    /// notice rather than an answer, and handing one over as though it were addressed to
    /// somebody would teach a household to ignore the next one.
    #[test]
    fn a_member_with_nothing_to_say_is_handed_nothing() {
        assert!(to_hand_over(&member(None, Vec::new()), &quality(), false).is_empty());
    }

    /// A request nothing holds a title for is still named as something.
    #[test]
    fn a_request_with_no_title_is_still_named() {
        let mut untitled = request(41, State::WaitingForApproval, None);
        untitled.title = None;
        let mut nameless = untitled.clone();
        nameless.media = None;

        let by_kind = to_hand_over(&member(None, vec![untitled]), &quality(), false);
        let by_number = to_hand_over(&member(None, vec![nameless]), &quality(), false);
        assert!(
            !by_kind.is_empty() && !by_number.is_empty(),
            "no message at all"
        );

        assert!(
            by_kind
                .iter()
                .any(|line| line.contains("the film you asked for")),
            "{by_kind:?}"
        );
        assert!(
            by_number.iter().any(|line| line.contains("request 41")),
            "{by_number:?}"
        );
    }

    /// One day reads as one day rather than as one days.
    #[test]
    fn one_day_waiting_reads_as_one_day() {
        let mut overnight = request(1, State::WaitingForApproval, None);
        overnight.waiting_days = Some(1);
        let mut unread = request(2, State::WaitingForApproval, None);
        unread.waiting_days = None;
        unread.estimate = None;

        let said = to_hand_over(&member(None, vec![overnight, unread]), &quality(), false);

        assert!(
            said.iter().any(|line| line.contains("1 day ago")),
            "{said:?}"
        );
        assert!(
            said.iter()
                .any(|line| line.contains("Waiting on an answer: The Thing. Nothing expires")),
            "{said:?}"
        );
    }

    /// Every policy a household can be under has words to be told in.
    #[test]
    fn every_policy_has_words_to_be_told_in() {
        for policy in Policy::ALL {
            let mut held = asking(None);
            held.policy = policy;
            let said = to_hand_over(&member(Some(held), Vec::new()), &quality(), false);
            assert!(
                said.first()
                    .is_some_and(|line| line.starts_with("What you may ask for: ")),
                "{said:?}"
            );
        }
    }
}
