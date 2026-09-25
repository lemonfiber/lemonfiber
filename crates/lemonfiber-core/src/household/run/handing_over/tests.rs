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
            // What became of the words does not reach this message. It is written
            // to the member, and telling somebody how they were told is a sentence
            // for the operator rather than for them.
            told: None,
            expired: false,
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
        None,
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
    let said = to_hand_over(
        &member(Some(asking(None)), Vec::new()),
        &quality(),
        None,
        false,
    );

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
    let said = to_hand_over(
        &member(Some(asking(None)), Vec::new()),
        &quality(),
        None,
        false,
    );
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
        None,
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
        None,
        false,
    );
    let nothing_refused = to_hand_over(
        &member(Some(asking(Some(5))), Vec::new()),
        &quality(),
        None,
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
        None,
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
    let said = to_hand_over(
        &member(Some(asking(Some(5))), Vec::new()),
        &quality(),
        None,
        true,
    );
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
    assert!(to_hand_over(&member(None, Vec::new()), &quality(), None, false).is_empty());
}

/// A request nothing holds a title for is still named as something.
#[test]
fn a_request_with_no_title_is_still_named() {
    let mut untitled = request(41, State::WaitingForApproval, None);
    untitled.title = None;
    let mut nameless = untitled.clone();
    nameless.media = None;

    let by_kind = to_hand_over(&member(None, vec![untitled]), &quality(), None, false);
    let by_number = to_hand_over(&member(None, vec![nameless]), &quality(), None, false);
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

    let said = to_hand_over(
        &member(None, vec![overnight, unread]),
        &quality(),
        None,
        false,
    );

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

/// Where a period is arranged, the member stops being promised nothing ends the wait.
///
/// **And is promised no date instead.** The period is the operator's to change and
/// nothing runs it on its own, so a figure said here is one this could not keep —
/// what they are owed is that the wait can end unanswered and that they will hear.
#[test]
fn a_member_under_a_period_is_told_the_wait_can_end_unanswered() {
    let said = to_hand_over(
        &member(None, vec![request(1, State::WaitingForApproval, None)]),
        &quality(),
        Some(30),
        false,
    );
    let line = said
        .iter()
        .find(|line| line.starts_with("Waiting"))
        .cloned()
        .unwrap_or_default();

    assert!(line.contains("closed for having waited too long"), "{line}");
    assert!(line.contains("you will be told which"), "{line}");
    assert!(!line.contains("Nothing expires it"), "{line}");
    assert!(!line.contains("30"), "a date this could not keep: {line}");
}

/// A request nobody ruled on is not said to have been turned down.
///
/// Being refused and having run out are different things to have happened to
/// somebody, and only one of them makes asking again the sensible next move.
#[test]
fn a_request_that_ran_out_is_not_said_to_have_been_turned_down() {
    let mut ran_out = request(
        1,
        State::Declined,
        Some("nobody ruled on it within 30 days"),
    );
    if let Some(refused) = ran_out.refused.as_mut() {
        refused.expired = true;
    }

    let said = to_hand_over(&member(None, vec![ran_out]), &quality(), Some(30), false);

    assert!(
        said.iter()
            .any(|line| line.starts_with("Closed unanswered on 2026-08-17: ")),
        "{said:?}"
    );
    assert!(
        !said.iter().any(|line| line.starts_with("Turned down")),
        "a request nobody ruled on was said to have been turned down: {said:?}"
    );
}

/// Every policy a household can be under has words to be told in.
#[test]
fn every_policy_has_words_to_be_told_in() {
    for policy in Policy::ALL {
        let mut held = asking(None);
        held.policy = policy;
        let said = to_hand_over(&member(Some(held), Vec::new()), &quality(), None, false);
        assert!(
            said.first()
                .is_some_and(|line| line.starts_with("What you may ask for: ")),
            "{said:?}"
        );
    }
}
