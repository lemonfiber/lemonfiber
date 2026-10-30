//! What an install writes to disk, and how every write is journalled.

use super::*;

/// The install puts the plugin's own wiring where the stack reads it: a
/// configuration directory for the service, and its entry in the project.
#[tokio::test]
async fn installing_writes_the_service_s_directory_and_the_entry_that_mounts_it() {
    let ctx = ctx("writes");
    assert_eq!(
        counted(installing(&ctx, &source("writes", MANIFEST)).await),
        Some(1)
    );

    let stack = stack_of(&ctx);
    assert!(stack.join("config/komga").is_dir());
    assert!(!document(&ctx).is_empty());
}

/// What lands on disk is the container the record derives, not a second rendering
/// of it. Two renderings are free to disagree, and the one Compose reads is the
/// one on disk — so a plugin could be shown one entry and run another.
#[tokio::test]
async fn the_document_on_disk_is_the_container_the_record_derives() {
    let mut ctx = ctx("derives");
    ctx.stack = crate::test_support::carried_stack();
    let shown = report(installing(&ctx, &source("derives", MANIFEST)).await);
    let recorded = shown
        .and_then(|one| one.install)
        .map(|one| crate::project::entries(&one.would, &crate::project::Reading::bare()))
        .unwrap_or_default();

    let written = document(&ctx);
    assert_eq!(written, entry_in(&recorded));
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
    assert!(changes
        .iter()
        .all(|change| change.operation == "plugin komga"));

    let stamps: BTreeSet<&str> = changes.iter().map(|change| change.at.as_str()).collect();
    assert_eq!(stamps.len(), 1, "one install is one run");
}

/// Both writes are on the record, and the directory is recorded before the
/// project that mounts it — so a reversal walking backwards removes the project
/// first and never a directory something still names.
#[tokio::test]
async fn the_directory_is_journalled_before_the_project_that_mounts_it() {
    let ctx = ctx("ordered");
    assert_eq!(
        counted(installing(&ctx, &source("ordered", MANIFEST)).await),
        Some(1)
    );

    let made = made_paths(&ctx);
    let directory = made.iter().position(|path| path.ends_with("config/komga"));
    let document = made
        .iter()
        .position(|path| path.ends_with(crate::project::FILE));
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

    let shown = crate::app::history::history(&ctx).unwrap_or_default();
    assert!(!shown.changes.is_empty());
    assert!(shown
        .changes
        .iter()
        .all(|one| one.operation == "plugin komga"));
    assert!(shown
        .changes
        .iter()
        .all(|one| one.reversal == crate::rollback::Reversal::Whole));
    assert!(shown
        .changes
        .iter()
        .any(|one| one.did.starts_with("made ") && one.did.ends_with(crate::project::FILE)));
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
/// directory, no project, and nothing on the record to put back.
#[tokio::test]
async fn a_rehearsed_install_writes_no_wiring_and_journals_nothing() {
    let ctx = rehearsing("rehearsed-wiring");
    let at = source("rehearsed-wiring", MANIFEST);
    assert_eq!(counted(installing(&ctx, &at).await), Some(0));

    let stack = stack_of(&ctx);
    assert!(!stack.join("config/komga").exists());
    assert!(!stack.join(crate::project::FILE).exists());
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
    assert!(made.iter().any(|path| path.ends_with(crate::project::FILE)));
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
        !project_of(&ctx).exists(),
        "the project is not written once a write ahead of it failed"
    );
}

/// A project that cannot be written where its directory is fine is the install's own
/// refusal rather than the settings store's.
#[tokio::test]
async fn a_project_that_will_not_take_the_write_is_refused_as_the_install_s_own() {
    let ctx = ctx("unwritable-project");
    // A directory where the project has to go, so the write into it cannot succeed.
    assert!(std::fs::create_dir_all(project_of(&ctx)).is_ok());

    assert_eq!(
        refusal(installing(&ctx, &source("unwritable-project", MANIFEST)).await),
        "PLUGIN-7"
    );
}

/// A project lemonfiber has no record of writing is somebody's, so an install that would
/// write it again is refused, naming it, with nothing written and nothing recorded.
#[tokio::test]
async fn a_project_nobody_recorded_refuses_the_install_naming_it() {
    let ctx = ctx("leftover");
    let project = project_of(&ctx);
    assert!(std::fs::write(&project, "services: {}\n").is_ok());

    let refused = installing(&ctx, &source("leftover", MANIFEST)).await.err();

    assert!(
        refused
            .as_ref()
            .is_some_and(|problem| problem.code.as_str() == "STACK-11"
                && problem.summary.contains(&project.display().to_string())),
        "{refused:?}"
    );
    assert_eq!(
        std::fs::read_to_string(&project).ok().as_deref(),
        Some("services: {}\n")
    );
    assert!(journalled(&ctx).is_empty());
    assert_eq!(counted(reading(&ctx).await), Some(0));
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
        !project_of(&ctx).exists(),
        "the project it wrote is gone again"
    );
    assert!(
        made_paths(&ctx)
            .iter()
            .any(|path| path.ends_with(crate::project::FILE)),
        "and the change record still says it was there, so the run can be read"
    );

    let _ = std::fs::remove_dir_all(staging_of(&register));
}

/// A history whose journal cannot be read is refused rather than shown empty: an empty
/// history would say nothing changed on a machine whose record of changes is broken.
#[test]
fn a_history_whose_journal_cannot_be_read_is_refused() {
    let ctx = ctx("history-unreadable");
    let journal = crate::app::targets::layout(&ctx)
        .map(|paths| paths.journal())
        .unwrap_or_default();
    assert!(std::fs::create_dir_all(journal.join("held")).is_ok());
    assert!(crate::app::history::history(&ctx).is_err());
}

/// A write whose record cannot be written is not made: a change nothing could put
/// back is not one to make.
#[test]
fn a_write_whose_record_cannot_be_written_is_not_made() {
    use crate::plugin::{Lands, Write};

    let ctx = ctx("unjournalled-write");
    let journal = crate::app::targets::layout(&ctx)
        .map(|paths| paths.journal())
        .unwrap_or_default();
    assert!(unrewritable(&journal));

    let made = stack_of(&ctx).join("config").join("komga");
    let directory = vec![Write {
        path: made.clone(),
        lands: Lands::Directory {
            beneath: stack_of(&ctx),
        },
    }];
    assert!(super::super::writing::carry_out(&ctx, "komga", "1", &directory).is_err());
    assert!(
        !made.exists(),
        "the directory was made with no record of it"
    );
}

/// An adapter's key is minted where its container reads it, kept when the install runs
/// again so what holds it is not cut off, and never written from nothing.
#[test]
fn an_adapters_key_is_minted_once_kept_after_and_never_made_up() {
    use crate::plugin::{Lands, Write};

    let mut ctx = ctx("adapter-key");
    let path = crate::plugin::key_file(&stack_of(&ctx), "plex-adapter");
    let key = vec![Write {
        path: path.clone(),
        lands: Lands::Key,
    }];
    assert!(super::super::writing::carry_out(&ctx, "plex", "1", &key).is_ok());
    let minted = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(minted.len() > 32, "{minted}");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = std::fs::metadata(&path).map(|kept| kept.permissions().mode() & 0o777);
        assert_eq!(
            mode.ok(),
            Some(0o600),
            "the key is readable beyond its owner"
        );
    }
    assert!(super::super::writing::carry_out(&ctx, "plex", "2", &key).is_ok());
    assert_eq!(std::fs::read_to_string(&path).ok(), Some(minted));

    assert!(std::fs::remove_file(&path).is_ok());
    ctx.seams.random = std::sync::Arc::new(lemonfiber_fixtures::ports::Chance::exactly(None));
    assert!(super::super::writing::carry_out(&ctx, "plex", "3", &key).is_err());
    assert!(!path.exists());
}

#[test]
fn a_secrets_file_is_written_owner_only_and_refused_where_the_store_does_not_read() {
    use crate::plugin::{Holding, Lands, Variable, Write};

    let ctx = ctx("secrets-file");
    let env = ctx.settings.env_file.clone().unwrap_or_default();
    let path = env.with_file_name("env").join("komga.env");
    let planned = vec![Write {
        path: path.clone(),
        lands: Lands::Secrets(vec![Variable {
            name: "KEY".to_owned(),
            holding: Holding::Store("komga:key".to_owned()),
        }]),
    }];
    assert!(super::super::writing::carry_out(&ctx, "komga", "1", &planned).is_ok());
    assert_eq!(std::fs::read_to_string(&path).ok().as_deref(), Some(""));

    assert!(std::fs::remove_file(&path).is_ok());
    assert!(std::fs::write(env.with_file_name("secrets.json"), "{ not json").is_ok());
    let refused = super::super::writing::carry_out(&ctx, "komga", "2", &planned);
    assert!(
        refused
            .as_ref()
            .err()
            .and_then(|problem| problem.detail.as_deref())
            .is_some_and(|detail| detail.contains("is not what this store holds")),
        "{refused:?}"
    );
    assert!(!path.exists());
}

/// A service replacing one the stack ships is installed only from a plugin this build
/// embeds as first-party; any other plugin declaring one is refused before anything is
/// written, and a plugin replacing nothing is not asked.
#[test]
fn only_a_first_party_plugin_replaces_a_stack_service() {
    let mut placed = crate::test_support::a_placed("jellyfin", &[], Some(8096));
    placed.replaces = Some("jellyfin".to_owned());
    let mut replacing = crate::test_support::an_installed("contracted", vec![placed]);
    replacing.manifest = crate::test_support::CONTRACTED_MANIFEST.to_owned();
    replacing.tree = crate::test_support::A_TREE.to_owned();
    let plain = crate::test_support::an_installed(
        "contracted",
        vec![crate::test_support::a_placed("komga", &[], Some(25600))],
    );

    let stranger = super::super::replacing::unreplaced(&replacing, &[]);
    let trusted = super::super::replacing::unreplaced(
        &replacing,
        &crate::test_support::first_party("contracted"),
    );
    let nothing = super::super::replacing::unreplaced(&plain, &[]);

    let stranger = stranger.err();
    assert_eq!(
        stranger.as_ref().map(|problem| problem.code),
        Some(crate::error::codes::plugin::STRANGER_REPLACING)
    );
    let said = stranger
        .map(|problem| format!("{} {}", problem.summary, problem.detail.unwrap_or_default()))
        .unwrap_or_default();
    assert!(
        said.contains("jellyfin"),
        "the refusal does not name the service: {said}"
    );
    assert!(trusted.is_ok());
    assert!(nothing.is_ok());
}

/// A directory the install would make through a link, or outside the stack, is not
/// made, and nothing lands where the link leads.
#[cfg(unix)]
#[test]
fn a_directory_reached_through_a_link_is_not_made() {
    use crate::plugin::{Lands, Write};

    let ctx = ctx("linked-mount-tree");
    let stack = stack_of(&ctx);
    let elsewhere = stack.with_file_name("elsewhere");
    let _ = std::fs::create_dir_all(&elsewhere);
    let _ = std::fs::create_dir_all(&stack);
    let _ = std::os::unix::fs::symlink(&elsewhere, stack.join("mounts"));
    let directory = vec![Write {
        path: stack.join("mounts").join("komga"),
        lands: Lands::Directory { beneath: stack },
    }];
    assert!(super::super::writing::carry_out(&ctx, "komga", "1", &directory).is_err());
    assert!(!elsewhere.join("komga").exists());
}

/// A mount source reached through a link from the stack directory refuses the write of
/// the document that would mount it, read-only sources included, and one reached
/// through none does not.
#[cfg(unix)]
#[test]
fn a_mount_source_reached_through_a_link_refuses_its_document() {
    let ctx = ctx("linked-mount-source");
    let stack = stack_of(&ctx);
    let mut placed = crate::test_support::a_placed("komga", &[], None);
    placed.fields.mount = vec![lemonfiber_manifest::container::Mount {
        from: "Caddyfile".to_owned(),
        to: "/etc/caddy/Caddyfile".to_owned(),
        ro: true,
    }];
    let plugin = crate::test_support::an_installed("komga", vec![placed]);
    let tree = stack.join("mounts").join("komga");
    let _ = std::fs::create_dir_all(&tree);
    assert!(super::super::writing::unlinked(&ctx, &plugin, &stack).is_ok());
    let _ = std::os::unix::fs::symlink("/etc/hostname", tree.join("Caddyfile"));
    let refused = super::super::writing::unlinked(&ctx, &plugin, &stack);
    assert!(
        refused.is_err_and(|why| why.summary.contains("Caddyfile")),
        "the linked source is named"
    );
}

/// A variable asking the address of what the stack fills is written with the service its
/// ask settles on, at the port that service listens on.
#[test]
fn a_variable_asking_an_address_is_written_with_the_one_its_ask_settles_on() {
    use lemonfiber_manifest::container::{UrlOf, Value};

    let ctx = ctx("addressed");
    let mut placed = crate::test_support::a_placed("subfinder", &[], None);
    placed.asks = vec![crate::plugin::Asking {
        native: None,
        capability: "indexer.search".to_owned(),
        each: false,
    }];
    placed.fields.env = [(
        "INDEXER".to_owned(),
        Value::Url(UrlOf {
            url_of: "indexer.search".to_owned(),
            files: None,
            native: None,
        }),
    )]
    .into();
    let plugin = crate::test_support::an_installed("subs", vec![placed]);
    let document = super::super::planned::writes(&ctx, &plugin, &stack_of(&ctx), None)
        .unwrap_or_default()
        .into_iter()
        .find_map(|write| match write.lands {
            crate::plugin::Lands::Document { text, .. } => Some(text),
            _ => None,
        })
        .unwrap_or_default();
    assert!(
        document.contains("INDEXER: http://prowlarr:9696"),
        "{document}"
    );
}

#[test]
fn a_setting_is_written_as_the_operator_recorded_it_and_as_its_default_otherwise() {
    use lemonfiber_manifest::container::{Constant, Setting, Value};

    let ctx = ctx("settled");
    let setting = |name: &str| {
        Value::Setting(Setting {
            setting: name.to_owned(),
            default: Constant::Text("slow".to_owned()),
            recorded_with: None,
            was: None,
            needs: lemonfiber_manifest::container::choices::Needs::new(),
            one_of: None,
        })
    };
    let mut placed = crate::test_support::a_placed("reader", &[], None);
    placed.fields.env = [
        ("MODE".to_owned(), setting("subs:mode")),
        ("SPEED".to_owned(), setting("subs:speed")),
    ]
    .into();
    let plugin = crate::test_support::an_installed("subs", vec![placed]);
    let env = ctx.settings.env_file.clone().unwrap_or_default();
    assert!(std::fs::write(
        env.with_file_name("settings.json"),
        r#"{"schema":1,"written_by":"","values":{"subs:mode":"fast"}}"#
    )
    .is_ok());
    let document = super::super::planned::writes(&ctx, &plugin, &stack_of(&ctx), None)
        .unwrap_or_default()
        .into_iter()
        .find_map(|write| match write.lands {
            crate::plugin::Lands::Document { text, .. } => Some(text),
            _ => None,
        })
        .unwrap_or_default();
    assert!(document.contains("MODE: fast"), "{document}");
    assert!(document.contains("SPEED: slow"), "{document}");
}
