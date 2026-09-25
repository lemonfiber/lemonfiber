//! The words about the people in the household: who is offered an account, whose shelf
//! is read, and what they may ask for.

use lemonfiber_core::app::{Allowance, Answer, Arranged, Chosen, Command, Decision};
use lemonfiber_core::asking::Policy;
use lemonfiber_core::ports::service::{Quota, Unrated};

use crate::exit::USAGE;
use crate::say::complain;
use lemonfiber::cli::{HouseholdCommand, RawAllowance, RawUnrated};

/// The command line spells what somebody may watch as three flags and the core carries
/// them as one choice, because they are one decision taken at one moment. Only the
/// third needs turning: libraries are named as the media server names them and an age
/// limit is the age the server already keeps, while what to do about unrated content is
/// a word here and a choice there.
///
/// **Nothing said is nothing carried.** Leaving the flag out is not choosing to let
/// unrated content through — it is saying nothing about it, which leaves the answer to
/// whatever a restriction carries by default. A `false` written for a word nobody typed
/// would be this surface deciding on the household's behalf.
pub(crate) fn invitation(name: String, allowance: RawAllowance) -> Command {
    Command::Invite {
        name,
        confirm: true,
        allowance: Allowance {
            libraries: allowance.libraries,
            age_limit: allowance.age_limit,
            unrated: allowance.unrated.map(|chosen| match chosen {
                RawUnrated::Block => Unrated::HeldBack,
                RawUnrated::Allow => Unrated::LetThrough,
            }),
        },
    }
}

/// Whose shelf, and how much of it.
///
/// Naming nobody cannot happen — the word requires it, because there is no
/// whole-household form of this to fall back to. Naming a number of nought or more than
/// one read answers with is refused rather than rounded: somebody who asked for a
/// thousand and was shown five hundred has been told that is the shelf.
pub(crate) fn held(member: String, most: Option<u32>) -> Result<Command, u8> {
    // Taken from the served read rather than restated, so a terminal and a browser
    // looking at one household cannot come to see two different shelves. A number
    // written down twice is a number that drifts the first time one of them moves.
    let most = most.unwrap_or(lemonfiber_api::read::table::A_SHELF);
    if most == 0 || most > lemonfiber_api::read::table::MOST_AT_ONCE {
        complain!(
            "error: `--most` takes a number from 1 to {}",
            lemonfiber_api::read::table::MOST_AT_ONCE
        );
        return Err(USAGE);
    }
    Ok(Command::Held { member, most })
}

/// What is being asked about the household: who is here, or what they may ask for.
///
/// One word with four things under it, because they are one subject. Naming nothing is
/// the reading; naming one of the three is a decision about what that reading shows.
///
/// **The narrowing and the decisions do not mix.** `--member` on the word itself narrows
/// the *reading* to one person, and a decision about one person carries its own — so the
/// two together are two requests in one line, and the pair is refused rather than one
/// half being dropped.
pub(crate) fn household(
    member: Option<String>,
    action: Option<HouseholdCommand>,
) -> Result<Command, u8> {
    let Some(action) = action else {
        return Ok(Command::Household { member });
    };
    if member.is_some() {
        complain!(
            "error: `--member` narrows who is listed and cannot be given to a decision \
             (name the person on the decision instead)"
        );
        return Err(USAGE);
    }
    match action {
        HouseholdCommand::Allow {
            member,
            policy,
            requests,
            days,
        } => allowing(member, policy.as_deref(), requests, days),
        HouseholdCommand::Approve { request } => Ok(Command::Deciding(Decision {
            request,
            answer: Answer::LetThrough,
        })),
        HouseholdCommand::Decline { request, reason } => Ok(Command::Deciding(Decision {
            request,
            answer: Answer::TurnedDown { reason },
        })),
        HouseholdCommand::Expiring { after, never } => {
            Ok(Command::Expiring(arranging(after, never)))
        }
    }
}

/// What is being arranged about the requests nobody rules on.
///
/// **Naming nothing is a request in its own right rather than an omission**, and it is the
/// one that runs: an operator who has already said how long is too long is not saying it
/// again to act on it. That is why there is no shape here for a default — a period this
/// invented would close somebody's request on lemonfiber's authority, and the run that
/// names none is asking to act on the household's own.
const fn arranging(after: Option<u32>, never: bool) -> Arranged {
    match (after, never) {
        (Some(days), _) => Arranged::After(days),
        (None, true) => Arranged::Never,
        (None, false) => Arranged::AsAgreed,
    }
}

/// What the household is to be allowed to ask for, from the words it was chosen in.
///
/// The policy is a word here and a value there; the limit is two numbers that only mean
/// something together, which is why the command line refuses either without the other
/// before this is reached. A word this build does not know is refused by name rather
/// than falling to whichever policy is safer — somebody who wrote a word and meant it
/// must not be given a different arrangement because of a spelling.
fn allowing(
    member: Option<String>,
    policy: Option<&str>,
    requests: Option<u32>,
    days: Option<u32>,
) -> Result<Command, u8> {
    let mut chosen = None;
    if let Some(written) = policy {
        let Some(named) = Policy::from_label(written) else {
            complain!(
                "error: no policy named `{written}` (try {})",
                Policy::labels()
            );
            return Err(USAGE);
        };
        chosen = Some(named);
    }
    Ok(Command::Allowing(Chosen {
        member,
        policy: chosen,
        quota: requests
            .zip(days)
            .map(|(requests, days)| Quota { requests, days }),
    }))
}
