use std::path::Path;

use include_dir::{include_dir, Dir};

use super::{within, Source};

/// An app of three files, standing in for one a build would embed.
///
/// The real directory is a submodule that does not exist yet, so what is held
/// still here is the mechanism: a fixture reaches every branch the shipped
/// app would, and the day the submodule arrives nothing below this changes.
static EMBEDDED: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/tests/fixtures/frontend");

/// A directory that is certainly not an app.
static NOT_AN_APP: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/tests/fixtures/future-schema");

/// The same fixture, read from disk instead of from the binary.
fn on_disk() -> Source {
    Source::External(Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/frontend"
    )))
}

/// Both readings of the same app, so every rule is proven against each.
fn both() -> [Source; 2] {
    [Source::Embedded(&EMBEDDED), on_disk()]
}

/// What a path answers with, as text — empty where it answers with nothing.
fn read(source: Source, asked: &str) -> String {
    source.asset(asked).map_or_else(String::new, |asset| {
        String::from_utf8_lossy(&asset.bytes).into_owned()
    })
}

#[test]
fn a_named_file_is_the_file_that_was_named() {
    for source in both() {
        assert!(read(source, "/assets/app.js").contains("the app"));
        assert!(read(source, "/assets/app.css").contains("body"));
    }
}

#[test]
fn the_root_is_the_app() {
    for source in both() {
        assert!(read(source, "/").contains("<!doctype html>"));
        assert!(read(source, "").contains("<!doctype html>"));
    }
}

#[test]
fn a_route_the_app_reads_for_itself_is_the_app() {
    // The browser's own router reads this once the page is loaded, so the
    // page is what has to arrive for it to read anything at all.
    for source in both() {
        assert!(read(source, "/services/sonarr").contains("<!doctype html>"));
    }
}

#[test]
fn a_named_file_that_is_not_here_is_not_the_app_instead() {
    // A script asked for a stylesheet; handing it a document would be a
    // wrong answer where an absent one was owed.
    for source in both() {
        assert_eq!(source.asset("/assets/missing.css"), None);
    }
}

#[test]
fn a_path_climbing_out_of_the_app_is_refused() {
    for source in both() {
        assert_eq!(source.asset("/../../Cargo.toml"), None);
        assert_eq!(source.asset("../Cargo.toml"), None);
        assert_eq!(source.asset("/assets/../../Cargo.toml"), None);
    }
}

#[test]
fn a_path_written_with_the_other_separator_is_refused() {
    for source in both() {
        assert_eq!(source.asset(r"..\Cargo.toml"), None);
    }
}

#[test]
fn a_path_that_only_marks_where_it_is_reaches_the_same_file() {
    for source in both() {
        assert!(read(source, "/./assets/./app.js").contains("the app"));
    }
}

#[test]
fn the_two_readings_of_one_app_agree() {
    for asked in ["/", "/assets/app.js", "/assets/app.css", "/services/sonarr"] {
        assert_eq!(
            read(Source::Embedded(&EMBEDDED), asked),
            read(on_disk(), asked),
            "the same app read two ways is the same app: {asked}"
        );
    }
}

#[test]
fn a_directory_holding_an_index_holds_an_app() {
    for source in both() {
        assert!(source.holds_an_app());
    }
}

#[test]
fn a_directory_holding_no_index_holds_no_app() {
    assert!(!Source::Embedded(&NOT_AN_APP).holds_an_app());
    assert!(!Source::External(Path::new("/lemonfiber/no/such/app")).holds_an_app());
}

#[test]
fn a_build_carrying_no_app_answers_nothing_rather_than_something_else() {
    let empty = Source::Embedded(&NOT_AN_APP);
    assert_eq!(empty.asset("/"), None);
    assert_eq!(empty.asset("/assets/app.js"), None);
}

#[test]
fn a_path_of_nothing_but_separators_is_still_the_app() {
    assert_eq!(within("///").as_deref(), Some(Path::new("index.html")));
}
