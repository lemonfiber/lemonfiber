//! What an install writes to disk, and how every write is journalled.

use super::*;

/// The install puts the plugin's own wiring where the stack reads it: a
/// configuration directory for the service, and one Compose document naming the
/// container lemonfiber writes.
#[tokio::test]
async fn installing_writes_the_service_s_directory_and_the_document_that_mounts_it() {
    let ctx = ctx("writes");
    assert_eq!(
        counted(installing(&ctx, &source("writes", MANIFEST)).await),
        Some(1)
    );

    let stack = stack_of(&ctx);
    assert!(stack.join("config/komga").is_dir());
    assert!(stack.join("compose/plugins/komga.yml").is_file());
}

/// What lands on disk is the container the record derives, not a second rendering
/// of it. Two renderings are free to disagree, and the one Compose reads is the
/// one on disk — so a plugin could be shown one entry and run another.
#[tokio::test]
async fn the_document_on_disk_is_the_container_the_record_derives() {
    let ctx = ctx("derives");
    let shown = report(installing(&ctx, &source("derives", MANIFEST)).await);
    let recorded = shown
        .and_then(|one| one.install)
        .map(|one| crate::plugin::written(&one.would))
        .unwrap_or_default();

    let written = std::fs::read_to_string(stack_of(&ctx).join("compose/plugins/komga.yml"))
        .unwrap_or_default();
    assert_eq!(written, recorded);
    assert!(written.contains("profiles:\n    - plugin-komga\n"));
}

/// The whole of what makes a plugin's changes ordinary: they are in the record
/// every other change is in, named as the plugin rather than as lemonfiber, and
/// stamped as one run so a reversal can ask for exactly them.
#[tokio::test]
async fn every_write_is_journalled_under_the_plugin_s_own_name_as_one_run() {
    let ctx = ctx("journalled");
    assert_eq!(
        counted(installing(&ctx, &source("journalled", MANIFEST)).await),
        Some(1)
    );

    let changes = journalled(&ctx);
    assert!(!changes.is_empty(), "the install journalled what it wrote");
    assert!(changes.iter().all(|change| change.operation == "komga"));

    let stamps: BTreeSet<&str> = changes.iter().map(|change| change.at.as_str()).collect();
    assert_eq!(stamps.len(), 1, "one install is one run");
}

/// Both writes are on the record, and the directory is recorded before the
/// document that mounts it — so a reversal walking backwards removes the document
/// first and never a directory something still names.
#[tokio::test]
async fn the_directory_is_journalled_before_the_document_that_mounts_it() {
    let ctx = ctx("ordered");
    assert_eq!(
        counted(installing(&ctx, &source("ordered", MANIFEST)).await),
        Some(1)
    );

    let made = made_paths(&ctx);
    let directory = made.iter().position(|path| path.ends_with("config/komga"));
    let document = made
        .iter()
        .position(|path| path.ends_with("compose/plugins/komga.yml"));
    assert!(
        directory.is_some() && document.is_some(),
        "both are recorded"
    );
    assert!(directory < document);
}

/// The surface an operator actually reads. A plugin's changes are in the history
/// with every other change, named as the plugin, and each carries the rollback
/// layer's own verdict — which is what makes a plugin's change an ordinary change
/// rather than a thing with an account of its own.
#[tokio::test]
async fn a_plugin_s_changes_are_in_the_history_named_as_the_plugin() {
    let ctx = ctx("in-history");
    assert_eq!(
        counted(installing(&ctx, &source("in-history", MANIFEST)).await),
        Some(1)
    );

    let shown = crate::app::history::history(&ctx);
    assert!(!shown.changes.is_empty());
    assert!(shown.changes.iter().all(|one| one.operation == "komga"));
    assert!(shown
        .changes
        .iter()
        .all(|one| one.reversal == crate::rollback::Reversal::Whole));
    assert!(shown
        .changes
        .iter()
        .any(|one| one.did.starts_with("made ") && one.did.ends_with("komga.yml")));
}

/// A plugin's changes are classified by the rollback layer like any other change,
/// which for a path lemonfiber made is reversible in full. Asked of the layer
/// itself rather than asserted here, so this stays true the day that judgement
/// changes.
#[tokio::test]
async fn what_an_install_journalled_is_judged_by_the_rollback_layer_like_anything_else() {
    let ctx = ctx("judged");
    assert_eq!(
        counted(installing(&ctx, &source("judged", MANIFEST)).await),
        Some(1)
    );

    let changes = journalled(&ctx);
    assert!(changes.iter().all(|change| {
        crate::rollback::standing(change, &[], &|_| None, &|_| None).reversal
            == crate::rollback::Reversal::Whole
    }));
}

/// A rehearsal settles everything and leaves the machine exactly as it was — no
/// directory, no document, and nothing on the record to put back.
#[tokio::test]
async fn a_rehearsed_install_writes_no_wiring_and_journals_nothing() {
    let ctx = rehearsing("rehearsed-wiring");
    let at = source("rehearsed-wiring", MANIFEST);
    assert_eq!(counted(installing(&ctx, &at).await), Some(0));

    let stack = stack_of(&ctx);
    assert!(!stack.join("config/komga").exists());
    assert!(!stack.join("compose/plugins/komga.yml").exists());
    assert!(journalled(&ctx).is_empty());
}

/// A directory the operator already had is theirs. The install uses it and does
/// not record it, so putting the install back never removes something it found
/// rather than made.
#[tokio::test]
async fn a_directory_that_was_already_there_is_used_and_not_recorded() {
    let ctx = ctx("already-there");
    let existing = stack_of(&ctx).join("config/komga");
    assert!(std::fs::create_dir_all(&existing).is_ok());

    assert_eq!(
        counted(installing(&ctx, &source("already-there", MANIFEST)).await),
        Some(1)
    );

    let made = made_paths(&ctx);
    assert!(
        !made.iter().any(|path| Path::new(path) == existing),
        "a directory the install found is not one it made"
    );
    assert!(made
        .iter()
        .any(|path| path.ends_with("compose/plugins/komga.yml")));
}

/// Shown refusing before it is relied on. With the recording taken out, the run
/// still writes — so the assertions above are about the record being kept rather
/// than about the writes happening to succeed.
#[tokio::test]
async fn the_record_is_what_the_assertions_above_turn_on() {
    let untouched = ctx("turns-on-second");
    assert!(
        journalled(&untouched).is_empty(),
        "a machine that installed nothing has no record"
    );

    let installed = ctx("turns-on");
    assert_eq!(
        counted(installing(&installed, &source("turns-on", MANIFEST)).await),
        Some(1)
    );
    assert!(!journalled(&installed).is_empty());
}

/// A stack to write into is not enough on its own: the record of what was written
/// lives beside the settings, and a machine that cannot say where those are has
/// nowhere to journal to. Refused rather than written unrecorded, because an
/// unrecorded write is the one that cannot be put back.
#[tokio::test]
async fn a_machine_that_cannot_say_where_its_own_files_are_refuses_the_install() {
    let ctx = a_context()
        .settings(crate::config::Settings {
            env_file: None,
            stack_dir: Some(
                lemonfiber_fixtures::scratch::Scratch::named("unrooted")
                    .kept()
                    .join("stack"),
            ),
            ..crate::config::Settings::default()
        })
        .build();
    assert!(!refusal(installing(&ctx, &source("unrooted", MANIFEST)).await).is_empty());
    assert_ne!(
        refusal(installing(&ctx, &source("unrooted", MANIFEST)).await),
        "PLUGIN-6",
        "a machine with a stack but no home for its records is not a machine \
             with nowhere to put a container"
    );
}

/// A write that cannot land stops the install where it is rather than carrying
/// on. What it had already written is on the record, which is what makes the
/// half-done state recoverable rather than a mystery.
#[tokio::test]
async fn a_write_that_cannot_land_stops_the_install_and_leaves_what_it_wrote_on_the_record() {
    let ctx = ctx("blocked");
    let occupied = stack_of(&ctx).join("config/komga");
    let _ = occupied.parent().map(std::fs::create_dir_all);
    // A file where the service's configuration directory has to go, so making
    // that directory cannot succeed.
    assert!(std::fs::write(&occupied, "not a directory").is_ok());

    assert_eq!(
        refusal(installing(&ctx, &source("blocked", MANIFEST)).await),
        "PLUGIN-7"
    );
    assert!(
        !stack_of(&ctx).join("compose/plugins/komga.yml").exists(),
        "the document is not written once a write ahead of it failed"
    );
}

/// The directory a document is written into is made on the way to writing it, so
/// a stack where that cannot happen stops the install in the same words a
/// service's own directory does. Driven through a file standing where the
/// document's directory has to go, which is the one arrangement that fails the
/// making without failing anything before it.
#[tokio::test]
async fn a_document_whose_directory_cannot_be_made_stops_the_install() {
    let ctx = ctx("no-room");
    let overlays = stack_of(&ctx).join("compose");
    let _ = overlays.parent().map(std::fs::create_dir_all);
    assert!(std::fs::write(&overlays, "not a directory").is_ok());

    assert_eq!(
        refusal(installing(&ctx, &source("no-room", MANIFEST)).await),
        "PLUGIN-7"
    );
}

/// And a document that cannot be written where its directory is fine is the same
/// refusal rather than the settings store's. The writer is shared with the
/// settings file, and its own failure says *your settings could not be saved* —
/// which about a plugin's Compose document names the wrong file and offers the
/// wrong remedy.
#[tokio::test]
async fn a_document_that_will_not_take_the_write_is_refused_as_the_install_s_own() {
    let ctx = ctx("unwritable-document");
    let document = stack_of(&ctx).join("compose/plugins/komga.yml");
    // A directory where the document has to go: its own directory is made, and
    // the write into it cannot succeed.
    assert!(std::fs::create_dir_all(&document).is_ok());

    assert_eq!(
        refusal(installing(&ctx, &source("unwritable-document", MANIFEST)).await),
        "PLUGIN-7"
    );
}

/// A leftover document is overwritten rather than recorded. It is derived from
/// the record and holds nothing an earlier run is owed, so recording it as made
/// would be the one journal entry that removes a file this run did not create.
#[tokio::test]
async fn a_leftover_document_is_overwritten_and_not_recorded_as_made() {
    let ctx = ctx("leftover");
    let document = stack_of(&ctx).join("compose/plugins/komga.yml");
    let _ = document.parent().map(std::fs::create_dir_all);
    assert!(std::fs::write(&document, "services: {}\n").is_ok());

    assert_eq!(
        counted(installing(&ctx, &source("leftover", MANIFEST)).await),
        Some(1)
    );
    assert!(std::fs::read_to_string(&document)
        .unwrap_or_default()
        .contains("profiles:\n    - plugin-komga\n"));
    assert!(
        !made_paths(&ctx)
            .iter()
            .any(|path| Path::new(path) == document),
        "a document this run found is not one it made"
    );
}

/// The register is written last, and a run that cannot write it puts the whole
/// install back. Everything ahead of that write held, so the only thing between
/// this run and an installed plugin is one file that would not be written — and
/// a container running with nothing recording it is the state the order exists to
/// avoid, not one to leave somebody to find.
///
/// Driven through a register that can be read and not rewritten, because that is
/// the one arrangement in which everything ahead of the last write succeeds.
#[tokio::test]
async fn a_register_that_cannot_be_written_puts_the_whole_install_back() {
    let ctx = ctx("unrecordable");
    let register = ctx
        .settings
        .env_file
        .as_deref()
        .map(|env| env.with_file_name(PLUGINS))
        .unwrap_or_default();
    assert!(std::fs::write(&register, "{}").is_ok());
    assert!(unrewritable(&register));

    let (code, said) = refused(installing(&ctx, &source("unrecordable", MANIFEST)).await);
    assert_eq!(code, "PLUGIN-8");
    assert!(
        said.contains("was put back"),
        "what it left is read off the reversal: {said}"
    );
    assert!(
        !stack_of(&ctx).join("compose/plugins/komga.yml").exists(),
        "the document it wrote is gone again"
    );
    assert!(
        made_paths(&ctx)
            .iter()
            .any(|path| path.ends_with("compose/plugins/komga.yml")),
        "and the change record still says it was there, so the run can be read"
    );

    let _ = std::fs::remove_dir_all(staging_of(&register));
}
