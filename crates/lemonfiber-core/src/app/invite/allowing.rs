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
use crate::ports::service::{Allowed, Household as _, NamedLibrary, Unrated};

/// What the person being invited is to be allowed, or nothing where nothing was chosen.
///
/// Nothing rather than an open [`Allowed`], because the two are different requests. An
/// offer that named neither a library nor a limit is asking for an account and saying
/// nothing about access, and writing "every library, no limit" for it would put an
/// account somebody is being offered again back to open — undoing whatever the
/// household had narrowed it to, and undoing it silently.
pub(super) async fn allowing(
    server: &crate::jellyfin::Jellyfin,
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
    server: &crate::jellyfin::Jellyfin,
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
        crate::error::Code::new("INVITE-6"),
        crate::error::Severity::Error,
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
        crate::error::Code::new("INVITE-7"),
        crate::error::Severity::Error,
        format!("this media server holds no library called {named}, so nobody was invited"),
        "Libraries are named the way the media server's own screens name them, though \
         not necessarily in the same capitalisation",
        crate::error::Remedy::new("Name a library the media server holds")
            .with_detail(there.join(", ")),
    )
}

/// Said where the account was made and what it may watch could not be written on it.
///
/// Said as an account that exists and is open, because that is what is now true. An
/// operator told only that something failed would not know whether to invite again or
/// to go and narrow an account that is already there.
pub(super) fn would_not_allow(name: &str) -> crate::error::Problem {
    crate::error::Problem::new(
        crate::error::Code::new("INVITE-8"),
        crate::error::Severity::Error,
        format!("{name} has an account, but the media server would not set what it may watch"),
        "The account exists and is open — every library, no age limit — so it is not an \
         invitation to send on until that is put right",
        crate::error::Remedy::new(
            "Run this again with the same choices, or set them in the media server's own \
             settings",
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::{no_libraries_read, no_such_library, would_not_allow};
    use crate::ports::service::NamedLibrary;

    /// A library nobody holds is refused by the name that was typed, with the ones
    /// there are named beside it.
    #[test]
    fn a_library_nobody_holds_is_refused_with_the_ones_there_are_named() {
        let held = vec![
            NamedLibrary {
                id: "aa".to_owned(),
                name: "Films".to_owned(),
            },
            NamedLibrary {
                id: "bb".to_owned(),
                name: "Shows".to_owned(),
            },
        ];
        let problem = no_such_library("Musicals", &held);

        assert!(problem.summary.contains("Musicals"), "{problem:?}");
        let said = where_to_look(&problem);
        assert!(said.contains("Films") && said.contains("Shows"), "{said}");
    }

    /// The media server refusing to say what it holds is said as a read that did not
    /// answer rather than as a library that is not there.
    #[test]
    fn a_library_list_that_would_not_answer_is_not_a_library_that_is_missing() {
        let problem = no_libraries_read();

        assert!(problem.summary.contains("libraries"), "{problem:?}");
        assert!(
            !problem.summary.contains("no library called"),
            "an unreadable list was said as a missing library: {problem:?}"
        );
    }

    /// An account made and then not narrowed says both halves: that it exists, and
    /// that it is open.
    #[test]
    fn an_account_that_could_not_be_narrowed_says_it_is_open() {
        let problem = would_not_allow("ana");

        assert!(problem.summary.contains("ana"), "{problem:?}");
        assert!(problem.meaning.contains("open"), "{problem:?}");
    }

    /// Where a refusal's first remedy points, which is where it puts the words
    /// somebody could have typed instead.
    fn where_to_look(problem: &crate::error::Problem) -> String {
        problem
            .remedies
            .first()
            .and_then(|remedy| remedy.detail.clone())
            .unwrap_or_default()
    }
}
