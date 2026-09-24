//! What an age limit comes to in the certificates the media server itself names.
//!
//! [`crate::age_limit`] holds the number and the words for it. This holds the other
//! half: the certificates a household already recognises, read off the media server
//! rather than shipped, because **the table differs by country**. Driven against
//! `jellyfin/jellyfin:10.10.3`, `GET /Localization/ParentalRatings` answers with the
//! server's own certificates against the ages they are for, and the same read under a
//! different country is a different list — `U` at nought, `PG` at eight, `12A` at
//! twelve for the United Kingdom; `G` at nought, `PG` at ten, `PG-13` at thirteen,
//! `R` at seventeen for the United States.
//!
//! **A number alone is not what a parent chose.** Under a United States table a limit
//! of eighteen holds back nothing an American calls adult, because the highest
//! certificate below it is `R` at seventeen. An operator reading "nothing above about
//! 18" beside that table has been told something true and misleading at once. So a
//! limit is said as the certificates on either side of it: what it still allows, and
//! the first thing it holds back.
//!
//! **Where the server names nothing, a documented mapping stands in.** The steps
//! offered are a British ladder, and a server whose table names no certificate at all
//! would leave every one of them bare. The fallback is stated wherever it is used
//! rather than passed off as the server's own — see
//! [`.docs/architecture/parental-controls.md`](https://github.com/lemonfiber/lemonfiber/blob/main/.docs/architecture/parental-controls.md).

use serde::Serialize;

pub use crate::ports::service::Certificate;

/// What one age limit comes to, in the certificates named for it.
///
/// Both sides, because either alone misleads. What is allowed without what is held
/// back reads as a limit that stops nothing; what is held back without what is allowed
/// reads as a limit that stops everything.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Rated {
    /// The certificates at the highest age this limit still lets through.
    ///
    /// Empty where the table names nothing at or below the limit, which is a limit
    /// that lets nothing rated through at all.
    pub allows: Vec<String>,
    /// The certificates at the lowest age this limit holds back.
    ///
    /// Empty where the table names nothing above the limit, which is a limit that
    /// holds nothing rated back at all.
    pub holds_back: Vec<String>,
    /// Whether these came from lemonfiber's own mapping because the media server's
    /// table named no certificates.
    ///
    /// Carried rather than hidden: a certificate said to be this household's when it
    /// is this program's is the kind of claim a parent would act on.
    pub fell_back: bool,
}

/// The mapping used where the media server's own table names nothing.
///
/// One certificate per step [`crate::age_limit`] offers, so no step is ever bare. They
/// are the British ones, which is the ladder those steps were written against — read
/// off `jellyfin/jellyfin:10.10.3` under `GB` rather than recalled.
const FALLBACK: &[(u32, &str)] = &[(0, "U"), (7, "7+"), (12, "12A"), (15, "15"), (18, "18")];

/// Said after a reading whose names came from the mapping rather than the server.
///
/// A certificate said to be this household's when it is this program's is exactly the
/// claim a parent would act on, so it is attributed in the reading itself rather than
/// only in a document.
const NOT_THIS_SERVERS: &str = " (named from lemonfiber's own mapping, not this server's)";

/// How many certificates are named for one age.
///
/// A table can put a great many names against one age: the media server's own United
/// States table has twenty at thirteen, one for each combination of the letters it
/// qualifies `TV-PG` with. Naming all of them buries the age they are all for, and
/// naming the first few says the same thing — they are one rating.
const NAMED_AT_MOST: usize = 3;

/// What a limit comes to against this media server's own certificates.
#[must_use]
pub fn rated(named: &[Certificate], age: u32) -> Rated {
    let fell_back = named.is_empty();
    let table: Vec<(u32, &str)> = if fell_back {
        FALLBACK.to_vec()
    } else {
        named
            .iter()
            .map(|certificate| (certificate.age, certificate.name.as_str()))
            .collect()
    };

    let below = table
        .iter()
        .filter(|(at, _)| *at <= age)
        .map(|(at, _)| *at)
        .max();
    let above = table
        .iter()
        .filter(|(at, _)| *at > age)
        .map(|(at, _)| *at)
        .min();

    Rated {
        allows: at(&table, below),
        holds_back: at(&table, above),
        fell_back,
    }
}

/// The certificates a table names at one age, in the order it named them.
///
/// Nothing where there is no such age, which is what an empty side of a [`Rated`]
/// means: the table had nothing to say on that side of the limit.
fn at(table: &[(u32, &str)], age: Option<u32>) -> Vec<String> {
    let Some(age) = age else {
        return Vec::new();
    };
    table
        .iter()
        .filter(|(at, _)| *at == age)
        .map(|(_, name)| (*name).to_owned())
        .take(NAMED_AT_MOST)
        .collect()
}

/// How an age limit reads, in the words for the number and the certificates beside it.
///
/// The one sentence every surface says a limit in — the chooser that sets it and the
/// household list that reports it — because two surfaces naming one setting differently
/// is two surfaces disagreeing about it.
#[must_use]
pub fn reading(named: &[Certificate], age: Option<u32>) -> String {
    match age {
        None => crate::age_limit::reading(None),
        Some(age) => said(age, &rated(named, age)),
    }
}

/// The same sentence, from a reading already taken.
///
/// Apart from [`reading`] because the two callers hold different things: the surface
/// that *sets* a limit has the media server in front of it and the surface that
/// *reports* one has only what a report carried. Both say it in these words, which is
/// the point of there being one function — a household list naming a limit differently
/// from the chooser that set it is two surfaces disagreeing about one setting.
#[must_use]
pub fn said(age: u32, rated: &Rated) -> String {
    let allows =
        (!rated.allows.is_empty()).then(|| format!(" — allows {}", rated.allows.join(", ")));
    let holds_back = (!rated.holds_back.is_empty()).then(|| {
        format!(
            "{} holds back {}",
            if rated.allows.is_empty() { " —" } else { ";" },
            rated.holds_back.join(", ")
        )
    });
    // Always said where it applies, and it applies whenever the table was empty: the
    // mapping brackets every age on at least one side, so there is no reading from it
    // that carries no certificate to attribute.
    let attributed = rated.fell_back.then(|| NOT_THIS_SERVERS.to_owned());
    [
        Some(crate::age_limit::reading(Some(age))),
        allows,
        holds_back,
        attributed,
    ]
    .into_iter()
    .flatten()
    .collect()
}

#[cfg(test)]
mod tests;
