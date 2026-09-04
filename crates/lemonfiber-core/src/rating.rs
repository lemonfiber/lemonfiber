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
    let mut said = crate::age_limit::reading(Some(age));
    if !rated.allows.is_empty() {
        said.push_str(&format!(" — allows {}", rated.allows.join(", ")));
    }
    if !rated.holds_back.is_empty() {
        said.push_str(&format!(
            "{} holds back {}",
            if rated.allows.is_empty() { " —" } else { ";" },
            rated.holds_back.join(", ")
        ));
    }
    // Always said where it applies, and it applies whenever the table was empty: the
    // mapping brackets every age on at least one side, so there is no reading from it
    // that carries no certificate to attribute.
    if rated.fell_back {
        said.push_str(" (named from lemonfiber's own mapping, not this server's)");
    }
    said
}

#[cfg(test)]
mod tests {
    use super::{rated, reading, said, Certificate, FALLBACK, NAMED_AT_MOST};

    /// The media server's own table for the United States, as the pinned image
    /// answers it — trimmed to the certificates the assertions turn on.
    fn american() -> Vec<Certificate> {
        certificates(&[
            (0, "G"),
            (7, "TV-Y7"),
            (10, "PG"),
            (13, "PG-13"),
            (13, "TV-PG"),
            (13, "TV-PG-D"),
            (13, "TV-PG-L"),
            (17, "R"),
            (21, "21"),
        ])
    }

    /// The same read under the United Kingdom, which is a different list.
    fn british() -> Vec<Certificate> {
        certificates(&[
            (0, "U"),
            (7, "7+"),
            (8, "PG"),
            (12, "12A"),
            (15, "15"),
            (18, "18"),
            (1000, "R18"),
        ])
    }

    /// A table from the pairs the media server answers with.
    fn certificates(pairs: &[(u32, &str)]) -> Vec<Certificate> {
        pairs
            .iter()
            .map(|(age, name)| Certificate {
                name: (*name).to_owned(),
                age: *age,
            })
            .collect()
    }

    /// The defect the certificates exist to expose.
    ///
    /// Eighteen against an American table holds back nothing an American calls adult:
    /// the highest certificate below it is `R` at seventeen, and the first thing above
    /// it is a certificate almost nothing carries. An operator reading the number alone
    /// has been told something true and misleading at once.
    #[test]
    fn an_age_that_names_nothing_still_says_what_it_lets_through() {
        let meaning = rated(&american(), 18);

        assert_eq!(meaning.allows, vec!["R".to_owned()], "{meaning:?}");
        assert_eq!(meaning.holds_back, vec!["21".to_owned()], "{meaning:?}");
        assert!(!meaning.fell_back, "the server's own table was not used");
    }

    /// The same number against a different country is a different pair of answers.
    #[test]
    fn one_number_reads_differently_under_a_different_country() {
        let here = rated(&british(), 12);
        let there = rated(&american(), 12);

        assert_eq!(here.allows, vec!["12A".to_owned()], "{here:?}");
        assert_eq!(here.holds_back, vec!["15".to_owned()], "{here:?}");
        assert_eq!(there.allows, vec!["PG".to_owned()], "{there:?}");
        assert_eq!(there.holds_back, vec!["PG-13".to_owned()], "{there:?}");
    }

    /// A table naming a great many at one age names the first few and stops.
    #[test]
    fn one_age_wearing_many_names_is_not_read_out_in_full() {
        let meaning = rated(&american(), 12);

        assert_eq!(meaning.holds_back.len(), NAMED_AT_MOST, "{meaning:?}");
        assert_eq!(
            meaning.holds_back.first(),
            Some(&"PG-13".to_owned()),
            "the order the server gave them was not kept: {meaning:?}"
        );
    }

    /// A server that names no certificates falls back, and says it fell back.
    #[test]
    fn a_server_naming_nothing_falls_back_to_the_documented_mapping() {
        let meaning = rated(&[], 12);

        assert!(meaning.fell_back, "{meaning:?}");
        assert_eq!(meaning.allows, vec!["12A".to_owned()], "{meaning:?}");
        assert_eq!(meaning.holds_back, vec!["15".to_owned()], "{meaning:?}");
    }

    /// Every step the surfaces offer is named by the mapping, or a step would read as
    /// a bare number on a server whose own table said nothing.
    #[test]
    fn the_mapping_names_every_step_the_surfaces_offer() {
        for step in crate::age_limit::steps() {
            assert!(
                FALLBACK.iter().any(|(age, _)| *age == step.age),
                "the mapping names nothing at {}",
                step.age
            );
        }
    }

    /// A limit above everything the table names holds nothing back, and says so by
    /// naming nothing on that side rather than by inventing a certificate.
    #[test]
    fn a_limit_above_everything_named_holds_nothing_back() {
        let meaning = rated(&british(), 2000);

        assert_eq!(meaning.allows, vec!["R18".to_owned()], "{meaning:?}");
        assert!(meaning.holds_back.is_empty(), "{meaning:?}");
    }

    /// A limit below everything the table names lets nothing rated through.
    #[test]
    fn a_limit_below_everything_named_lets_nothing_rated_through() {
        let table = certificates(&[(7, "7+"), (12, "12A")]);

        let meaning = rated(&table, 0);

        assert!(meaning.allows.is_empty(), "{meaning:?}");
        assert_eq!(meaning.holds_back, vec!["7+".to_owned()], "{meaning:?}");
    }

    /// No limit reads as the words for no limit, with no certificates hung off it.
    #[test]
    fn no_limit_reads_as_the_words_for_no_limit() {
        assert_eq!(reading(&british(), None), "anything");
    }

    /// A limit reads as the words for the number and the certificates on either side.
    #[test]
    fn a_limit_reads_as_its_words_and_the_certificates_around_it() {
        let said = reading(&american(), Some(12));

        assert!(said.starts_with("nothing above about 12"), "{said}");
        assert!(said.contains("allows PG"), "{said}");
        assert!(said.contains("holds back PG-13"), "{said}");
        assert!(
            !said.contains("lemonfiber's own mapping"),
            "the server's own table was claimed as a fallback: {said}"
        );
    }

    /// A reading from the mapping says it is the mapping's.
    ///
    /// A certificate said to be this household's when it is this program's is the kind
    /// of claim a parent would act on.
    #[test]
    fn a_reading_from_the_mapping_says_where_the_names_came_from() {
        let said = reading(&[], Some(15));

        assert!(said.contains("lemonfiber's own mapping"), "{said}");
        assert!(said.contains("allows 15"), "{said}");
    }

    /// A limit with nothing to allow still says what it holds back, on its own.
    #[test]
    fn a_limit_with_nothing_below_it_still_says_what_it_holds_back() {
        let table = certificates(&[(12, "12A")]);

        let said = reading(&table, Some(0));

        assert!(said.contains("only what suits everyone"), "{said}");
        assert!(said.contains("— holds back 12A"), "{said}");
    }

    /// A limit past the top of the table says what it allows and stops there.
    #[test]
    fn a_limit_past_the_top_of_the_table_names_only_what_it_allows() {
        let words = reading(&british(), Some(2000));

        assert_eq!(words, "nothing above about 2000 — allows R18", "{words}");
    }

    /// A reading taken and a reading said come to the same sentence.
    ///
    /// The surface that sets a limit has the media server in front of it and the one
    /// that reports a limit has only what the report carried, and the two saying it
    /// differently is the disagreement having one function is for.
    #[test]
    fn a_reading_already_taken_says_what_taking_it_again_would_say() {
        let table = british();

        assert_eq!(reading(&table, Some(12)), said(12, &rated(&table, 12)));
    }
}
