use std::path::Path;

use include_dir::{include_dir, Dir};

use super::{Failure, Source};
use crate::error::Diagnose;
use crate::error::{Severity, State};
use crate::test_support::frozen_day as today;

/// The same stack the binary embeds, so both variants are exercised against
/// the real thing rather than against a fixture that could drift from it.
static EMBEDDED: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../assets/media-stack");

/// A stack whose compose file splits the data root, read as the stack lemonfiber
/// ships. The same directory is read as an operator's own below, which is the
/// pairing the two tests about it exist for: one set of bytes, two answers, and
/// the difference is whose stack it is.
static SPLIT_MOUNTS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/tests/fixtures/split-mounts");

/// A directory that is certainly not a stack.
///
/// `adapters` because the crate cannot compile without it. The previous choice was a
/// directory that later moved out to its own crate, and an emptied directory fails
/// here rather than where it was emptied.
static NOT_A_STACK: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/src/config");

/// A stack naming a lemonfiber far newer than any this build could be.
static NEEDS_NEWER: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/tests/fixtures/stack-needs-newer");

/// The stack this repository carries as a submodule, read from disk.
fn checked_out() -> Source {
    Source::External(Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/media-stack"
    )))
}

#[test]
fn reads_a_manifest_compiled_into_the_binary() {
    let read = Source::Embedded(&EMBEDDED)
        .manifest()
        .ok()
        .map(|manifest| (manifest.schema_version, manifest.services.len()));
    assert_eq!(read, Some((1, 22)));
}

#[test]
fn the_embedded_and_external_readings_agree() {
    assert_eq!(
        Source::Embedded(&EMBEDDED).manifest_text().ok(),
        checked_out().manifest_text().ok(),
        "the same stack read two ways is the same stack"
    );
}

#[test]
fn an_embedded_directory_with_no_manifest_is_a_broken_build() {
    let refusal = Source::Embedded(&NOT_A_STACK).manifest().err();
    assert!(matches!(refusal, Some(Failure::NotEmbedded)));
}

#[test]
fn reads_a_manifest_from_a_directory() {
    let read = checked_out()
        .manifest()
        .ok()
        .map(|manifest| (manifest.schema_version, manifest.services.len()));
    assert_eq!(read, Some((1, 22)));
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
fn an_embedded_stack_is_written_where_compose_can_read_it() {
    let dir = scratch("materialise");
    let written = Source::Embedded(&EMBEDDED).materialise(Some(&dir));

    assert_eq!(written.ok().as_deref(), Some(dir.path()));
    assert!(dir.join("stack.toml").is_file(), "the manifest is written");
    assert!(
        dir.join("compose.yml").is_file(),
        "so are the compose files"
    );
    assert!(
        dir.join("compose").join("tv.yml").is_file(),
        "including the fragments, which the root file includes by path"
    );

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

#[test]
fn a_build_that_lost_its_stack_admits_it_rather_than_guessing() {
    let problem = Failure::NotEmbedded.problem();
    assert_eq!(problem.severity, Severity::Critical);
    assert_eq!(problem.state, State::Unknown);
    assert!(!problem.remedies.is_empty(), "escalation is still offered");
}

/// One of every way a stack fails to be read.
fn every_failure() -> [Failure; 10] {
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
            names: vec!["service jellyfin: api.kind: unknown variant `plex`".to_owned()],
        },
        Failure::NotEmbedded,
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
            failure.problem().amiss,
            crate::wiring::UNREAD_AMISS,
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
api = { kind = \"plex\", key_source = \"config-xml\" }
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

#[test]
fn the_stack_we_ship_obeys_its_own_single_mount_rule() {
    // The rule is invisible to every probe — from the host the data root is
    // one filesystem and links work perfectly — so the only thing that can
    // hold the shipped stack to it is this.
    assert_eq!(refusal(Source::Embedded(&EMBEDDED)), "");
    assert!(
        Source::Embedded(&EMBEDDED).crowded_mounts().is_empty(),
        "and the reading the refusal is built from agrees"
    );
}

/// The fixture above, read the way an operator's own stack directory is read.
fn forked() -> Source {
    Source::External(Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/split-mounts"
    )))
}

#[test]
fn a_shipped_stack_that_splits_the_data_root_is_refused() {
    // Two mounts beneath the data root put the download and the library on
    // opposite sides of a filesystem boundary inside the container, and every
    // import silently becomes a copy. In the stack this binary carries, that is
    // the binary being wrong about its own contents: nobody chose it, and nobody
    // running it could put it right.
    //
    // Asserted on what the operator is actually shown, rather than on the shape
    // of the error behind it.
    let said = refusal(Source::Embedded(&SPLIT_MOUNTS));
    assert!(said.contains("sonarr"), "{said}");
    assert!(said.contains("copied rather than hardlinked"), "{said}");
}

#[test]
fn the_same_stack_as_an_operator_s_own_is_reported_rather_than_refused() {
    // The same bytes, read as a directory the operator pointed at. A rule of
    // lemonfiber's is guidance over somebody else's stack: refusing to operate it
    // would put this tool between an operator and their own system over a cost
    // that is theirs to carry, so what it costs is reported and the stack runs.
    // Read once rather than inside the message an assertion would only format on
    // its way to failing: a call in there is a line no passing run enters.
    let refused = refusal(forked());
    assert!(
        forked().checked_manifest(today()).is_ok(),
        "a fork was refused over a rule of ours: {refused}"
    );

    let found = forked().crowded_mounts();
    assert_eq!(
        found
            .iter()
            .map(|one| one.service.as_str())
            .collect::<Vec<_>>(),
        vec!["sonarr"],
        "the reading itself still happens, and still names the service"
    );
    let said = found.first().map(ToString::to_string).unwrap_or_default();
    assert!(said.contains("copied rather than hardlinked"), "{said}");
}

#[test]
fn a_link_back_to_an_ancestor_is_not_walked_into() {
    // A stack directory is the operator's own, and a link inside it is theirs
    // to make. Following one back to an ancestor would walk for ever, on every
    // command — so the entry's own type decides, and a link is not a directory.
    let dir = lemonfiber_fixtures::scratch::Scratch::named("loop");
    let _ = std::fs::remove_dir_all(&dir);
    let inner = dir.join("compose");
    assert!(std::fs::create_dir_all(&inner).is_ok());
    assert!(std::fs::write(inner.join("tv.yml"), "services: {}\n").is_ok());
    // Where the platform makes one cheaply. Its absence changes no branch: the
    // walk reads the entry's type either way.
    #[cfg(unix)]
    let _ = std::os::unix::fs::symlink(&dir, inner.join("back"));

    let read = super::on_disk(&dir);
    assert_eq!(read.len(), 1, "the real file, once: {read:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_directory_that_cannot_be_read_contributes_nothing_rather_than_stopping() {
    // Whether this is a stack at all is the manifest's business, and it has
    // already been read by the time this runs. A path that is not a directory
    // to walk simply has no compose files in it.
    assert!(super::on_disk(Path::new("/lemonfiber-no-such-directory")).is_empty());
}

/// What an operator's stack is run from is its own files and the overlay over it, and
/// never the documents lemonfiber writes for its plugins.
#[test]
fn a_stack_is_run_from_its_own_files_and_the_overlay_and_not_the_plugins() {
    let dir = written("run-from", "");
    for (path, text) in [
        ("compose.yml", "services:\n  theirs: {}\n"),
        ("compose/plugins/komga.yml", "services:\n  komga: {}\n"),
    ] {
        let at = dir.join(path);
        assert!(at
            .parent()
            .is_some_and(|up| std::fs::create_dir_all(up).is_ok()));
        assert!(std::fs::write(&at, text).is_ok());
    }
    let overlay = dir.with_extension("override.yml");
    assert!(std::fs::write(&overlay, "services:\n  watchtower: {}\n").is_ok());

    let read: Vec<String> = Source::External(dir)
        .run_from(&[overlay.clone(), dir.join("no-such-overlay.yml")])
        .into_iter()
        .map(|(_, text)| text)
        .collect();

    assert_eq!(
        read,
        [
            "services:\n  theirs: {}\n".to_owned(),
            "services:\n  watchtower: {}\n".to_owned()
        ]
    );
    assert!(!Source::Embedded(&EMBEDDED).run_from(&[]).is_empty());
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

/// The embedded stack's service files are read from where the build put them.
#[test]
fn the_embedded_stack_assembles_from_its_own_files() {
    let text = Source::Embedded(&EMBEDDED)
        .manifest_text()
        .unwrap_or_default();
    assert!(
        text.contains("\n# services/"),
        "the service files were not joined"
    );
}
