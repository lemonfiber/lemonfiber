use std::path::Path;

use include_dir::Dir;

use super::{Carried, Failure, Source};
use crate::error::Diagnose;
use crate::error::State;
use crate::test_support::frozen_day as today;

/// The same stack the binary embeds, so both variants are exercised against
/// the real thing rather than against a fixture that could drift from it.
use super::BUNDLED as EMBEDDED;

/// A stack naming a lemonfiber far newer than any this build could be.
static NEEDS_NEWER: Carried = Carried {
    manifest: include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/stack-needs-newer/stack.toml"
    )),
    files: &NOTHING,
};

/// No file beside a manifest.
static NOTHING: Dir<'_> = Dir::new("", &[]);

/// The stack this build carries, written as a stack directory holds it.
fn checked_out() -> Source {
    Source::External(lemonfiber_fixtures::stack::on_disk())
}

#[test]
fn reads_a_manifest_compiled_into_the_binary() {
    let read = Source::Embedded(&EMBEDDED)
        .manifest()
        .ok()
        .map(|manifest| (manifest.schema_version, manifest.services.len()));
    assert_eq!(read, Some((1, 23)));
}

#[test]
fn a_stack_directory_reads_as_the_embedded_stack_but_for_the_secrets_it_holds_none_of() {
    let mut read = checked_out()
        .manifest_text()
        .ok()
        .and_then(|text| toml::from_str::<toml::Table>(&text).ok())
        .unwrap_or_default();
    assert!(
        read.remove("include").is_some(),
        "a directory names its service files"
    );
    assert_eq!(read, lemonfiber_fixtures::stack::carried());
}

#[test]
fn reads_a_manifest_from_a_directory() {
    let read = checked_out()
        .manifest()
        .ok()
        .map(|manifest| (manifest.schema_version, manifest.services.len()));
    assert_eq!(read, Some((1, 23)));
}

#[test]
fn a_readable_manifest_from_another_generation_is_refused_as_such() {
    let future = Source::External(Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/future-schema"
    )));
    let refusal = future.manifest().err().map(|err| err.to_string());
    assert_eq!(
            refusal.as_deref(),
            Some("the stack manifest cannot be used: the manifest declares schema version 99, and this build reads [1]"),
            "read cleanly, and refused on the pairing rather than the syntax"
        );
}

#[test]
fn a_directory_with_no_manifest_shows_the_path_it_looked_for() {
    let missing = Source::External(Path::new("/lemonfiber/no/such/stack"));
    let refusal = missing.manifest().err().map(|err| err.to_string());
    assert_eq!(
        refusal
            .as_deref()
            .map(|message| message.contains("/lemonfiber/no/such/stack/stack.toml")),
        Some(true),
        "the path is shown in full: {refusal:?}"
    );
}

/// A directory of our own under the system temporary directory.
fn scratch(name: &str) -> lemonfiber_fixtures::scratch::Scratch {
    lemonfiber_fixtures::scratch::Scratch::unmade(name)
}

#[test]
fn an_embedded_stack_writes_its_starting_files_and_never_its_manifest() {
    let dir = scratch("materialise");
    let written = Source::Embedded(&EMBEDDED).materialise(Some(&dir));

    assert_eq!(written.ok().as_deref(), Some(dir.path()));
    assert!(
        dir.join("recordings").is_dir(),
        "the starting files are written"
    );
    assert!(
        !dir.join("stack.toml").exists(),
        "the manifest is never written"
    );
    assert!(!dir.join("services").exists(), "no service file is written");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn writing_the_stack_twice_is_the_same_as_writing_it_once() {
    let dir = scratch("materialise-twice");
    let first = Source::Embedded(&EMBEDDED).materialise(Some(&dir)).is_ok();
    let second = Source::Embedded(&EMBEDDED).materialise(Some(&dir)).is_ok();
    assert!(first && second, "an existing directory is written over");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_external_stack_is_left_exactly_where_it_is() {
    let path = Path::new("/opt/somebody/stack");
    assert_eq!(
        Source::External(path).materialise(None).ok().as_deref(),
        Some(path),
        "nothing is written, and no destination is needed"
    );
}

#[test]
fn an_embedded_stack_with_nowhere_to_go_says_setup_has_not_run() {
    let refusal = Source::Embedded(&EMBEDDED).materialise(None).err();
    assert!(matches!(refusal, Some(Failure::NowhereToWrite)));

    let problem = Failure::NowhereToWrite.problem();
    assert_eq!(
        problem.remedies.len(),
        2,
        "run setup, or bring your own stack"
    );
}

#[test]
fn a_destination_that_cannot_be_written_names_it() {
    // A file where a directory needs to be: the closest thing to a
    // permission failure that behaves the same way on every platform.
    let blocker = scratch("not-a-directory");
    if let Some(parent) = blocker.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&blocker, "in the way");

    let refusal = Source::Embedded(&EMBEDDED)
        .materialise(Some(&blocker.join("stack")))
        .err()
        .map(|err| err.to_string());
    assert_eq!(
        refusal
            .as_deref()
            .map(|m| m.contains("could not be written")),
        Some(true),
        "got: {refusal:?}"
    );

    let _ = std::fs::remove_file(&blocker);
}

#[test]
fn a_missing_stack_offers_both_ways_out() {
    let problem = Failure::Unreadable {
        path: "/tmp/nowhere/stack.toml".into(),
        reason: "No such file or directory".to_owned(),
    }
    .problem();
    assert_eq!(problem.state, State::Guided);
    assert_eq!(
        problem.remedies.len(),
        2,
        "point somewhere else, or stop pointing"
    );
    assert!(problem.summary.contains("/tmp/nowhere/stack.toml"));
}

#[test]
fn an_unusable_manifest_reads_as_a_pairing_rather_than_a_syntax_error() {
    let reason = "the manifest declares schema version 99, and this build reads [1]";
    let problem = Failure::Unusable {
        reason: reason.to_owned(),
    }
    .problem();
    assert!(problem.summary.contains("different version"));
    assert_eq!(problem.detail.as_deref(), Some(reason));
}

/// A stack needing a newer binary, one this build cannot read and one that will not
/// parse are three answers, and each says its own.
#[test]
fn a_binary_too_old_is_told_apart_from_a_generation_and_a_syntax_error() {
    let too_old = Failure::TooOld {
        required: "0.99.0".to_owned(),
        running: "0.17.0".to_owned(),
    }
    .problem();
    let unusable = Failure::Unusable {
        reason: "schema 99".to_owned(),
    }
    .problem();
    let malformed = Failure::Malformed {
        reason: "expected a value".to_owned(),
    }
    .problem();
    assert_eq!(too_old.code, crate::error::codes::stack::STACK_NEEDS_NEWER);
    assert_eq!(
        too_old.summary,
        "This stack needs lemonfiber 0.99.0 or newer"
    );
    assert!(too_old.meaning.contains("0.17.0"), "{}", too_old.meaning);
    let summaries = [&too_old.summary, &unusable.summary, &malformed.summary];
    assert!(summaries
        .iter()
        .all(|one| summaries.iter().filter(|other| other == &one).count() == 1));
    assert_ne!(too_old.code, unusable.code);
}

/// A stack directory naming a newer binary than this one is refused as one, naming
/// the version it asked for.
#[test]
fn a_stack_asking_for_a_newer_binary_is_refused_naming_it() {
    let refused = Source::Embedded(&NEEDS_NEWER).manifest();
    assert!(
        matches!(&refused, Err(Failure::TooOld { required, running })
            if required == "999.0.0" && running == env!("CARGO_PKG_VERSION")),
        "{refused:?}"
    );
}

/// One of every way a stack fails to be read.
fn every_failure() -> [Failure; 9] {
    [
        Failure::Unreadable {
            path: "/tmp/x/stack.toml".into(),
            reason: "denied".to_owned(),
        },
        Failure::Unusable {
            reason: "schema 99".to_owned(),
        },
        Failure::TooOld {
            required: "0.99.0".to_owned(),
            running: "0.17.0".to_owned(),
        },
        Failure::Malformed {
            reason: "expected a value".to_owned(),
        },
        Failure::Unrecognised {
            names: vec!["service jellyfin: health.kind: unknown variant plex".to_owned()],
        },
        Failure::NowhereToWrite,
        Failure::NotWritten {
            path: "/tmp/x".into(),
            reason: "denied".to_owned(),
        },
        Failure::Invalid {
            violations: vec!["service sonarr: port 8989 is published twice".to_owned()],
        },
        Failure::Unassembled {
            faults: vec!["include entry services/sonarr.toml: names no file".to_owned()],
        },
    ]
}

#[test]
fn every_failure_says_something_and_offers_something() {
    let failures = every_failure();
    for failure in &failures {
        assert!(!failure.to_string().is_empty());
        assert!(!failure.problem().remedies.is_empty());
    }
}

/// The published list is the list the failures raise, both ways: a code raised and not
/// listed is one no client can name, and one listed and never raised is a refusal a
/// client is told to expect and never meets.
#[test]
fn every_code_a_failure_raises_is_listed_and_every_listed_one_is_raised() {
    let raised: Vec<_> = every_failure()
        .iter()
        .map(|failure| failure.problem().code)
        .collect();
    for code in &raised {
        assert!(super::FAILURES.contains(code), "{code:?} is not listed");
    }
    for code in &super::FAILURES {
        assert!(raised.contains(code), "{code:?} is listed and never raised");
    }
    for failure in every_failure() {
        assert_eq!(
            failure.problem().status(),
            500,
            "{failure} is listed at the status a failure of the machine is answered with"
        );
    }
}

/// What checking a stack complained about, in the words the operator is
/// shown — empty where it complained about nothing.
fn refusal(source: Source) -> String {
    source
        .checked_manifest(today())
        .err()
        .map(|failure| failure.problem())
        .and_then(|problem| problem.detail)
        .unwrap_or_default()
}

/// The headline a stack's refusal is shown under.
fn headline(source: Source) -> String {
    source
        .checked_manifest(today())
        .err()
        .map(|failure| failure.problem().summary)
        .unwrap_or_default()
}

/// A stack directory holding the manifest `toml` describes, laid out as a stack is
/// written, for a single test.
fn written(named: &str, toml: &str) -> &'static Path {
    let dir = lemonfiber_fixtures::scratch::Scratch::named(named).kept();
    let _ = std::fs::remove_dir_all(&dir);
    lemonfiber_fixtures::stack::manifest_written(&dir, toml);
    // Leaked deliberately, as the split-mount fixture above is and for the same
    // reason: `Source::External` holds a `&'static Path`.
    Box::leak(dir.into_boxed_path())
}

/// A typo is a typo, and is not reported as a version the operator does not have.
///
/// Every way of failing to read a manifest arrived under one headline, and that
/// headline told an operator who had left out a quotation mark to go and find a
/// different build of lemonfiber. The file names the line it broke on; the answer
/// they need is on their own disk.
#[test]
fn a_stack_that_is_not_toml_is_not_blamed_on_the_version() {
    let said = headline(Source::External(written("typo", "version = \n")));
    assert!(
        !said.contains("different version"),
        "a plain syntax error was reported as a version mismatch: {said}"
    );
    assert!(said.contains("could not be read"), "{said}");
}

/// A name this build does not know is not a typo either, and says which names.
///
/// The manifest crate asks every declaration separately so a fork learns all of
/// its mistakes in one run. That list survives to the operator only if this
/// keeps it — flattened into a sentence about versions, the one pass was for
/// nothing.
#[test]
fn a_name_this_build_does_not_know_is_named_rather_than_called_a_typo() {
    let stack = "
schema_version = 1

[[service]]
id = \"jellyfin\"
health = { kind = \"plex\" }
";
    let problem = Source::External(written("unknown-name", stack))
        .checked_manifest(today())
        .err()
        .map(|failure| failure.problem());
    let said = problem
        .as_ref()
        .map(|problem| problem.summary.clone())
        .unwrap_or_default();
    let detail = problem
        .and_then(|problem| problem.detail)
        .unwrap_or_default();

    assert!(
        !said.contains("different version") && !said.contains("could not be read"),
        "an unknown name was reported as a version or a typo: {said}"
    );
    assert!(said.contains("names this build does not know"), "{said}");
    assert!(
        detail.contains("jellyfin") && detail.contains("plex"),
        "the operator was not told which name, or where: {detail}"
    );
}

/// A stack whose files are not laid out as the contract says is refused for that,
/// naming each file, and not as a manifest that contradicts itself.
#[test]
fn a_stack_whose_files_break_the_layout_names_each_one() {
    let dir = written("unassembled", "schema_version = 1\n");
    let _ = std::fs::write(dir.join("services").join("stray.toml"), "");
    let source = Source::External(dir);
    assert!(
        headline(source).contains("manifest files break the contract"),
        "{}",
        headline(source)
    );
    assert_eq!(
        refusal(source),
        "services/stray.toml: is in services/ and no include entry names it"
    );
}

/// The embedded stack's manifest is read whole, as the bundle assembles it, and its files
/// hold no manifest of their own.
#[test]
fn the_embedded_stack_reads_its_manifest_whole() {
    let text = Source::Embedded(&EMBEDDED)
        .manifest_text()
        .unwrap_or_default();
    assert_eq!(text, lemonfiber_bundled::STACK);
    assert!(EMBEDDED.files.get_file("stack.toml").is_none());
    assert!(EMBEDDED.files.get_dir("services").is_none());
}
