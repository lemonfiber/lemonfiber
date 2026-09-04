//! The requests addressed to somebody who lives here.
//!
//! Apart from the table that names them because they are the half of it with nothing
//! to do with forms, files or services: an account offered, a password taken off, an
//! account taken away, and what the household may ask for. Each has to say what it
//! lacks before it can name a command, which is longer than a row.
//!
//! **A name is required of three of the four and optional on the fourth.** Offering,
//! reissuing and removing are addressed to one person and have lost their subject
//! without one. Choosing what may be asked for is a decision about the *household*,
//! and naming somebody narrows it to them — so a choice with nobody named is a
//! request in its own right rather than an omission.

use lemonfiber_core::app::{Allowance, Answer, Chosen, Command, Decision};
use lemonfiber_core::asking::Policy;
use lemonfiber_core::ports::service::{Quota, Unrated};

use crate::actions::Refused;

/// The command one of the three household actions names.
///
/// All are addressed to a member rather than to a form, a file or a service, and all are
/// refused the same way when nobody is named — so the refusal is written once.
///
/// What they may watch reaches only the one that makes an account. The other two are
/// given it and drop it, which they may because the carrier refuses it to them first:
/// a library, an age limit or a choice about unrated content named to a reissue or a
/// removal is turned away by name before this is reached.
pub(super) fn about_a_person(
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
pub(super) struct RawAllowance {
    /// The libraries named, as the operator names them.
    pub(super) libraries: Vec<String>,
    /// The age above which the media server holds things back.
    pub(super) age_limit: Option<u32>,
    /// What is to happen to content the media server has no rating for, as written.
    pub(super) unrated: Option<String>,
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
pub(super) fn allowing(
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
pub(super) fn deciding(
    action: &str,
    request: Option<i64>,
    answer: Answer,
) -> Result<Command, Refused> {
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
