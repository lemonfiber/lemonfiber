use super::EVERY;

/// The layout's own source, read at compile time.
///
/// The layout is the list of places lemonfiber writes. Reading it is what makes
/// the disclosure hold: a place added there and not here is a thing kept on
/// somebody's machine that nothing tells them about, and it is exactly the kind
/// of addition nobody thinks to mention.
const LAYOUT: &str = include_str!("../../config/paths.rs");

/// Every location the layout names, by the accessor that names it.
///
/// The accessors that answer with a path of their own. The two that answer with
/// the directories those sit under are the roots, and they are listed as roots
/// rather than as things kept.
///
/// One line at a time, which is what it can see: an accessor whose signature was
/// wrapped across two lines would be invisible here, and the floor asserted below
/// catches a reader that has stopped working rather than one that missed a single
/// place. Every signature in that file fits on one line today and the formatter
/// keeps it that way; a longer one is the shape to watch for.
fn located() -> Vec<&'static str> {
    LAYOUT
        .lines()
        .filter_map(|line| line.trim().strip_prefix("pub fn "))
        .filter(|rest| rest.contains("(&self) -> PathBuf {"))
        .filter_map(|rest| rest.split_once('(').map(|(name, _)| name))
        .collect()
}

#[test]
fn every_place_the_layout_names_is_disclosed() {
    let places = located();
    let counted = places.len();
    assert!(
        counted > 10,
        "the layout was read as naming {counted} places, which means this is reading the \
         wrong file and is about to agree with itself"
    );
    let disclosed: Vec<&str> = EVERY.iter().map(|entry| entry.accessor).collect();
    let unsaid: Vec<&&str> = places
        .iter()
        .filter(|place| !disclosed.contains(place))
        .collect();
    assert!(
        unsaid.is_empty(),
        "lemonfiber writes these and nothing tells the operator they are there: {unsaid:?}"
    );
}

/// And the other direction, which is not the same test: an entry for a place the
/// layout no longer has would tell somebody about a file that is not on their
/// machine, and send them looking for it.
#[test]
fn nothing_is_disclosed_that_the_layout_no_longer_names() {
    let places = located();
    let gone: Vec<&str> = EVERY
        .iter()
        .map(|entry| entry.accessor)
        .filter(|accessor| !places.contains(accessor))
        .collect();
    assert!(
        gone.is_empty(),
        "these are disclosed as kept and the layout names no such place: {gone:?}"
    );
}

#[test]
fn every_entry_is_named_something_and_says_why_it_is_kept() {
    let silent: Vec<&str> = EVERY
        .iter()
        .filter(|entry| {
            entry.what.split_whitespace().count() < 2 || entry.why.split_whitespace().count() < 8
        })
        .map(|entry| entry.accessor)
        .collect();
    assert!(
        silent.is_empty(),
        "these are disclosed and the disclosure says nothing: {silent:?}"
    );
}
