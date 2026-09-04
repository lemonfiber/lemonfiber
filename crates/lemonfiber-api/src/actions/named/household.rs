//! The requests addressed to somebody who lives here.
//!
//! Apart from the table that names them because they are the half of it with nothing
//! to do with forms, files or services: an account offered, a password taken off, an
//! account taken away, what the household may ask for, and one thing it already asked
//! for. Each has to say what it lacks before it can name a command, which is longer
//! than a row — and none of the fields they read is one that table reads.
//!
//! **A name is required of three of the six and optional on a fourth.** Offering,
//! reissuing and removing are addressed to one person and have lost their subject
//! without one. Choosing what may be asked for is a decision about the *household*,
//! and naming somebody narrows it to them — so a choice with nobody named is a
//! request in its own right rather than an omission. The two that rule on one waiting
//! request name a number instead, because a household asks for the same film twice
//! under two spellings and a decision matched on words could rule on the wrong one.

use lemonfiber_core::app::{Allowance, Answer, Chosen, Command, Decision};
use lemonfiber_core::asking::Policy;
use lemonfiber_core::ports::service::{Quota, Unrated};

use crate::actions::{Arguments, Refused};

/// The requests this file answers, which is every one addressed to somebody here.
///
/// A list rather than a match arm for the reason the tables next door are lists: it is
/// read from two places — the row that hands them over, and the reading below that
/// tells them apart — and two matches would be two answers to which requests these are.
const ADDRESSED: [&str; 6] = [
    "invite",
    "reissue",
    "remove",
    "household-allow",
    "household-approve",
    "household-decline",
];

/// Whether this action is one of them.
#[must_use]
pub(super) fn about_the_household(action: &str) -> bool {
    ADDRESSED.contains(&action)
}

/// The command one of them names, or why it names none.
///
/// Takes the carrier whole rather than the fields it needs, because which fields those
/// are is what this file is about: a signature naming ten of them would be a second
/// place to keep the answer, and the one place that already keeps it is the table of
/// which action takes which.
pub(super) fn asked_for(action: &str, given: Arguments) -> Result<Command, Refused> {
    let Arguments {
        name,
        confirm,
        libraries,
        age_limit,
        unrated,
        policy,
        requests,
        days,
        request,
        reason,
        ..
    } = given;
    match action {
        "invite" | "reissue" | "remove" => about_a_person(
            action,
            name,
            confirm,
            RawAllowance {
                libraries,
                age_limit,
                unrated,
            },
        ),
        "household-allow" => allowing(name, policy.as_deref(), requests, days),
        "household-approve" => deciding(action, request, Answer::LetThrough),
        // The last of the six. A refusal is the one decision that owes the person who
        // asked a sentence, so it is the one that cannot be made without one.
        _ => match reason {
            Some(reason) => deciding(action, request, Answer::TurnedDown { reason }),
            None => Err(missing(action, "reason")),
        },
    }
}

/// The command one of the three household actions names.
///
/// All are addressed to a member rather than to a form, a file or a service, and all are
/// refused the same way when nobody is named — so the refusal is written once.
///
/// What they may watch reaches only the one that makes an account. The other two are
/// given it and drop it, which they may because the carrier refuses it to them first:
/// a library, an age limit or a choice about unrated content named to a reissue or a
/// removal is turned away by name before this is reached.
fn about_a_person(
    action: &str,
    name: Option<String>,
    confirm: bool,
    allowance: RawAllowance,
) -> Result<Command, Refused> {
    let name = name.ok_or_else(|| Refused::Missing {
        action: action.to_owned(),
        argument: "name".to_owned(),
    })?;
    let allowance = Allowance {
        libraries: allowance.libraries,
        age_limit: allowance.age_limit,
        unrated: unrated(allowance.unrated.as_deref())?,
    };
    match action {
        "invite" => Ok(Command::Invite { name, allowance }),
        "reissue" => Ok(Command::Reissue { name }),
        _ => Ok(Command::Remove { name, confirm }),
    }
}

/// What an invitation was told to allow, as the three arguments carrying it.
///
/// Gathered rather than passed as three, because they are one decision taken at one
/// moment and because a function taking three of one request's arguments beside two of
/// another's is a signature a caller gets wrong silently.
struct RawAllowance {
    /// The libraries named, as the operator names them.
    libraries: Vec<String>,
    /// The age above which the media server holds things back.
    age_limit: Option<u32>,
    /// What is to happen to content the media server has no rating for, as written.
    unrated: Option<String>,
}

/// What a word about unrated content means, or why it means nothing.
///
/// Nothing given is nothing said, which leaves the choice to the default a restriction
/// carries. A word this build does not know is refused with the ones it does, rather
/// than falling to whichever answer is safer — a caller who wrote `allow` and meant it
/// must not be given `block` because of a spelling.
fn unrated(written: Option<&str>) -> Result<Option<Unrated>, Refused> {
    match written {
        None => Ok(None),
        Some("block") => Ok(Some(Unrated::HeldBack)),
        Some("allow") => Ok(Some(Unrated::LetThrough)),
        Some(other) => Err(Refused::Unrecognised {
            argument: "unrated".to_owned(),
            offered: format!("`{other}` is neither `block` nor `allow`"),
        }),
    }
}

/// What the household is to be allowed to ask for, or why the words name nothing.
///
/// The two numbers arrive as a pair or not at all: half a limit is not a limit, and one
/// half accepted alone would be a household held to a figure over no period, or to no
/// figure over one. Refused rather than completed with a length of this surface's
/// choosing, which is the same refusal the command line makes with `requires`.
fn allowing(
    member: Option<String>,
    policy: Option<&str>,
    requests: Option<u32>,
    days: Option<u32>,
) -> Result<Command, Refused> {
    let chosen = match policy {
        None => None,
        Some(written) => {
            Some(
                Policy::from_label(written).ok_or_else(|| Refused::Unrecognised {
                    argument: "policy".to_owned(),
                    offered: format!("try {}", Policy::labels()),
                })?,
            )
        }
    };
    let quota = match (requests, days) {
        (Some(requests), Some(days)) => Some(Quota { requests, days }),
        (None, None) => None,
        (None, Some(_)) => return Err(missing("household-allow", "requests")),
        (Some(_), None) => return Err(missing("household-allow", "days")),
    };
    Ok(Command::Allowing(Chosen {
        member,
        policy: chosen,
        quota,
    }))
}

/// Ruling on one waiting request, or why it rules on none.
///
/// The number is required for the reason a trace's term is: a decision with no request
/// has lost its subject, and answering it with every waiting request would rule on
/// things nobody mentioned.
fn deciding(action: &str, request: Option<i64>, answer: Answer) -> Result<Command, Refused> {
    let request = request.ok_or_else(|| missing(action, "request"))?;
    Ok(Command::Deciding(Decision { request, answer }))
}

/// The refusal for an argument an action cannot do without.
fn missing(action: &str, argument: &str) -> Refused {
    Refused::Missing {
        action: action.to_owned(),
        argument: argument.to_owned(),
    }
}
