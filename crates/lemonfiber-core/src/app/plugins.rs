//! Installing somebody else's plugin, and reading back what is installed.
//!
//! Nothing here starts a container or asks a service anything. What it does is
//! settle what installing this manifest decides, write that down, and put the
//! plugin's own wiring where the stack reads it — the record being the half every
//! later step reads rather than re-deriving, because the manifest is the author's
//! file and may be gone tomorrow, and a run that re-read it would be answering a
//! question about a document rather than about the machine.
//!
//! **Every write is journalled before it is made, under the plugin's own name.** A
//! plugin's changes are not a second kind of change: they go in the record an apply
//! and a reconfigure go in, they are classified by the same rollback layer, and they
//! are put back by the same reversal. Nothing here implements undoing, and the day
//! removal arrives it will not either.
//!
//! **A manifest this build refuses is not installed.** The reader's verdict is total
//! — a non-conforming file yields no manifest at all — and the rules over values are
//! asked here in the same pass, so a plugin whose digest is not a digest or whose
//! configuration directory is the library is refused with every reason at once rather
//! than one per attempt.
//!
//! **The record is refused rather than defaulted.** Every other small record beside
//! the settings reads a damaged file as its default, and is right to: a forgotten
//! preference is asked for again and the cost is a question. This one is the only
//! memory that a stranger's service is on this machine, and reading it as empty would
//! report a stack with a plugin in it as a stack with none — then install a second
//! copy over the first without noticing.

use std::path::{Path, PathBuf};

use lemonfiber_ports::error::{Code, Problem, Remedy, Severity, State};

use crate::doctor::BUNDLED_CHECKS;
use crate::plugin::{Install, Installed, Installs, Register};

use super::{Ctx, Outcome};

// Carrying the writes out, and journalling each before it is made. Its own file
// because the deciding and the touching are two concerns, and only one of them has a
// disk under it.
mod writing;

use writing::{carry_out, nowhere_to_write};

/// What is asked about the plugins on this machine.
///
/// Beside the handler rather than in the command vocabulary, as an update's request
/// is: the shape of what may be asked and the code that answers it move together,
/// and a word added to one without the other does not compile.
///
/// Apart from the five documents a plugin *author* reads, which are generated at
/// build time and answer the same on a machine with nothing installed as on one
/// running everything — so nothing dispatches them and nothing needs a stack. These
/// two are about one operator's machine, so they arrive the way every other verb
/// does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Asked {
    /// Install the plugin whose source is at this path, and record what that
    /// decided.
    ///
    /// The path is the operator's, and it is the only argument: what an install
    /// writes is settled by the manifest rather than chosen at the command line, so
    /// there is no flag by which an operator could be talked into installing
    /// something on terms the manifest did not declare.
    Install {
        /// The plugin's source: its directory, or the `plugin.toml` inside it.
        path: PathBuf,
    },
    /// Say what is installed, and what each install decided.
    Installed,
}

/// The source names no plugin this build can read.
const UNREADABLE: Code = Code::new("PLUGIN-2");

/// The manifest is read and this build refuses what it declares.
const REFUSED: Code = Code::new("PLUGIN-3");

/// The record of what is installed cannot be read.
const UNRECORDED: Code = Code::new("PLUGIN-4");

/// The plugin is installed already.
const ALREADY: Code = Code::new("PLUGIN-5");

/// There is no stack on this machine to put a plugin's container in.
pub(super) const NOWHERE: Code = Code::new("PLUGIN-6");

/// A directory or a document the install decided on would not land.
pub(super) const UNWRITABLE: Code = Code::new("PLUGIN-7");

/// The wiring went down and the record of what is installed did not.
const UNRECORDABLE: Code = Code::new("PLUGIN-8");

/// What is installed, and what installing one came to.
///
/// One entry point for the reading and for the verb, because they answer one
/// question: somebody who has just installed something wants to see it among what
/// they had, and a rehearsal showing only the new entry would not say what it joins.
///
/// # Errors
///
/// Where the record cannot be read, where the source names no manifest this build
/// can read, where the manifest is refused, where the plugin is installed already,
/// where there is no stack to put its container in, where one of the writes would
/// not land, or where the record of what is installed cannot be written.
pub(super) fn asked(ctx: &Ctx, action: &Asked) -> Result<Outcome, Box<Problem>> {
    let held = read(ctx)?;
    match action {
        Asked::Installed => Ok(Outcome::Plugins(Installs {
            installed: held.installed().to_vec(),
            install: None,
        })),
        Asked::Install { path } => install(ctx, held, path),
    }
}

/// Settle what installing this source decides, write the plugin's wiring, and record
/// it.
///
/// Everything a rehearsal holds back is behind one branch, so what a rehearsal
/// reports is what the real run reports — settled by the same code, refused for the
/// same reasons, and stopping short of the writes rather than describing them
/// separately.
///
/// **The wiring goes down before the register, and the order is the safe one.** The
/// register is what says a plugin is installed and what layers its document into the
/// stack, so a run that wrote the wiring and then failed to record it leaves files
/// nothing reads — inert, on the change record, and removable. The other order would
/// leave a plugin the machine reports as installed with nothing behind it.
fn install(ctx: &Ctx, held: Register, path: &Path) -> Result<Outcome, Box<Problem>> {
    let manifest =
        crate::plugin::read(path).map_err(|unreadable| Box::new(unreadable_source(&unreadable)))?;
    let refusals = lemonfiber_plugin::refusals(&manifest, BUNDLED_CHECKS);
    if !refusals.is_empty() {
        return Err(Box::new(refused(&manifest.plugin.id, &refusals)));
    }

    let would = Installed::of(&manifest);
    let mut after = held.clone();
    after
        .record(would.clone())
        .map_err(|there| Box::new(already(&there)))?;

    let recorded = !ctx.dry_run;
    if recorded {
        // Where the writes land. Asked for only on the run that makes them: a
        // rehearsal on a machine that has never been set up can still say what
        // installing this would decide, and refusing it for want of somewhere to put
        // files it is not going to write would be refusing the wrong question.
        let stack = ctx
            .settings
            .stack_dir
            .as_deref()
            .ok_or_else(|| Box::new(nowhere_to_write(&would.plugin)))?;
        carry_out(ctx, &would.plugin, &crate::plugin::writes(&would, stack))?;
        // Answered for here rather than passed on. The record writer is shared and
        // says *your settings could not be saved, your existing settings are
        // untouched* — which after the line above is false twice over: the file is
        // not the settings, and the machine has been written to.
        super::record::keep(kept_at(ctx).as_deref(), &after)
            .map_err(|why| Box::new(unrecordable(&would.plugin, *why)))?;
    }

    // What the record holds, which after a rehearsal is what it held before. A
    // listing that counted the entry nobody wrote would report an install that did
    // not happen, in the same breath as saying nothing was written — and a reader
    // who believes the count over the sentence is the one this is written for.
    let standing = if recorded { after } else { held };

    Ok(Outcome::Plugins(Installs {
        installed: standing.installed().to_vec(),
        install: Some(Install { would, recorded }),
    }))
}

/// What the record holds, or why nothing can be said about it.
///
/// Three answers and never two. No file at all is a machine that has installed
/// nothing, which is an answer rather than a fault. A file that is there and cannot
/// be read — damaged, half-written, unreadable to this user — is refused, because
/// the alternative is telling an operator that nothing is installed while somebody
/// else's service is running.
fn read(ctx: &Ctx) -> Result<Register, Box<Problem>> {
    let Some(at) = kept_at(ctx) else {
        return Ok(Register::empty());
    };
    match std::fs::read_to_string(&at) {
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => Ok(Register::empty()),
        Err(why) => Err(Box::new(unrecorded(&at, &why.to_string()))),
        Ok(text) => {
            Register::parse(&text).map_err(|why| Box::new(unrecorded(&at, &why.to_string())))
        }
    }
}

/// Where the record is kept: beside the environment file, in the configuration
/// directory a backup captures, or nowhere when nothing is configured. Equal to
/// [`crate::config::paths::Paths::plugins`].
fn kept_at(ctx: &Ctx) -> Option<PathBuf> {
    super::targets::beside_env(ctx, crate::config::paths::PLUGINS)
}

/// Nothing at the path the operator named is a plugin this build can read.
fn unreadable_source(why: &crate::plugin::Unreadable) -> Problem {
    Problem::new(
        UNREADABLE,
        Severity::Error,
        "That is not a plugin lemonfiber can read",
        "Nothing was installed and nothing was written.",
        Remedy::new("Point at the plugin's directory, or the `plugin.toml` inside it"),
    )
    .in_state(State::Guided)
    .with_detail(why.to_string())
}

/// The manifest is readable and this build will not act on what it says.
///
/// Every reason at once, each placed where the author wrote it. An operator handed
/// one fault per attempt at somebody else's manifest is guessing at how many are
/// left.
fn refused(plugin: &str, found: &[lemonfiber_plugin::Violation]) -> Problem {
    let listed = found
        .iter()
        .map(std::string::ToString::to_string)
        .collect::<Vec<String>>()
        .join("; ");
    Problem::new(
        REFUSED,
        Severity::Error,
        format!("{plugin} declares things lemonfiber will not install"),
        "Nothing was installed and nothing was written. A manifest is refused whole, so none \
         of it was acted on.",
        Remedy::new("Read what each one says, and take it up with whoever published the plugin"),
    )
    .in_state(State::Guided)
    .with_detail(listed)
}

/// The record is there and cannot be read, which is not the same as empty.
fn unrecorded(at: &Path, why: &str) -> Problem {
    Problem::new(
        UNRECORDED,
        Severity::Error,
        format!(
            "The record of what is installed, at {}, could not be read",
            at.display()
        ),
        "Nothing was installed and nothing was written. lemonfiber will not report this machine \
         as having no plugins on the strength of a record it cannot read — a plugin's service \
         may be running, and installing over it would leave two.",
        Remedy::new("Restore the file from a backup, or move it aside if nothing is installed"),
    )
    .in_state(State::Guided)
    .with_detail(why.to_owned())
}

/// The wiring is on the machine and the record that says so could not be written.
///
/// Its own refusal rather than the record writer's, because the writer is shared with
/// the settings file and says *your settings could not be saved, your existing
/// settings are untouched*. Both halves are wrong here: the file is not the settings,
/// and the wiring is already on disk. What an operator needs is the second sentence —
/// that something was written, that it is inert until the record catches up, and that
/// the change record already holds it.
fn unrecordable(plugin: &str, why: Problem) -> Problem {
    Problem::new(
        UNRECORDABLE,
        Severity::Error,
        format!("{plugin} was written and could not be recorded as installed"),
        "Its wiring is on the machine and nothing reads it: the record of what is installed \
         is what layers a plugin into the stack, so what was written is inert rather than \
         half-running. Every one of those writes is in the change record, so `lemonfiber \
         history` says what is there and it can be put back.",
        Remedy::new("Check the permissions on the configuration directory, then install it again"),
    )
    .in_state(State::Guided)
    .caused_by(why)
}

/// This plugin is installed, so what was asked for is an update.
fn already(held: &crate::plugin::Already) -> Problem {
    Problem::new(
        ALREADY,
        Severity::Error,
        format!("{} is already installed", held.plugin),
        "Nothing was written. Installing over an installation is an update, which puts one set \
         of changes back before it applies another — doing it as an install would leave the \
         record describing one version and the machine carrying two.",
        Remedy::new(format!(
            "Remove {} first, or wait for the word that updates one",
            held.plugin
        )),
    )
    .in_state(State::Guided)
    .with_detail(format!("the record holds version {}", held.version))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::path::{Path, PathBuf};

    use super::{asked, Asked};
    use crate::app::{Ctx, Outcome};
    use crate::config::paths::PLUGINS;
    use crate::journal::Change;
    use crate::plugin::Installs;
    use crate::test_support::{a_context, a_password, env_at};

    /// A plugin's source, as one lands on an operator's disk.
    const MANIFEST: &str = r#"
schema_version = 1

[plugin]
id          = "komga"
name        = "Komga"
version     = "1.2.0"
description = "Reads your comics on any browser"
without_it  = "Files on disk, no way to read them"
upstream    = "https://example.invalid"
license     = "MIT"
forms       = ["library"]

[[service]]
id          = "komga"
name        = "Komga"
image       = "example.invalid/komga"
digest      = "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"
tag         = "1.11.0"
port        = 25600
bind        = "lan"
criticality = "important"
takes_data  = true
config_path = "/app/data"
"#;

    /// A context whose settings point at a scratch configuration directory, with a
    /// stack directory beside it for the install to write its wiring into.
    ///
    /// Both under one scratch root, so the layout a run resolves — the journal in the
    /// configuration directory, the stack under the data one — is the layout an
    /// installed machine has rather than an arrangement invented for the test.
    fn ctx(name: &str) -> Ctx {
        let env_file = env_at(name, &a_password());
        let stack = env_file.with_file_name("data").join("stack");
        a_context()
            .settings(crate::config::Settings {
                env_file: Some(env_file),
                stack_dir: Some(stack),
                ..crate::config::Settings::default()
            })
            .build()
    }

    /// Where a context writes the stack, which is what the install puts wiring under.
    fn stack_of(ctx: &Ctx) -> PathBuf {
        ctx.settings.stack_dir.clone().unwrap_or_default()
    }

    /// The same, rehearsing rather than writing.
    fn rehearsing(name: &str) -> Ctx {
        let mut ctx = ctx(name);
        ctx.dry_run = true;
        ctx
    }

    /// Where a context keeps the record.
    fn record_of(ctx: &Ctx) -> PathBuf {
        ctx.settings
            .env_file
            .as_deref()
            .map(|env| env.with_file_name(PLUGINS))
            .unwrap_or_default()
    }

    /// A plugin source written to a scratch directory.
    fn source(named: &str, manifest: &str) -> PathBuf {
        let at = std::env::temp_dir().join(format!("lemonfiber-installing-{named}"));
        let _ = std::fs::remove_dir_all(&at);
        let _ = std::fs::create_dir_all(&at);
        let _ = std::fs::write(at.join("plugin.toml"), manifest);
        at
    }

    /// What installing that source came to.
    fn installing(ctx: &Ctx, at: &Path) -> Result<Outcome, Box<crate::error::Problem>> {
        asked(
            ctx,
            &Asked::Install {
                path: at.to_path_buf(),
            },
        )
    }

    /// What the reading came to.
    fn reading(ctx: &Ctx) -> Result<Outcome, Box<crate::error::Problem>> {
        asked(ctx, &Asked::Installed)
    }

    /// The report an answer carries, or nothing where it was not one.
    fn report(outcome: Result<Outcome, Box<crate::error::Problem>>) -> Option<Installs> {
        match outcome {
            Ok(Outcome::Plugins(report)) => Some(report),
            _ => None,
        }
    }

    /// How many plugins the record holds, as the answer says.
    fn counted(outcome: Result<Outcome, Box<crate::error::Problem>>) -> Option<usize> {
        report(outcome).map(|one| one.installed.len())
    }

    /// The code a refusal carries, or nothing where the answer was not one.
    fn refusal(outcome: Result<Outcome, Box<crate::error::Problem>>) -> String {
        outcome
            .err()
            .map(|problem| problem.code.to_string())
            .unwrap_or_default()
    }

    /// The whole of the slice: what the manifest declared survives, and a later run
    /// reads it back without the manifest being anywhere near.
    #[test]
    fn where_a_service_keeps_its_configuration_survives_the_install_and_is_read_back() {
        let ctx = ctx("survives");
        let at = source("survives", MANIFEST);
        assert_eq!(counted(installing(&ctx, &at)), Some(1));

        // The author's file goes, as it may the moment an install is done.
        let _ = std::fs::remove_dir_all(&at);

        let path = report(reading(&ctx))
            .and_then(|report| report.installed.first().cloned())
            .and_then(|one| one.services.first().cloned())
            .map(|one| one.config_path);
        assert_eq!(path.as_deref(), Some("/app/data"));
    }

    #[test]
    fn a_machine_with_nothing_installed_answers_with_an_empty_list() {
        let read = report(reading(&ctx("empty")));
        assert_eq!(read.as_ref().map(|one| one.installed.len()), Some(0));
        assert_eq!(read.map(|one| one.install.is_none()), Some(true));
    }

    #[test]
    fn the_install_says_what_it_recorded_and_what_it_joined() {
        let ctx = ctx("recorded");
        let shown = report(installing(&ctx, &source("recorded", MANIFEST)));
        let install = shown.as_ref().and_then(|one| one.install.clone());
        assert_eq!(install.as_ref().map(|one| one.recorded), Some(true));
        assert_eq!(
            install.map(|one| one.would.plugin),
            Some("komga".to_owned())
        );
        assert_eq!(shown.map(|one| one.installed.len()), Some(1));
    }

    /// The gate a rehearsal exists to pass: the account is the same and the file is
    /// not there afterwards.
    #[test]
    fn a_rehearsed_install_says_everything_the_real_one_would_and_writes_nothing() {
        let ctx = rehearsing("rehearsed");
        let install = report(installing(&ctx, &source("rehearsed", MANIFEST)))
            .and_then(|one| one.install.clone());
        assert_eq!(
            install.as_ref().map(|one| one.would.plugin.clone()),
            Some("komga".to_owned())
        );
        assert_eq!(install.map(|one| one.recorded), Some(false));
        assert!(!record_of(&ctx).exists(), "the record was written");
        assert_eq!(counted(reading(&ctx)), Some(0));
    }

    /// And the listing beside it counts what is installed rather than what would
    /// be. A rehearsal that said *one plugin is installed* in the same breath as
    /// *nothing was written* is a rehearsal an operator has to choose between two
    /// halves of.
    #[test]
    fn a_rehearsed_install_is_not_counted_among_what_is_installed() {
        let ctx = rehearsing("uncounted");
        assert_eq!(
            counted(installing(&ctx, &source("uncounted", MANIFEST))),
            Some(0)
        );
    }

    #[test]
    fn a_path_holding_no_manifest_is_refused_rather_than_installed() {
        let ctx = ctx("nothing-there");
        assert_eq!(
            refusal(installing(&ctx, Path::new("/nowhere/at/all"))),
            "PLUGIN-2"
        );
        assert!(!record_of(&ctx).exists());
    }

    /// The reader's verdict is total, and the install honours it: a manifest that
    /// over-reaches on where it keeps its state is not installed at all.
    #[test]
    fn a_manifest_this_build_refuses_is_not_installed() {
        let ctx = ctx("refused");
        let at = source(
            "refused",
            &MANIFEST.replace(
                r#"config_path = "/app/data""#,
                r#"config_path = "/data/media""#,
            ),
        );
        assert_eq!(refusal(installing(&ctx, &at)), "PLUGIN-3");
        assert!(!record_of(&ctx).exists(), "the record was written");
    }

    #[test]
    fn installing_what_is_installed_is_refused_naming_it() {
        let ctx = ctx("twice");
        let at = source("twice", MANIFEST);
        assert_eq!(counted(installing(&ctx, &at)), Some(1));
        assert_eq!(refusal(installing(&ctx, &at)), "PLUGIN-5");
        assert_eq!(counted(reading(&ctx)), Some(1));
    }

    /// The gate this record exists to pass, in the place it runs. A damaged record
    /// read as empty would answer *nothing is installed* about a machine running
    /// somebody else's service — and then install a second copy over it.
    #[test]
    fn a_damaged_record_refuses_the_read_and_the_install_rather_than_reading_as_empty() {
        let ctx = ctx("damaged");
        let at = source("damaged", MANIFEST);
        assert_eq!(counted(installing(&ctx, &at)), Some(1));
        assert!(crate::config::store::write(&record_of(&ctx), "{ half a record").is_ok());

        assert_eq!(refusal(reading(&ctx)), "PLUGIN-4");
        assert_eq!(refusal(installing(&ctx, &at)), "PLUGIN-4");
        // And a refusal is not an answer with a shorter listing in it: there is no
        // report at all, which is what stops a surface rendering one.
        assert_eq!(report(reading(&ctx)), None);
    }

    /// A record that is there and cannot be opened at all is the same answer as one
    /// that will not parse, and for the same reason: the one thing that must not
    /// happen is answering *nothing is installed*.
    #[test]
    fn a_record_that_cannot_be_opened_is_refused_rather_than_read_as_empty() {
        let ctx = ctx("unopenable");
        assert!(std::fs::create_dir_all(record_of(&ctx)).is_ok());
        assert_eq!(refusal(reading(&ctx)), "PLUGIN-4");
    }

    /// Nowhere configured is a machine that has not been set up, which has no
    /// plugins rather than an unreadable record.
    #[test]
    fn a_machine_with_nowhere_to_keep_a_record_reads_as_nothing_installed() {
        assert_eq!(counted(reading(&a_context().build())), Some(0));
    }

    /// And refuses to write one, because the alternative is telling an operator
    /// something was remembered that was not.
    #[test]
    fn a_machine_with_nowhere_to_keep_a_record_refuses_to_install() {
        let ctx = a_context().build();
        assert!(!refusal(installing(&ctx, &source("nowhere", MANIFEST))).is_empty());
    }

    /// Every change the record holds, oldest first, as a later run reads them back.
    fn journalled(ctx: &Ctx) -> Vec<Change> {
        let at = ctx
            .settings
            .env_file
            .as_deref()
            .map(|env| env.with_file_name(crate::config::paths::JOURNAL))
            .unwrap_or_default();
        super::super::recover::journal_at(&at).changes().to_vec()
    }

    /// The paths a run journalled, in the order it recorded them.
    ///
    /// Read off each change's target rather than out of its kind. The target of a
    /// change an install writes *is* the path, and that these are paths lemonfiber
    /// made is asserted where an operator would see it — in the history below, which
    /// renders the kind rather than being told it.
    fn made_paths(ctx: &Ctx) -> Vec<String> {
        journalled(ctx)
            .into_iter()
            .map(|change| change.target)
            .collect()
    }

    /// The install puts the plugin's own wiring where the stack reads it: a
    /// configuration directory for the service, and one Compose document naming the
    /// container lemonfiber writes.
    #[test]
    fn installing_writes_the_service_s_directory_and_the_document_that_mounts_it() {
        let ctx = ctx("writes");
        assert_eq!(
            counted(installing(&ctx, &source("writes", MANIFEST))),
            Some(1)
        );

        let stack = stack_of(&ctx);
        assert!(stack.join("config/komga").is_dir());
        assert!(stack.join("compose/plugins/komga.yml").is_file());
    }

    /// What lands on disk is the container the record derives, not a second rendering
    /// of it. Two renderings are free to disagree, and the one Compose reads is the
    /// one on disk — so a plugin could be shown one entry and run another.
    #[test]
    fn the_document_on_disk_is_the_container_the_record_derives() {
        let ctx = ctx("derives");
        let shown = report(installing(&ctx, &source("derives", MANIFEST)));
        let recorded = shown
            .and_then(|one| one.install)
            .map(|one| crate::plugin::written(&one.would))
            .unwrap_or_default();

        let written = std::fs::read_to_string(stack_of(&ctx).join("compose/plugins/komga.yml"))
            .unwrap_or_default();
        assert_eq!(written, recorded);
        assert!(written.contains("profiles: [plugin-komga]"));
    }

    /// The whole of what makes a plugin's changes ordinary: they are in the record
    /// every other change is in, named as the plugin rather than as lemonfiber, and
    /// stamped as one run so a reversal can ask for exactly them.
    #[test]
    fn every_write_is_journalled_under_the_plugin_s_own_name_as_one_run() {
        let ctx = ctx("journalled");
        assert_eq!(
            counted(installing(&ctx, &source("journalled", MANIFEST))),
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
    #[test]
    fn the_directory_is_journalled_before_the_document_that_mounts_it() {
        let ctx = ctx("ordered");
        assert_eq!(
            counted(installing(&ctx, &source("ordered", MANIFEST))),
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
    #[test]
    fn a_plugin_s_changes_are_in_the_history_named_as_the_plugin() {
        let ctx = ctx("in-history");
        assert_eq!(
            counted(installing(&ctx, &source("in-history", MANIFEST))),
            Some(1)
        );

        let shown = super::super::history::history(&ctx);
        assert!(!shown.changes.is_empty());
        assert!(shown.changes.iter().all(|one| one.operation == "komga"));
        assert!(shown.changes.iter().all(|one| one.reversal == "whole"));
        assert!(shown
            .changes
            .iter()
            .any(|one| one.did.starts_with("made ") && one.did.ends_with("komga.yml")));
    }

    /// A plugin's changes are classified by the rollback layer like any other change,
    /// which for a path lemonfiber made is reversible in full. Asked of the layer
    /// itself rather than asserted here, so this stays true the day that judgement
    /// changes.
    #[test]
    fn what_an_install_journalled_is_judged_by_the_rollback_layer_like_anything_else() {
        let ctx = ctx("judged");
        assert_eq!(
            counted(installing(&ctx, &source("judged", MANIFEST))),
            Some(1)
        );

        let changes = journalled(&ctx);
        assert!(changes.iter().all(|change| {
            crate::rollback::standing(change, &[], &|_| None).reversal
                == crate::rollback::Reversal::Whole
        }));
    }

    /// A rehearsal settles everything and leaves the machine exactly as it was — no
    /// directory, no document, and nothing on the record to put back.
    #[test]
    fn a_rehearsed_install_writes_no_wiring_and_journals_nothing() {
        let ctx = rehearsing("rehearsed-wiring");
        let at = source("rehearsed-wiring", MANIFEST);
        assert_eq!(counted(installing(&ctx, &at)), Some(0));

        let stack = stack_of(&ctx);
        assert!(!stack.join("config/komga").exists());
        assert!(!stack.join("compose/plugins/komga.yml").exists());
        assert!(journalled(&ctx).is_empty());
    }

    /// A directory the operator already had is theirs. The install uses it and does
    /// not record it, so putting the install back never removes something it found
    /// rather than made.
    #[test]
    fn a_directory_that_was_already_there_is_used_and_not_recorded() {
        let ctx = ctx("already-there");
        let existing = stack_of(&ctx).join("config/komga");
        assert!(std::fs::create_dir_all(&existing).is_ok());

        assert_eq!(
            counted(installing(&ctx, &source("already-there", MANIFEST))),
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
    #[test]
    fn the_record_is_what_the_assertions_above_turn_on() {
        let untouched = ctx("turns-on-second");
        assert!(
            journalled(&untouched).is_empty(),
            "a machine that installed nothing has no record"
        );

        let installed = ctx("turns-on");
        assert_eq!(
            counted(installing(&installed, &source("turns-on", MANIFEST))),
            Some(1)
        );
        assert!(!journalled(&installed).is_empty());
    }

    /// A stack to write into is not enough on its own: the record of what was written
    /// lives beside the settings, and a machine that cannot say where those are has
    /// nowhere to journal to. Refused rather than written unrecorded, because an
    /// unrecorded write is the one that cannot be put back.
    #[test]
    fn a_machine_that_cannot_say_where_its_own_files_are_refuses_the_install() {
        let ctx = a_context()
            .settings(crate::config::Settings {
                env_file: None,
                stack_dir: Some(std::env::temp_dir().join("lemonfiber-unrooted/stack")),
                ..crate::config::Settings::default()
            })
            .build();
        assert!(!refusal(installing(&ctx, &source("unrooted", MANIFEST))).is_empty());
        assert_ne!(
            refusal(installing(&ctx, &source("unrooted", MANIFEST))),
            "PLUGIN-6",
            "a machine with a stack but no home for its records is not a machine \
             with nowhere to put a container"
        );
    }

    /// A write that cannot land stops the install where it is rather than carrying
    /// on. What it had already written is on the record, which is what makes the
    /// half-done state recoverable rather than a mystery.
    #[test]
    fn a_write_that_cannot_land_stops_the_install_and_leaves_what_it_wrote_on_the_record() {
        let ctx = ctx("blocked");
        let occupied = stack_of(&ctx).join("config/komga");
        let _ = occupied.parent().map(std::fs::create_dir_all);
        // A file where the service's configuration directory has to go, so making
        // that directory cannot succeed.
        assert!(std::fs::write(&occupied, "not a directory").is_ok());

        assert_eq!(
            refusal(installing(&ctx, &source("blocked", MANIFEST))),
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
    #[test]
    fn a_document_whose_directory_cannot_be_made_stops_the_install() {
        let ctx = ctx("no-room");
        let overlays = stack_of(&ctx).join("compose");
        let _ = overlays.parent().map(std::fs::create_dir_all);
        assert!(std::fs::write(&overlays, "not a directory").is_ok());

        assert_eq!(
            refusal(installing(&ctx, &source("no-room", MANIFEST))),
            "PLUGIN-7"
        );
    }

    /// And a document that cannot be written where its directory is fine is the same
    /// refusal rather than the settings store's. The writer is shared with the
    /// settings file, and its own failure says *your settings could not be saved* —
    /// which about a plugin's Compose document names the wrong file and offers the
    /// wrong remedy.
    #[test]
    fn a_document_that_will_not_take_the_write_is_refused_as_the_install_s_own() {
        let ctx = ctx("unwritable-document");
        let document = stack_of(&ctx).join("compose/plugins/komga.yml");
        // A directory where the document has to go: its own directory is made, and
        // the write into it cannot succeed.
        assert!(std::fs::create_dir_all(&document).is_ok());

        assert_eq!(
            refusal(installing(&ctx, &source("unwritable-document", MANIFEST))),
            "PLUGIN-7"
        );
    }

    /// A leftover document is overwritten rather than recorded. It is derived from
    /// the record and holds nothing an earlier run is owed, so recording it as made
    /// would be the one journal entry that removes a file this run did not create.
    #[test]
    fn a_leftover_document_is_overwritten_and_not_recorded_as_made() {
        let ctx = ctx("leftover");
        let document = stack_of(&ctx).join("compose/plugins/komga.yml");
        let _ = document.parent().map(std::fs::create_dir_all);
        assert!(std::fs::write(&document, "services: {}\n").is_ok());

        assert_eq!(
            counted(installing(&ctx, &source("leftover", MANIFEST))),
            Some(1)
        );
        assert!(std::fs::read_to_string(&document)
            .unwrap_or_default()
            .contains("profiles: [plugin-komga]"));
        assert!(
            !made_paths(&ctx)
                .iter()
                .any(|path| Path::new(path) == document),
            "a document this run found is not one it made"
        );
    }

    /// The register is written last, and a run that cannot write it refuses — which
    /// leaves the wiring on disk with nothing layering it. That is what the order
    /// buys: files nothing reads are inert and are on the change record, where the
    /// other order would leave a plugin the machine reports as installed with
    /// nothing behind it.
    ///
    /// Driven through a register this user may read and may not rewrite, because
    /// that is the one arrangement in which everything ahead of the last write
    /// succeeds.
    #[cfg(unix)]
    #[test]
    fn a_register_that_cannot_be_written_refuses_and_leaves_the_wiring_on_the_record() {
        use std::os::unix::fs::PermissionsExt as _;

        let ctx = ctx("unrecordable");
        let register = ctx
            .settings
            .env_file
            .as_deref()
            .map(|env| env.with_file_name(PLUGINS))
            .unwrap_or_default();
        assert!(std::fs::write(&register, "{}").is_ok());
        assert!(
            std::fs::set_permissions(&register, std::fs::Permissions::from_mode(0o400)).is_ok()
        );

        assert_eq!(
            refusal(installing(&ctx, &source("unrecordable", MANIFEST))),
            "PLUGIN-8"
        );
        assert!(
            stack_of(&ctx).join("compose/plugins/komga.yml").is_file(),
            "what was written before the refusal is still there"
        );
        assert!(
            made_paths(&ctx)
                .iter()
                .any(|path| path.ends_with("compose/plugins/komga.yml")),
            "and is on the record, so it can be put back"
        );

        let _ = std::fs::set_permissions(&register, std::fs::Permissions::from_mode(0o600));
    }

    /// A stack directory is where a plugin's container has to go, so a machine
    /// without one is refused by name rather than installed half-way.
    #[test]
    fn a_machine_with_no_stack_directory_refuses_the_install_naming_it() {
        let env_file = env_at("no-stack", &a_password());
        let ctx = a_context()
            .settings(crate::config::Settings {
                env_file: Some(env_file),
                stack_dir: None,
                ..crate::config::Settings::default()
            })
            .build();
        assert_eq!(
            refusal(installing(&ctx, &source("no-stack", MANIFEST))),
            "PLUGIN-6"
        );
    }

    /// And a rehearsal on that same machine still answers, because what it is being
    /// asked is what installing would decide rather than where the files would go.
    #[test]
    fn a_rehearsal_still_answers_on_a_machine_with_no_stack_directory() {
        let env_file = env_at("no-stack-rehearsed", &a_password());
        let mut ctx = a_context()
            .settings(crate::config::Settings {
                env_file: Some(env_file),
                stack_dir: None,
                ..crate::config::Settings::default()
            })
            .build();
        ctx.dry_run = true;
        let shown = report(installing(&ctx, &source("no-stack-rehearsed", MANIFEST)));
        assert_eq!(
            shown.and_then(|one| one.install).map(|one| one.recorded),
            Some(false)
        );
    }
}
