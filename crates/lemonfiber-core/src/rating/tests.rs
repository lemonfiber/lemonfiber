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
    // Three, because that table puts three of its own names against thirteen and
    // the reading takes the first few rather than all of them.
    assert_eq!(
        there.holds_back.first(),
        Some(&"PG-13".to_owned()),
        "{there:?}"
    );
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
