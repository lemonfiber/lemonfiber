//! What somebody being invited may watch, chosen while they are being invited.
//!
//! Apart from the offer itself because it is a different question. [`super`] decides
//! who has an account and who has stopped having one; this decides what one account may
//! open and how far up the ratings it may go — and it is asked at the same moment for
//! the reason it exists at all: an account made open and narrowed afterwards is open
//! for as long as it takes anybody to remember, and the person most likely to be given
//! a limit has already been handed the address.
//!
//! **One translation happens here, and only one.** The operator names libraries the
//! way the media server's own screens name them and that server keeps them by an
//! identifier nobody reads, so the names are resolved here, once, and a name that
//! matches nothing is a refusal in the operator's words rather than the server's. The
//! age limit needs no translating: what the server keeps is an age, which is the same
//! thing the operator chose.
//!
//! **The libraries are resolved before an account exists.** A refusal after the account
//! is made leaves somebody holding an open account they were meant to be given a narrow
//! one, which is the failure worth designing out — so the names are matched before the
//! account is made, in a rehearsal too, and `--dry-run` refuses what the real run would
//! refuse.

use crate::app::Allowance;
use crate::ports::service::{Allowed, NamedLibrary, Unrated};

/// What the person being invited is to be allowed, or nothing where nothing was chosen.
///
/// Nothing rather than an open [`Allowed`], because the two are different requests. An
/// offer that named neither a library nor a limit is asking for an account and saying
/// nothing about access, and writing "every library, no limit" for it would put an
/// account somebody is being offered again back to open — undoing whatever the
/// household had narrowed it to, and undoing it silently.
pub(crate) async fn allowing(
    server: &dyn crate::ports::service::Household,
    allowance: &Allowance,
) -> Result<Option<Allowed>, Box<crate::error::Problem>> {
    if allowance.libraries.is_empty() && allowance.age_limit.is_none() {
        return Ok(None);
    }
    Ok(Some(Allowed {
        // Naming no library is saying nothing about libraries rather than asking for
        // all of them. An offer that set only an age limit must leave what an account
        // already opens alone, and an account being made has every library anyway.
        libraries: chosen(server, &allowance.libraries).await?,
        age_limit: allowance.age_limit,
        unrated: Some(unrated(allowance)),
    }))
}

/// What is to happen to content the media server has no rating for.
///
/// **Held back unless the operator said otherwise, and only on somebody being
/// restricted.** A great deal of content carries no rating at all, and a rating limit
/// cannot decide about a thing it has no rating for — so the choice has to be made, and
/// the conservative one is the one to make for a person somebody has just decided to
/// narrow. The cost is stated rather than hidden: some legitimate content becomes
/// invisible to them, which is why what was applied travels back on the answer.
///
/// An offer that narrows nothing never reaches here, because nothing is written at all.
const fn unrated(allowance: &Allowance) -> Unrated {
    match allowance.unrated {
        Some(chosen) => chosen,
        None => Unrated::HeldBack,
    }
}

/// The libraries named, by the identifiers the media server tells them apart by.
///
/// Matched without regard to case, for the reason a member's name is: a library called
/// `Films` typed as `films` is the same library, and refusing it would be refusing
/// somebody for their shift key.
async fn chosen(
    server: &dyn crate::ports::service::Household,
    named: &[String],
) -> Result<Option<Vec<String>>, Box<crate::error::Problem>> {
    if named.is_empty() {
        return Ok(None);
    }
    let Ok(held) = server.libraries().await else {
        return Err(Box::new(no_libraries_read()));
    };
    let mut chosen = Vec::new();
    for name in named {
        let asked = name.trim().to_lowercase();
        let Some(library) = held
            .iter()
            .find(|library| library.name.to_lowercase() == asked)
        else {
            return Err(Box::new(no_such_library(name, &held)));
        };
        chosen.push(library.id.clone());
    }
    Ok(Some(chosen))
}

/// Said where the media server would not say what libraries it holds.
///
/// Refused rather than read as no libraries at all: a name matched against an empty
/// list is a name that could not be found, and the operator would be told their library
/// does not exist when what happened is that nobody could ask.
fn no_libraries_read() -> crate::error::Problem {
    crate::error::Problem::new(
        crate::error::codes::invite::NO_LIBRARIES_READ,
        "the media server would not say what libraries it holds, so nobody was invited",
        "Choosing which libraries somebody may open starts by finding them, and that \
         read did not answer",
        crate::error::Remedy::new("Check the media server is running, then run this again"),
    )
}

/// Said where no library goes by a name that was given.
///
/// The ones there are, named: the fix is one word, and the words are already in hand.
fn no_such_library(named: &str, held: &[NamedLibrary]) -> crate::error::Problem {
    let there: Vec<&str> = held.iter().map(|library| library.name.as_str()).collect();
    crate::error::Problem::new(
        crate::error::codes::invite::NO_SUCH_LIBRARY,
        format!("this media server holds no library called {named}, so nobody was invited"),
        "Libraries are named the way the media server's own screens name them, though \
         not necessarily in the same capitalisation",
        crate::error::Remedy::new("Name a library the media server holds")
            .with_detail(there.join(", ")),
    )
}

/// Said where what the account may watch could not be written on it.
///
/// A new account is taken back rather than left open, so the offer is refused whole; an
/// existing one keeps what it already had. Said as which of the two is now true, because
/// an operator told only that something failed would not know whether an open account is
/// standing somewhere.
pub(crate) fn would_not_allow(name: &str, new: bool) -> crate::error::Problem {
    let meaning = if new {
        "The account was taken back rather than left open to every library, so there is \
         no invitation to send"
    } else {
        "Their account is still there, allowed what it was allowed before"
    };
    crate::error::Problem::new(
        crate::error::codes::invite::WOULD_NOT_ALLOW,
        format!("the media server would not set what {name} may watch"),
        meaning,
        crate::error::Remedy::new(
            "Run this again with the same choices, or set them in the media server's own \
             settings",
        ),
    )
}

#[cfg(test)]
mod tests;
