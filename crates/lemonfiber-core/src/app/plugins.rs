//! Installing somebody else's plugin, and reading back what is installed.
//!
//! Nothing here runs a container, writes a compose entry or asks a service
//! anything. What it does is settle what installing this manifest decides and write
//! that down, which is the half every later step reads rather than re-deriving: the
//! manifest is the author's file and may be gone tomorrow, and a run that re-read it
//! would be answering a question about a document rather than about the machine.
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
/// or where the record cannot be written.
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

/// Settle what installing this source decides, and write it down.
///
/// The write is the last thing and the only thing a rehearsal holds back, so what a
/// rehearsal reports is what the real run reports — read by the same code, refused
/// for the same reasons, and stopping one line short.
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
        super::record::keep(kept_at(ctx).as_deref(), &after)?;
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
    use std::path::{Path, PathBuf};

    use super::{asked, Asked};
    use crate::app::{Ctx, Outcome};
    use crate::config::paths::PLUGINS;
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

    /// A context whose settings point at a scratch configuration directory.
    fn ctx(name: &str) -> Ctx {
        a_context()
            .settings(crate::config::Settings {
                env_file: Some(env_at(name, &a_password())),
                ..crate::config::Settings::default()
            })
            .build()
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
}
