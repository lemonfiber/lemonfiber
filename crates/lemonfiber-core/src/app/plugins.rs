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

// Starting what the writing placed, asking it what the manifest said it would
// answer, and taking it back where it did not. Its own file because the one thing
// here that reaches a service and a container engine should be the one thing a
// reader has to hold a seam in mind for.
mod proving;
// Asking the stack's own checks what they make of the machine, before the writes and
// again after them. Apart from the proving because the two answer different
// questions: one asks the plugin whether it works, the other asks the stack whether
// it still does.
mod verifying;
// Taking a plugin off the machine. Its own file because a removal is the rollback
// layer's work with a name on it, and what is here is only the three things that are
// not a journal entry: the containers, the register, and what the machine is left
// without.
mod removing;
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
    /// Take a plugin off the machine, putting back everything installing it wrote.
    ///
    /// The id rather than a path, because the plugin's own source may be long gone and
    /// what is being removed is a record this machine holds rather than a document
    /// somebody still has.
    Remove {
        /// The plugin's id, as `lemonfiber plugin installed` lists it.
        plugin: String,
    },
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

/// The plugin's own service would not start, so nothing about it could be proved.
pub(super) const UNPROVED: Code = Code::new("PLUGIN-9");

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
/// not land, where the plugin's own service would not start, or where the record of
/// what is installed cannot be written. Every one of those after the first write puts
/// the install back before it answers.
pub(super) async fn asked(ctx: &Ctx, action: &Asked) -> Result<Outcome, Box<Problem>> {
    let held = read(ctx)?;
    match action {
        Asked::Installed => Ok(Outcome::Plugins(Installs {
            installed: held.installed().to_vec(),
            install: None,
            removal: None,
        })),
        Asked::Install { path } => install(ctx, held, path).await,
        Asked::Remove { plugin } => removing::remove(ctx, held, plugin).await,
    }
}

/// Settle what installing this source decides, write the plugin's wiring, and record
/// it.
///
/// Everything a rehearsal holds back is one branch wide, so what a rehearsal reports
/// is what the real run reports — settled by the same code, refused for the same
/// reasons, and stating the same three lists before stopping short of carrying them
/// out.
///
/// **A rehearsal is refused wherever the install would be, including for want of a
/// stack.** The account it gives is the one the install then follows, so a rehearsal
/// that answered on a machine the install could not run on would be describing an
/// operation that cannot happen there — and the operator would find that out on the
/// run they thought they had already checked. What answers with no machine at all is
/// `plugin claims`, which is the author's read and needs neither a stack nor a
/// record.
///
/// **The register is the last thing written, and it is written only once the proofs
/// have held.** That is what makes *registered* and *proved* the same fact rather than
/// two that agree on a good day: the wiring goes down, the plugin's own services come
/// up, every proof it declared is asked of them, and only then is the plugin recorded
/// as installed. Every failure after the first write puts the install back, so what
/// an operator is left with is the machine they had. And a run that dies outright
/// still leaves only files nothing reads — inert, on the change record, and
/// removable — because the register is what layers a plugin's document into the
/// stack. The other order would leave a plugin the machine reports as installed and
/// never proved.
///
/// **A proof that does not hold puts the whole install back.** The container comes off
/// first, because nothing on disk records that it is running and a document removed
/// out from under one leaves something Compose will never be asked about again; then
/// the files go back through the rollback layer, over the journal entries the writing
/// already made. Nothing here undoes anything itself.
async fn install(ctx: &Ctx, held: Register, path: &Path) -> Result<Outcome, Box<Problem>> {
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

    // Where the writes land, asked for before the branch rather than inside it. What
    // a rehearsal has to state is where every change goes, and a path is a fact about
    // this machine — so a machine with nowhere to put them has nothing for a
    // rehearsal to state and nothing for an install to do.
    let stack = ctx
        .settings
        .stack_dir
        .as_deref()
        .ok_or_else(|| Box::new(nowhere_to_write(&would.plugin)))?;
    let planned = crate::plugin::writes(&would, stack);

    let mut stated = crate::plugin::proofs(&manifest);
    let mut against = None;
    let mut checked = None;
    let mut put_back = None;
    let mut recorded = false;

    if !ctx.dry_run {
        let stamp = ctx.stamp();
        // Read before a byte of it is written, and that order is the whole of what
        // makes the second reading mean anything. What this has to tell apart is a
        // check the install broke from one that was already failing, and after the
        // fact there is nothing left to ask.
        let (standing, before) = verifying::looked(ctx).await?;

        carry_out(ctx, &would.plugin, &stamp, &planned)?;

        // Started before it is registered, which is why the invocation carries this
        // plugin rather than reading it back: the register is what layers a plugin's
        // document into the stack, and it is deliberately not written yet.
        proving::started(ctx, &would, stack, &stamp).await?;
        proving::asked(ctx, &manifest, &would, &mut stated).await;
        against = Some(proving::AGAINST);

        // The stack is asked only where the plugin's own proofs held. A run that has
        // already failed is a run being put back, and asking a machine mid-reversal
        // what it makes of itself would produce an account of neither state.
        if proving::held(&stated) {
            checked = Some(crate::plugin::against(
                &before,
                &verifying::again(ctx, &standing).await,
            ));
        }

        // Recorded where both halves held, and put back where either did not. One
        // question answers for both: a verification nobody took is a run whose proofs
        // did not hold, because that is the only way this gets here without one.
        if checked
            .as_ref()
            .is_some_and(crate::plugin::Verification::held)
        {
            // Answered for here rather than passed on. The record writer is shared and
            // says *your settings could not be saved, your existing settings are
            // untouched* — which after the lines above is false twice over: the file
            // is not the settings, and the machine has been written to.
            //
            // And it goes back, rather than being left for somebody to find. The
            // proofs held, so the only thing between here and an install is the one
            // file that could not be written — and a plugin whose container is up
            // with nothing recording it is the state this verb exists to not leave.
            if let Err(why) = super::record::keep(kept_at(ctx).as_deref(), &after) {
                let back = reversing(ctx, &would, stack, &stamp).await;
                return Err(Box::new(unrecordable(&would.plugin, *why, &back)));
            }
            recorded = true;
        } else {
            put_back = Some(reversing(ctx, &would, stack, &stamp).await);
        }
    }

    // What the record holds, which after a rehearsal or a reversal is what it held
    // before. A listing that counted the entry nobody wrote would report an install
    // that did not happen, in the same breath as saying nothing was written — and a
    // reader who believes the count over the sentence is the one this is written for.
    let standing = if recorded { after } else { held };

    Ok(Outcome::Plugins(Installs {
        removal: None,
        installed: standing.installed().to_vec(),
        install: Some(Install {
            would,
            recorded,
            changes: crate::plugin::changes(&planned),
            proofs: stated,
            against,
            verified: checked,
            overrides: crate::plugin::overrides(&manifest),
            reversed: put_back,
        }),
    }))
}

/// Put the install back, container first and then the files.
///
/// The container is not a journal entry — nothing on disk records that it is running —
/// so it comes off here, and everything after it is the rollback layer reversing the
/// entries the writing already made. A removal that the engine would not carry out is
/// reported rather than raised: this runs inside an install that is already failing,
/// and stopping at the first difficulty would leave more behind than carrying on does.
///
/// **A run that wrote nothing has nothing to put back, and that is not a second
/// failure.** An install records only what it actually had to make, so one that found
/// every directory and every document already there — the leftovers of an earlier run
/// that got as far as writing them — journals nothing under its own stamp, and the
/// rollback layer has no run of that stamp to find. What went back is then nothing,
/// which is the true answer: those files belong to the run that wrote them and are on
/// the record under it.
async fn reversing(
    ctx: &Ctx,
    would: &Installed,
    stack: &Path,
    stamp: &str,
) -> super::putting_back::Reversal {
    let off = proving::removed(ctx, would, stack).await;
    let mut back = super::putting_back::reversing(ctx, Some(stamp))
        .await
        .unwrap_or_default();
    if !off {
        back.left.push(super::putting_back::Left {
            target: would.plugin.clone(),
            because: "its container could not be taken off the machine, so it may still be \
                      running with nothing in the stack describing it"
                .to_owned(),
        });
    }
    back
}

/// What the machine holds now, read off what the putting back actually did.
///
/// Written after the reversal rather than beside the failure, because a sentence
/// written where the failure is raised is a promise about work that has not happened
/// yet — and on the run where the reversal cannot finish it is false in exactly the
/// place an operator would act on it.
///
/// One sentence for every refusal that puts the install back, so two refusals cannot
/// describe the same machine differently.
pub(super) fn left_behind(back: &super::putting_back::Reversal) -> String {
    if back.left.is_empty() {
        return "The install was put back, and nothing is recorded as installed. \
                `lemonfiber history` says what went back."
            .to_owned();
    }
    let standing = back
        .left
        .iter()
        .map(|one| format!("{} — {}", one.target, one.because))
        .collect::<Vec<_>>()
        .join("; ");
    format!(
        "The install was put back as far as it could go, and nothing is recorded as installed. \
         Still standing: {standing}."
    )
}

/// What the record holds, or why nothing can be said about it.
///
/// Three answers and never two. No file at all is a machine that has installed
/// nothing, which is an answer rather than a fault. A file that is there and cannot
/// be read — damaged, half-written, unreadable to this user — is refused, because
/// the alternative is telling an operator that nothing is installed while somebody
/// else's service is running.
///
/// Reachable from the diagnostics register too, which asks the same question for the
/// same reason: the rows a plugin contributed are run from this record, and a
/// diagnosis that read past a register it could not parse would report a clean bill of
/// health with a stranger's rows silently missing from it.
///
/// # Errors
///
/// Where the record is there and this build cannot read it, or names one plugin twice.
pub(crate) fn read(ctx: &Ctx) -> Result<Register, Box<Problem>> {
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

/// Everything held and the record that says so could not be written.
///
/// Its own refusal rather than the record writer's, because the writer is shared with
/// the settings file and says *your settings could not be saved, your existing
/// settings are untouched*. Both halves are wrong here: the file is not the settings,
/// and the machine has been written to and started.
///
/// What an operator needs is what the run left them with, which is why the sentence
/// is read off the reversal rather than written here. The install goes back for the
/// same reason every other failure does: the proofs held, so the one thing between
/// this and an installed plugin is a file that would not be written — and a container
/// running with nothing recording it is precisely the state this verb exists to not
/// leave behind.
fn unrecordable(plugin: &str, why: Problem, back: &super::putting_back::Reversal) -> Problem {
    Problem::new(
        UNRECORDABLE,
        Severity::Error,
        format!("{plugin} held every proof and could not be recorded as installed"),
        left_behind(back),
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

    use std::sync::Arc;

    use lemonfiber_fixtures::http::Fake;
    use lemonfiber_fixtures::support::{refused as engine_refused, spoke, Keyed, Recording};

    use super::{asked, Asked};
    use crate::app::{Ctx, Outcome};
    use crate::config::paths::PLUGINS;
    use crate::journal::Change;
    use crate::plugin::Installs;
    use crate::plugin::Verdict;
    use crate::ports::http::Http;
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

    /// The same manifest, declaring one proof of its one service.
    ///
    /// A proof rather than a claim's probe, because a proof is the thing that gates an
    /// install: a claim says what the service can do and a proof says what has to hold
    /// for installing it to be worth doing at all.
    const PROVING: &str = r#"
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

[[proof]]
id      = "answers"
title   = "the library API answers"
why     = "a plugin whose service does not answer is not installed"
request = { method = "GET", path = "/api/v1/libraries" }
expect  = { status = 200 }
"#;

    /// The same manifest again, this time also contributing a row to the doctor's own
    /// register — one whose service will not answer it.
    const CONTRIBUTING: &str = r#"
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

[requires]
capabilities = ["doctor.contribute"]

[[proof]]
id      = "answers"
title   = "the library API answers"
why     = "a plugin whose service does not answer is not installed"
request = { method = "GET", path = "/api/v1/libraries" }
expect  = { status = 200 }

[[contribution]]
at        = "doctor.check"
id        = "komga:claimed"
title     = "Komga has an administrator"
category  = "credentials"
request   = { method = "GET", path = "/api/v1/claim" }
expect    = { status = 200 }
fixture   = "fixtures/claim.json"
why       = "An unclaimed Komga hands administrator to whoever asks first."

[[contribution]]
at     = "doctor.remedy"
id     = "komga:claim-it"
for    = "komga:claimed"
action = "Open Komga and create the administrator account"
why    = "Until somebody does, the first caller on the household network becomes it."
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

    /// The same, with the runner and the transport a run that proves needs.
    ///
    /// The runner answers for the container engine and the transport answers for the
    /// service: an install that proves reaches both, and a context that faked neither
    /// would be a test about a real machine's Docker and a real machine's ports.
    fn proving(name: &str, runner: Arc<dyn crate::ports::Runner>, http: Arc<dyn Http>) -> Ctx {
        let env_file = env_at(name, &a_password());
        let stack = env_file.with_file_name("data").join("stack");
        a_context()
            .runner(runner)
            .settings(crate::config::Settings {
                env_file: Some(env_file),
                stack_dir: Some(stack),
                ..crate::config::Settings::default()
            })
            .build()
            .with_http(http)
            .waiting(std::time::Duration::ZERO)
    }

    /// A transport that answers the one path the proof above asks at.
    fn answering(status: u16) -> Arc<dyn Http> {
        Fake::by_path(vec![(
            "/api/v1/libraries",
            lemonfiber_fixtures::http::Answer::reply(status, "[]"),
        )])
    }

    /// What one verdict is, in one word.
    ///
    /// A word rather than a pattern at each case, because a pattern's other half is a
    /// branch nothing ever takes — and a case that cannot say which of the three it
    /// got is a case that would pass on any of them.
    fn came_to(verdict: Option<&Verdict>) -> &'static str {
        match verdict {
            None => "unasked",
            Some(Verdict::Passed) => "held",
            Some(Verdict::Failed { .. }) => "failed",
            Some(Verdict::Unproven { .. }) => "unproven",
        }
    }

    /// What the proofs on a report came to, in the order they were declared.
    fn verdicts(outcome: Result<Outcome, Box<crate::error::Problem>>) -> Vec<Option<Verdict>> {
        report(outcome)
            .and_then(|one| one.install)
            .map(|one| one.proofs.into_iter().map(|proof| proof.came_to).collect())
            .unwrap_or_default()
    }

    /// What a verdict that established nothing says stopped it.
    ///
    /// Empty for every other verdict, so a case asserting on it is asserting on the
    /// one that carries a reason rather than on whichever it happened to get.
    fn why(verdict: Option<&Verdict>) -> String {
        match verdict {
            Some(Verdict::Unproven { why }) => why.clone(),
            None | Some(Verdict::Passed | Verdict::Failed { .. }) => String::new(),
        }
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
    async fn installing(ctx: &Ctx, at: &Path) -> Result<Outcome, Box<crate::error::Problem>> {
        asked(
            ctx,
            &Asked::Install {
                path: at.to_path_buf(),
            },
        )
        .await
    }

    /// Put a settings change on the record under one operation's name, the way a
    /// recipe would once recipes are applied.
    ///
    /// Written directly rather than through a verb that makes one, because what is
    /// under test is the rollback layer's judgement of such a change and no verb makes
    /// one yet. The entry is the same shape an apply writes.
    fn journal_a_set(ctx: &Ctx, operation: &str, key: &str, wrote: &str) {
        let change = Change {
            at: ctx.stamp(),
            operation: operation.to_owned(),
            target: ".env".to_owned(),
            kind: crate::journal::Kind::Set {
                key: key.to_owned(),
                previous: None,
                current: wrote.to_owned(),
            },
        };
        let _ = crate::app::targets::layout(ctx).map(|paths| {
            crate::app::recover::journalled(&paths.journal(), &[change], ctx.random.as_ref());
        });
    }

    /// What removing that plugin came to.
    async fn removing(ctx: &Ctx, plugin: &str) -> Result<Outcome, Box<crate::error::Problem>> {
        asked(
            ctx,
            &Asked::Remove {
                plugin: plugin.to_owned(),
            },
        )
        .await
    }

    /// What a run's removal said, where it made one.
    fn removal(
        outcome: Result<Outcome, Box<crate::error::Problem>>,
    ) -> Option<crate::plugin::Removal> {
        report(outcome).and_then(|one| one.removal)
    }

    /// What the reading came to.
    async fn reading(ctx: &Ctx) -> Result<Outcome, Box<crate::error::Problem>> {
        asked(ctx, &Asked::Installed).await
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

    /// A refusal's code and what it told the operator it left them with, read off the
    /// one problem rather than by asking twice.
    fn refused(outcome: Result<Outcome, Box<crate::error::Problem>>) -> (String, String) {
        outcome
            .err()
            .map(|problem| (problem.code.to_string(), problem.meaning))
            .unwrap_or_default()
    }

    /// A refusal's own code and the code of whatever it carries underneath it.
    fn beneath(outcome: Result<Outcome, Box<crate::error::Problem>>) -> (String, String) {
        outcome
            .err()
            .map(|problem| {
                (
                    problem.code.to_string(),
                    problem
                        .cause
                        .map(|cause| cause.code.to_string())
                        .unwrap_or_default(),
                )
            })
            .unwrap_or_default()
    }

    /// The whole of the slice: what the manifest declared survives, and a later run
    /// reads it back without the manifest being anywhere near.
    #[tokio::test]
    async fn where_a_service_keeps_its_configuration_survives_the_install_and_is_read_back() {
        let ctx = ctx("survives");
        let at = source("survives", MANIFEST);
        assert_eq!(counted(installing(&ctx, &at).await), Some(1));

        // The author's file goes, as it may the moment an install is done.
        let _ = std::fs::remove_dir_all(&at);

        let path = report(reading(&ctx).await)
            .and_then(|report| report.installed.first().cloned())
            .and_then(|one| one.services.first().cloned())
            .map(|one| one.config_path);
        assert_eq!(path.as_deref(), Some("/app/data"));
    }

    #[tokio::test]
    async fn a_machine_with_nothing_installed_answers_with_an_empty_list() {
        let read = report(reading(&ctx("empty")).await);
        assert_eq!(read.as_ref().map(|one| one.installed.len()), Some(0));
        assert_eq!(read.map(|one| one.install.is_none()), Some(true));
    }

    #[tokio::test]
    async fn the_install_says_what_it_recorded_and_what_it_joined() {
        let ctx = ctx("recorded");
        let shown = report(installing(&ctx, &source("recorded", MANIFEST)).await);
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
    #[tokio::test]
    async fn a_rehearsed_install_says_everything_the_real_one_would_and_writes_nothing() {
        let ctx = rehearsing("rehearsed");
        let install = report(installing(&ctx, &source("rehearsed", MANIFEST)).await)
            .and_then(|one| one.install.clone());
        assert_eq!(
            install.as_ref().map(|one| one.would.plugin.clone()),
            Some("komga".to_owned())
        );
        assert_eq!(install.map(|one| one.recorded), Some(false));
        assert!(!record_of(&ctx).exists(), "the record was written");
        assert_eq!(counted(reading(&ctx).await), Some(0));
    }

    /// And the listing beside it counts what is installed rather than what would
    /// be. A rehearsal that said *one plugin is installed* in the same breath as
    /// *nothing was written* is a rehearsal an operator has to choose between two
    /// halves of.
    #[tokio::test]
    async fn a_rehearsed_install_is_not_counted_among_what_is_installed() {
        let ctx = rehearsing("uncounted");
        assert_eq!(
            counted(installing(&ctx, &source("uncounted", MANIFEST)).await),
            Some(0)
        );
    }

    #[tokio::test]
    async fn a_path_holding_no_manifest_is_refused_rather_than_installed() {
        let ctx = ctx("nothing-there");
        assert_eq!(
            refusal(installing(&ctx, Path::new("/nowhere/at/all")).await),
            "PLUGIN-2"
        );
        assert!(!record_of(&ctx).exists());
    }

    /// The reader's verdict is total, and the install honours it: a manifest that
    /// over-reaches on where it keeps its state is not installed at all.
    #[tokio::test]
    async fn a_manifest_this_build_refuses_is_not_installed() {
        let ctx = ctx("refused");
        let at = source(
            "refused",
            &MANIFEST.replace(
                r#"config_path = "/app/data""#,
                r#"config_path = "/data/media""#,
            ),
        );
        assert_eq!(refusal(installing(&ctx, &at).await), "PLUGIN-3");
        assert!(!record_of(&ctx).exists(), "the record was written");
    }

    #[tokio::test]
    async fn installing_what_is_installed_is_refused_naming_it() {
        let ctx = ctx("twice");
        let at = source("twice", MANIFEST);
        assert_eq!(counted(installing(&ctx, &at).await), Some(1));
        assert_eq!(refusal(installing(&ctx, &at).await), "PLUGIN-5");
        assert_eq!(counted(reading(&ctx).await), Some(1));
    }

    /// The gate this record exists to pass, in the place it runs. A damaged record
    /// read as empty would answer *nothing is installed* about a machine running
    /// somebody else's service — and then install a second copy over it.
    #[tokio::test]
    async fn a_damaged_record_refuses_the_read_and_the_install_rather_than_reading_as_empty() {
        let ctx = ctx("damaged");
        let at = source("damaged", MANIFEST);
        assert_eq!(counted(installing(&ctx, &at).await), Some(1));
        assert!(crate::config::store::write(&record_of(&ctx), "{ half a record").is_ok());

        assert_eq!(refusal(reading(&ctx).await), "PLUGIN-4");
        assert_eq!(refusal(installing(&ctx, &at).await), "PLUGIN-4");
        // And a refusal is not an answer with a shorter listing in it: there is no
        // report at all, which is what stops a surface rendering one.
        assert_eq!(report(reading(&ctx).await), None);
    }

    /// A record that is there and cannot be opened at all is the same answer as one
    /// that will not parse, and for the same reason: the one thing that must not
    /// happen is answering *nothing is installed*.
    #[tokio::test]
    async fn a_record_that_cannot_be_opened_is_refused_rather_than_read_as_empty() {
        let ctx = ctx("unopenable");
        assert!(std::fs::create_dir_all(record_of(&ctx)).is_ok());
        assert_eq!(refusal(reading(&ctx).await), "PLUGIN-4");
    }

    /// Nowhere configured is a machine that has not been set up, which has no
    /// plugins rather than an unreadable record.
    #[tokio::test]
    async fn a_machine_with_nowhere_to_keep_a_record_reads_as_nothing_installed() {
        assert_eq!(counted(reading(&a_context().build()).await), Some(0));
    }

    /// And refuses to write one, because the alternative is telling an operator
    /// something was remembered that was not.
    #[tokio::test]
    async fn a_machine_with_nowhere_to_keep_a_record_refuses_to_install() {
        let ctx = a_context().build();
        assert!(!refusal(installing(&ctx, &source("nowhere", MANIFEST)).await).is_empty());
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
        assert!(written.contains("profiles: [plugin-komga]"));
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
    #[tokio::test]
    async fn what_an_install_journalled_is_judged_by_the_rollback_layer_like_anything_else() {
        let ctx = ctx("judged");
        assert_eq!(
            counted(installing(&ctx, &source("judged", MANIFEST)).await),
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
                stack_dir: Some(std::env::temp_dir().join("lemonfiber-unrooted/stack")),
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
            .contains("profiles: [plugin-komga]"));
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
    /// Driven through a register this user may read and may not rewrite, because
    /// that is the one arrangement in which everything ahead of the last write
    /// succeeds.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_register_that_cannot_be_written_puts_the_whole_install_back() {
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

        let _ = std::fs::set_permissions(&register, std::fs::Permissions::from_mode(0o600));
    }

    /// The whole of what a proof buys: the plugin's own service is started, asked what
    /// the manifest said it would answer, and recorded as installed only once it has
    /// answered it.
    #[tokio::test]
    async fn a_plugin_whose_proof_holds_is_started_asked_and_then_recorded() {
        let runner = Arc::new(Recording::answering(Ok(spoke(""))));
        let ctx = proving("proved", runner.clone(), answering(200));

        assert_eq!(
            counted(installing(&ctx, &source("proved", PROVING)).await),
            Some(1)
        );
        assert!(runner.ran("up"), "its own service was started");
        assert!(
            !runner.ran("rm"),
            "and nothing was taken back off the machine"
        );
    }

    /// A runner that stops answering one reading the moment the plugin's container
    /// is up.
    ///
    /// The one lever a test has on the stack's own checks. Everything the diagnosis
    /// reaches here is a fake, and no fake answers differently because a file was
    /// written — so the change is tied to the install's own call, which is what the
    /// rule exists for said as plainly as a fake can say it.
    struct LostToTheInstall {
        /// The word the failing call carries besides `version`: `compose` singles out
        /// the Compose reading, and `version` itself takes the engine's own with it.
        once_up: &'static str,
        started: std::sync::atomic::AtomicBool,
    }

    impl LostToTheInstall {
        /// One of them, before anything has started.
        fn losing(once_up: &'static str) -> Arc<Self> {
            Arc::new(Self {
                once_up,
                started: std::sync::atomic::AtomicBool::new(false),
            })
        }
    }

    #[async_trait::async_trait]
    impl crate::ports::Runner for LostToTheInstall {
        async fn run(
            &self,
            argv: &[String],
        ) -> Result<lemonfiber_ports::process::Output, lemonfiber_ports::process::Failure> {
            let said = |named: &str| argv.iter().any(|word| word == named);
            if said("up") {
                self.started
                    .store(true, std::sync::atomic::Ordering::SeqCst);
            }
            if said("version")
                && said(self.once_up)
                && self.started.load(std::sync::atomic::Ordering::SeqCst)
            {
                return Err(lemonfiber_ports::process::Failure::NotFound {
                    program: "docker".to_owned(),
                });
            }
            Ok(spoke("25.0.5|1.44|1.44"))
        }
    }

    /// The whole of what the second half of an install buys. The plugin's own proofs
    /// held — its service answered exactly what the manifest said it would — and the
    /// stack was still taken back, because a check that was passing before the install
    /// is failing after it.
    ///
    /// **A differential attributes by time rather than by cause, and that is
    /// deliberate.** Nothing here can prove the install is what broke the engine check,
    /// and neither can an operator's machine. What it can say is that the check held
    /// before and does not now, which is the honest claim and the one worth acting on.
    #[tokio::test]
    async fn a_plugin_that_holds_its_own_proofs_and_breaks_the_stack_is_still_put_back() {
        let ctx = proving(
            "collateral",
            LostToTheInstall::losing("version"),
            answering(200),
        );

        let install = report(installing(&ctx, &source("collateral", PROVING)).await)
            .and_then(|one| one.install);
        assert_eq!(
            install
                .as_ref()
                .map(|one| came_to(one.proofs.first().and_then(|proof| proof.came_to.as_ref()))),
            Some("held"),
            "the plugin answered its own proof"
        );
        assert_eq!(
            install.as_ref().map(|one| one.recorded),
            Some(false),
            "and is not recorded as installed"
        );
        let broke: Vec<String> = install
            .as_ref()
            .and_then(|one| one.verified.as_ref())
            .map(|checked| {
                checked
                    .broke
                    .iter()
                    .map(|one| one.now.check.clone())
                    .collect()
            })
            .unwrap_or_default();
        assert!(
            broke.iter().any(|check| check == "environment.engine"),
            "the check that changed is named: {broke:?}"
        );
        assert!(
            install
                .as_ref()
                .and_then(|one| one.reversed.as_ref())
                .is_some(),
            "and the install went back"
        );
        assert!(
            !stack_of(&ctx).join("compose/plugins/komga.yml").exists(),
            "so what it wrote is gone again"
        );
    }

    /// A check that could no longer be told either way is reported and the install
    /// still stands. *I could not tell* is not *it is still broken*, and an install
    /// reversed on it would be punishing a plugin for something nobody established.
    #[tokio::test]
    async fn a_check_nothing_could_establish_is_reported_rather_than_held_against_it() {
        let ctx = proving(
            "unsettled",
            LostToTheInstall::losing("compose"),
            answering(200),
        );

        let install = report(installing(&ctx, &source("unsettled", PROVING)).await)
            .and_then(|one| one.install);
        let checked = install.as_ref().and_then(|one| one.verified.as_ref());
        assert!(
            checked.is_some_and(|one| one
                .unsettled
                .iter()
                .any(|changed| changed.now.check == "environment.compose")),
            "what could not be told is said out loud"
        );
        assert!(
            checked.is_some_and(crate::plugin::Verification::held),
            "and it does not stop the install"
        );
        assert_eq!(
            install.as_ref().map(|one| one.recorded),
            Some(true),
            "which is to say the plugin is installed"
        );
    }

    /// An install the stack was fine with says so, rather than saying nothing. A run
    /// that reported no verification at all would leave a reader unable to tell a
    /// clean reading from a reading nobody took.
    #[tokio::test]
    async fn an_install_the_stack_was_fine_with_says_the_checks_found_nothing() {
        let runner = Arc::new(Recording::answering(Ok(spoke(""))));
        let ctx = proving("unbroken", runner, answering(200));

        let checked = report(installing(&ctx, &source("unbroken", PROVING)).await)
            .and_then(|one| one.install)
            .and_then(|one| one.verified);
        assert!(
            checked
                .as_ref()
                .is_some_and(crate::plugin::Verification::held),
            "the checks were taken and nothing was worse for it"
        );
        assert!(
            checked.is_some_and(|one| one.broke.is_empty() && one.unsettled.is_empty()),
            "on a machine every fake answers the same way twice"
        );
    }

    /// A rehearsal takes no reading at all, and says nothing about one. A heading
    /// saying the checks found nothing would be a claim about a reading that never
    /// happened.
    #[tokio::test]
    async fn a_rehearsal_reports_no_verification_because_it_took_none() {
        let ctx = rehearsing("unchecked");

        assert!(
            report(installing(&ctx, &source("unchecked", PROVING)).await)
                .and_then(|one| one.install)
                .is_some_and(|one| one.verified.is_none())
        );
    }

    /// **A plugin's own contributed rows do not gate its own install, deliberately.**
    /// The register is written last, so the second reading of the stack's checks does
    /// not hold this plugin's rows — and it must not. What a contributed row says is
    /// an ongoing fact about a service an operator is running; a freshly installed one
    /// very often has nothing in it yet, and an install reversed for that would refuse
    /// every plugin whose first row is *is there anything in here*. What gates the
    /// install is what the plugin declared as a proof, which is the field that exists
    /// for saying so.
    #[tokio::test]
    async fn a_plugins_own_contributed_row_does_not_gate_its_own_install() {
        let runner = Arc::new(Recording::answering(Ok(spoke(""))));
        let ctx = proving(
            "contributing",
            runner,
            Fake::by_path(vec![
                (
                    "/api/v1/libraries",
                    lemonfiber_fixtures::http::Answer::reply(200, "[]"),
                ),
                (
                    "/api/v1/claim",
                    lemonfiber_fixtures::http::Answer::reply(500, "no"),
                ),
            ]),
        );

        let install = report(installing(&ctx, &source("contributing", CONTRIBUTING)).await)
            .and_then(|one| one.install);
        assert_eq!(
            install.as_ref().map(|one| one.recorded),
            Some(true),
            "the plugin holds its own proof and is installed"
        );
        assert!(
            install
                .as_ref()
                .and_then(|one| one.verified.as_ref())
                .is_some_and(crate::plugin::Verification::held),
            "and the row it contributes is not among what the stack was asked"
        );
        assert!(
            install.is_some_and(|one| one
                .would
                .contributions
                .iter()
                .any(|row| row.id == "komga:claimed" && row.service.as_deref() == Some("komga"))),
            "though the record keeps it, with the service it asks already settled"
        );
    }

    /// The whole of what a removal is: everything the install wrote goes back, its
    /// container comes off, and the record no longer holds it.
    #[tokio::test]
    async fn removing_a_plugin_puts_back_what_installing_it_wrote() {
        let runner = Arc::new(Recording::answering(Ok(spoke(""))));
        let ctx = proving("removing", runner.clone(), answering(200));
        assert_eq!(
            counted(installing(&ctx, &source("removing", PROVING)).await),
            Some(1)
        );
        let document = stack_of(&ctx).join("compose/plugins/komga.yml");
        assert!(document.is_file(), "it was installed");

        let gone = removal(removing(&ctx, "komga").await);
        assert_eq!(gone.as_ref().map(|one| one.removed), Some(true));
        assert!(!document.exists(), "the document it wrote is gone");
        assert!(
            !stack_of(&ctx).join("config/komga").exists(),
            "and so is the directory"
        );
        assert!(runner.ran("rm"), "its container was taken off the machine");
        assert_eq!(
            counted(reading(&ctx).await),
            Some(0),
            "nothing is installed"
        );
        assert!(
            gone.is_some_and(|one| one.went_back.left.is_empty()),
            "and nothing of its is still standing"
        );
        assert!(
            !record_of(&ctx).exists(),
            "and the record is taken away rather than kept empty, because an empty \
             register is still a file a reading can find"
        );
    }

    /// Taking one of two off leaves the record holding the other, rather than being
    /// taken away with it.
    #[tokio::test]
    async fn removing_one_of_two_leaves_the_record_holding_the_other() {
        let runner = Arc::new(Recording::answering(Ok(spoke(""))));
        let ctx = proving("two-of-them", runner, answering(200));
        assert_eq!(
            counted(installing(&ctx, &source("two-of-them", PROVING)).await),
            Some(1)
        );
        let second = PROVING.replace("\"komga\"", "\"kavita\"");
        assert_eq!(
            counted(installing(&ctx, &source("two-of-them-again", &second)).await),
            Some(2)
        );

        let gone = removal(removing(&ctx, "komga").await);
        assert!(gone.as_ref().is_some_and(|one| one.removed));
        assert!(
            gone.is_some_and(|one| one.went_back.left.iter().any(|left| left
                .because
                .contains("still holds something this run did not put there"))),
            "the directory the two share is named and left, rather than taken with the \
             other's document in it or stopping the reversal over it"
        );
        assert_eq!(
            counted(reading(&ctx).await),
            Some(1),
            "the other is still there"
        );
        assert!(
            record_of(&ctx).exists(),
            "and the record is kept rather than taken away"
        );
    }

    /// A stack this build cannot read stops a removal before it takes anything,
    /// because what the machine would be left without cannot be answered without it —
    /// and answering *nothing* would be a guess with a removal attached to it.
    #[tokio::test]
    async fn a_removal_on_an_unreadable_stack_is_refused_before_it_takes_anything() {
        let ctx = proving(
            "unreadable-stack",
            Arc::new(Recording::answering(Ok(spoke("")))),
            answering(200),
        );
        assert_eq!(
            counted(installing(&ctx, &source("unreadable-stack", PROVING)).await),
            Some(1)
        );
        let document = stack_of(&ctx).join("compose/plugins/komga.yml");

        let blind = a_context()
            .over(crate::test_support::nowhere())
            .settings(ctx.settings.clone())
            .build();
        assert_eq!(
            refusal(removing(&blind, "komga").await),
            crate::stack::STACK_UNREADABLE.to_string()
        );
        assert!(document.is_file(), "and nothing of its was taken");
    }

    /// A rehearsal says what would go back and touches none of it. A removal an
    /// operator has not agreed to yet is a reading, and a reading that removed a
    /// container would be the write done to describe itself.
    #[tokio::test]
    async fn rehearsing_a_removal_says_what_would_go_back_and_takes_nothing() {
        let runner = Arc::new(Recording::answering(Ok(spoke(""))));
        let ctx = proving("rehearsed-removal", runner.clone(), answering(200));
        assert_eq!(
            counted(installing(&ctx, &source("rehearsed-removal", PROVING)).await),
            Some(1)
        );
        let document = stack_of(&ctx).join("compose/plugins/komga.yml");

        let mut rehearsing = ctx;
        rehearsing.dry_run = true;
        let gone = removal(removing(&rehearsing, "komga").await);

        assert_eq!(gone.as_ref().map(|one| one.removed), Some(false));
        assert!(
            gone.is_some_and(|one| one.went_back.rehearsed && !one.went_back.reversed.is_empty()),
            "it names what would go back"
        );
        assert!(document.is_file(), "and none of it went");
        assert!(
            !runner.ran("rm"),
            "nothing was taken off the machine either"
        );
        assert_eq!(counted(reading(&rehearsing).await), Some(1));
    }

    /// A name nothing is installed under is refused, and the refusal says what is —
    /// because the commonest reason to reach it is a name spelled the way an operator
    /// remembers it rather than the way the plugin declares it.
    #[tokio::test]
    async fn removing_something_that_is_not_installed_is_refused_naming_what_is() {
        let ctx = proving(
            "not-installed",
            Arc::new(Recording::answering(Ok(spoke("")))),
            answering(200),
        );
        assert_eq!(
            refusal(removing(&ctx, "komga").await),
            "PLUGIN-10",
            "on a machine with nothing installed"
        );
        assert_eq!(
            counted(installing(&ctx, &source("not-installed", PROVING)).await),
            Some(1)
        );
        let (code, said) = refused(removing(&ctx, "komgaa").await);
        assert_eq!(code, "PLUGIN-10");
        assert!(
            said.contains("What is installed: komga"),
            "it names what is there: {said}"
        );
    }

    /// A container the engine would not take off is named as still standing, because
    /// *some of it worked* is the sentence that sends somebody looking by hand — and
    /// the exit status says so rather than reporting a removal that worked.
    #[tokio::test]
    async fn a_removal_whose_container_would_not_come_off_names_it_as_still_standing() {
        let runner = Keyed::answering(
            vec![("rm", Ok(engine_refused("no such container")))],
            Ok(spoke("")),
        );
        let ctx = proving("stuck-removal", runner, answering(200));
        assert_eq!(
            counted(installing(&ctx, &source("stuck-removal", PROVING)).await),
            Some(1)
        );

        let gone = removal(removing(&ctx, "komga").await);
        assert!(
            gone.as_ref().is_some_and(|one| one
                .went_back
                .left
                .iter()
                .any(|left| left.target == "komga"
                    && left.because.contains("could not be taken off"))),
            "the container is named with the reason it is still there"
        );
        assert!(
            gone.is_some_and(|one| one.removed),
            "and the record is written all the same: the files went back, so a plugin \
             the register still named would be a plugin nothing describes"
        );
    }

    /// A machine with nowhere to look for what was changed is refused by the rollback
    /// layer, in the rollback layer's own words. A removal that answered anyway would
    /// be reporting a plugin as gone on the strength of a record it never read.
    #[tokio::test]
    async fn a_removal_with_nowhere_to_look_for_the_record_is_refused_by_the_layer_that_looks() {
        let ctx = proving(
            "no-stack",
            Arc::new(Recording::answering(Ok(spoke("")))),
            answering(200),
        );
        assert_eq!(
            counted(installing(&ctx, &source("no-stack", PROVING)).await),
            Some(1)
        );

        let mut nowhere = ctx;
        nowhere.settings.stack_dir = None;

        assert_eq!(
            refusal(removing(&nowhere, "komga").await),
            "UNDO-4",
            "the layer that looks for the record is the one that says it cannot"
        );
    }

    /// A register that cannot be rewritten stops a removal the way it stops an
    /// install, and says what the run left.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_register_that_cannot_be_rewritten_stops_the_removal() {
        use std::os::unix::fs::PermissionsExt as _;

        let ctx = proving(
            "unwritable-removal",
            Arc::new(Recording::answering(Ok(spoke("")))),
            answering(200),
        );
        assert_eq!(
            counted(installing(&ctx, &source("unwritable-removal", PROVING)).await),
            Some(1)
        );
        let register = record_of(&ctx);
        assert!(
            std::fs::set_permissions(&register, std::fs::Permissions::from_mode(0o400)).is_ok()
        );

        let (code, said) = refused(removing(&ctx, "komga").await);
        assert_eq!(code, "PLUGIN-8");
        assert!(said.contains("was put back"), "{said}");

        let _ = std::fs::set_permissions(&register, std::fs::Permissions::from_mode(0o600));
    }

    /// **A removal inherits the rollback layer's refusals rather than restating them.**
    /// A setting somebody has set by hand since is drift, and putting it back would
    /// discard their edit — so the removal refuses and says what the file holds instead
    /// of overwriting it.
    #[tokio::test]
    async fn a_setting_edited_since_the_install_refuses_the_removal() {
        let ctx = proving(
            "drifted",
            Arc::new(Recording::answering(Ok(spoke("")))),
            answering(200),
        );
        assert_eq!(
            counted(installing(&ctx, &source("drifted", PROVING)).await),
            Some(1)
        );

        // A setting the plugin is on the record as having written, and an operator's
        // own value in the file where that change would be put back.
        let key = "LEMONFIBER_PLUGIN_TEST_KEY";
        journal_a_set(&ctx, "komga", key, "what the plugin wrote");
        let _ = ctx
            .settings
            .env_file
            .as_deref()
            .map(|file| crate::config::store::set(file, key, "what the operator wrote"));

        let (code, said) = refused(removing(&ctx, "komga").await);
        assert_eq!(
            code, "UNDO-3",
            "the rollback layer's own refusal, not a second one"
        );
        assert!(
            said.contains("somebody has set it since"),
            "and its own words: {said}"
        );
    }

    /// And the one refusal that is not a refusal: a change that re-points where data
    /// lives goes back and says plainly that the data does not move with it.
    #[tokio::test]
    async fn re_pointing_where_data_lives_says_the_data_does_not_move_back() {
        let ctx = proving(
            "repointed",
            Arc::new(Recording::answering(Ok(spoke("")))),
            answering(200),
        );
        assert_eq!(
            counted(installing(&ctx, &source("repointed", PROVING)).await),
            Some(1)
        );
        journal_a_set(
            &ctx,
            "komga",
            crate::config::DATA_ROOT_KEY,
            "/srv/elsewhere",
        );

        let gone = removal(removing(&ctx, "komga").await);
        assert!(
            gone.as_ref().is_some_and(|one| one.removed),
            "it is removed rather than refused"
        );
        assert!(
            gone.is_some_and(|one| one
                .went_back
                .noted
                .iter()
                .any(|note| note.because.contains("data does not move with it"))),
            "and the reversal says plainly that the library stays where it was moved to"
        );
    }

    /// **What a plugin contributed goes with it, and the run afterwards reads as one on
    /// a machine that never saw it.** Compared whole rather than checked for an absent
    /// row, because *answers exactly as* is a claim about the report and not about the
    /// absence of one line in it.
    ///
    /// Nothing withdraws anything. The rows a diagnosis runs are read from the register
    /// on every run, so a plugin taken out of the register takes its rows with it —
    /// which is why there is no withdrawal to get wrong.
    #[tokio::test]
    async fn what_a_plugin_contributed_goes_with_it_when_the_plugin_does() {
        let ctx = proving(
            "withdrawing",
            Arc::new(Recording::answering(Ok(spoke("")))),
            Fake::by_path(vec![
                (
                    "/api/v1/libraries",
                    lemonfiber_fixtures::http::Answer::reply(200, "[]"),
                ),
                (
                    "/api/v1/claim",
                    lemonfiber_fixtures::http::Answer::reply(200, "{}"),
                ),
            ]),
        );
        // Every finding but the one that counts lemonfiber's own files. A machine that
        // has installed and removed a plugin has a history of having done so, and the
        // journal holding it is one of the files whose permissions are checked. That is
        // a true fact about the machine and must not be erased: what the rule asks to go
        // is what the plugin contributed, not the record that it was here.
        let looked = || async {
            crate::app::diagnose(&ctx, &crate::doctor::Narrowing::Suite, false)
                .await
                .map(|report| {
                    report
                        .findings
                        .into_iter()
                        .filter(|finding| finding.check != "config.credential-permissions")
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        };

        let never = looked().await;
        assert!(!never.is_empty(), "the bundled rows ran");
        assert_eq!(
            counted(installing(&ctx, &source("withdrawing", CONTRIBUTING)).await),
            Some(1)
        );
        let holding = looked().await;
        assert_ne!(holding, never, "the plugin's row was there to be withdrawn");

        assert!(removal(removing(&ctx, "komga").await).is_some_and(|one| one.removed));
        assert_eq!(
            looked().await,
            never,
            "and a run after it reads as one on a machine that never saw it"
        );
    }

    /// The verdict is on the report, and so is what it was reached against — because a
    /// verdict against a recording the author shipped and one against the service on
    /// this machine are not the same claim, and a reader handed one has nothing else in
    /// the document to tell them apart.
    #[tokio::test]
    async fn the_report_carries_the_verdict_and_says_it_was_the_service_that_answered() {
        let ctx = proving(
            "against",
            Arc::new(Recording::answering(Ok(spoke("")))),
            answering(200),
        );
        let shown =
            report(installing(&ctx, &source("against", PROVING)).await).and_then(|one| one.install);

        assert_eq!(
            shown.as_ref().and_then(|one| one.against),
            Some(crate::plugin::Evidence::Service)
        );
        let stated: Vec<Option<Verdict>> = shown
            .map(|one| one.proofs.into_iter().map(|proof| proof.came_to).collect())
            .unwrap_or_default();
        assert_eq!(came_to(stated.first().and_then(Option::as_ref)), "held");
        assert!(
            why(stated.first().and_then(Option::as_ref)).is_empty(),
            "a proof that held carries no reason, because nothing stopped it"
        );
    }

    /// A proof the service refuses stops the install, and the install goes back: the
    /// container comes off the machine and every file it wrote is removed.
    #[tokio::test]
    async fn a_proof_the_service_refuses_puts_the_whole_install_back() {
        let runner = Arc::new(Recording::answering(Ok(spoke(""))));
        let ctx = proving("refuted", runner.clone(), answering(503));

        let outcome = installing(&ctx, &source("refuted", PROVING)).await;
        assert_eq!(
            came_to(verdicts(outcome).first().and_then(Option::as_ref)),
            "failed"
        );

        assert!(runner.ran("rm"), "its container came off the machine");
        assert!(
            !stack_of(&ctx).join("compose/plugins/komga.yml").exists(),
            "and the document it wrote is gone"
        );
        assert_eq!(
            counted(reading(&ctx).await),
            Some(0),
            "and nothing is installed"
        );
    }

    /// The reversal is the rollback layer's, reported in the rollback layer's own
    /// shape: what went back, and what did not with the reason each is standing.
    #[tokio::test]
    async fn what_a_failed_install_put_back_is_reported_as_the_reversal_it_was() {
        let ctx = proving(
            "reversed",
            Arc::new(Recording::answering(Ok(spoke("")))),
            answering(503),
        );
        let put_back = report(installing(&ctx, &source("reversed", PROVING)).await)
            .and_then(|one| one.install)
            .and_then(|one| one.reversed);

        let put_back = put_back.unwrap_or_default();
        assert!(!put_back.reversed.is_empty(), "what went back is named");
        assert!(put_back.left.is_empty(), "and nothing was left standing");
        assert!(put_back
            .reversed
            .iter()
            .any(|undo| undo.target.ends_with("komga.yml")));
    }

    /// A service that never answers is unproven rather than refuted, and the install
    /// goes back all the same. What it is called and what it costs are two decisions:
    /// nothing answered, so nothing was established, and installing over that would
    /// report an install as complete on the strength of a question nobody answered.
    #[tokio::test]
    async fn a_service_that_never_answers_is_unproven_and_the_install_still_goes_back() {
        let ctx = proving(
            "silent",
            Arc::new(Recording::answering(Ok(spoke("")))),
            Fake::silent(),
        );

        let outcome = installing(&ctx, &source("silent", PROVING)).await;
        assert_eq!(
            came_to(verdicts(outcome).first().and_then(Option::as_ref)),
            "unproven"
        );
        assert_eq!(counted(reading(&ctx).await), Some(0));
    }

    /// A service that has not answered *yet* is asked again. A container Compose has
    /// just created is not a service that is listening, and an install that took the
    /// first refusal would fail on every image that takes a moment to open its socket.
    #[tokio::test(start_paused = true)]
    async fn a_service_that_has_not_answered_yet_is_asked_again() {
        let http = Fake::by_path_in_turn(vec![(
            "/api/v1/libraries",
            vec![
                lemonfiber_fixtures::http::Answer::Silent,
                lemonfiber_fixtures::http::Answer::reply(200, "[]"),
            ],
        )]);
        let mut ctx = proving(
            "patient",
            Arc::new(Recording::answering(Ok(spoke("")))),
            http.clone(),
        );
        ctx.patience = std::time::Duration::from_secs(30);

        assert_eq!(
            counted(installing(&ctx, &source("patient", PROVING)).await),
            Some(1)
        );
        let asked = http
            .requests()
            .into_iter()
            .filter(|request| request.url.contains("/api/v1/libraries"))
            .count();
        assert_eq!(asked, 2, "the proof was asked twice");
    }

    /// A proof of a service that publishes no port is unproven naming it, because
    /// there is nowhere to ask rather than somewhere to guess.
    #[tokio::test]
    async fn a_proof_of_a_service_with_no_port_has_nowhere_to_ask() {
        let unpublished = PROVING.replace("port        = 25600\n", "");
        let unpublished = unpublished.replace("bind        = \"lan\"\n", "");
        let ctx = proving(
            "unpublished",
            Arc::new(Recording::answering(Ok(spoke("")))),
            Fake::silent(),
        );

        let stated = verdicts(installing(&ctx, &source("unpublished", &unpublished)).await);
        assert_eq!(came_to(stated.first().and_then(Option::as_ref)), "unproven");
        assert!(why(stated.first().and_then(Option::as_ref)).contains("publishes no port"));
    }

    /// A container that will not start stops the install by name, and the files it had
    /// already written go back.
    #[tokio::test]
    async fn a_service_that_will_not_start_stops_the_install_and_the_files_go_back() {
        let runner = Keyed::answering(
            vec![("up", Ok(engine_refused("no such image")))],
            Ok(spoke("")),
        );
        let ctx = proving("unstartable", runner, Fake::silent());
        let at = source("unstartable", PROVING);

        let (code, said) = refused(installing(&ctx, &at).await);
        assert_eq!(code, "PLUGIN-9");
        assert!(
            said.contains("was put back") && !said.contains("Still standing"),
            "a reversal that finished says so and names nothing as standing: {said}"
        );
        assert!(
            made_paths(&ctx)
                .iter()
                .all(|path| !std::path::Path::new(path).exists()),
            "every path this run made is gone"
        );
    }

    /// A reversal that could not take the container off says what is still standing,
    /// rather than repeating the sentence a reversal that finished would have got.
    #[tokio::test]
    async fn a_service_that_will_not_start_and_will_not_come_off_names_what_stands() {
        let runner = Keyed::answering(
            vec![
                ("up", Ok(engine_refused("no such image"))),
                ("rm", Ok(engine_refused("no such container"))),
            ],
            Ok(spoke("")),
        );
        let ctx = proving("standing", runner, Fake::silent());

        let (code, said) = refused(installing(&ctx, &source("standing", PROVING)).await);
        assert_eq!(code, "PLUGIN-9");
        assert!(
            said.contains("Still standing: komga"),
            "the container nothing could take off is named: {said}"
        );
    }

    /// A container engine that is not there at all is a different answer from one that
    /// ran and refused, and both stop the install — the engine's own words underneath
    /// the install's account rather than in place of it.
    #[tokio::test]
    async fn an_engine_that_cannot_be_run_at_all_stops_the_install() {
        let runner = Arc::new(Recording::answering(Err(
            lemonfiber_ports::process::Failure::NotFound {
                program: "docker".to_owned(),
            },
        )));
        let ctx = proving("no-engine", runner, Fake::silent());

        assert_eq!(
            beneath(installing(&ctx, &source("no-engine", PROVING)).await),
            ("PLUGIN-9".to_owned(), "PROC-1".to_owned()),
            "the install's own account leads, and what the engine said is carried \
             underneath it rather than dropped"
        );
    }

    /// A proof naming a method the transport cannot send is never put, and says so.
    /// Sending a different verb than the one written down would be proving something
    /// nobody declared.
    #[tokio::test]
    async fn a_proof_naming_a_method_lemonfiber_cannot_send_is_never_put() {
        let patched = PROVING.replace("method = \"GET\"", "method = \"PATCH\"");
        let ctx = proving(
            "patched",
            Arc::new(Recording::answering(Ok(spoke("")))),
            answering(200),
        );

        let stated = verdicts(installing(&ctx, &source("patched", &patched)).await);
        assert_eq!(came_to(stated.first().and_then(Option::as_ref)), "unproven");
        assert!(why(stated.first().and_then(Option::as_ref)).contains("PATCH"));
    }

    /// A container the engine would not take off the machine is named as still
    /// standing, because *some of it worked* is the sentence that sends somebody
    /// looking by hand.
    #[tokio::test]
    async fn a_container_that_would_not_come_off_is_named_as_still_standing() {
        let runner = Keyed::answering(
            vec![("rm", Ok(engine_refused("no such container")))],
            Ok(spoke("")),
        );
        let ctx = proving("stuck", runner.clone(), answering(503));

        let left = report(installing(&ctx, &source("stuck", PROVING)).await)
            .and_then(|one| one.install)
            .and_then(|one| one.reversed)
            .map(|back| back.left)
            .unwrap_or_default();
        assert_eq!(left.len(), 1);
        assert!(left
            .first()
            .is_some_and(|one| one.because.contains("could not be taken off")));
        assert!(
            runner.ran("rm"),
            "and it was asked, which is what makes the refusal the engine's rather than \
             an account of a call nobody made"
        );
    }

    /// An install that found everything it would write already there journals nothing
    /// of its own, so a reversal has no run of its stamp to put back. That is the true
    /// answer rather than a second failure: those files belong to the run that wrote
    /// them and are on the record under it.
    #[tokio::test]
    async fn an_install_that_wrote_nothing_has_nothing_of_its_own_to_put_back() {
        let ctx = proving(
            "leftovers",
            Arc::new(Recording::answering(Ok(spoke("")))),
            answering(503),
        );
        let stack = stack_of(&ctx);
        assert!(std::fs::create_dir_all(stack.join("config/komga")).is_ok());
        assert!(std::fs::create_dir_all(stack.join("compose/plugins")).is_ok());
        assert!(std::fs::write(stack.join("compose/plugins/komga.yml"), "services: {}\n").is_ok());

        let put_back = report(installing(&ctx, &source("leftovers", PROVING)).await)
            .and_then(|one| one.install)
            .and_then(|one| one.reversed)
            .unwrap_or_default();
        assert!(put_back.reversed.is_empty());
        assert!(put_back.left.is_empty());
    }

    /// A rehearsal asks nothing, so it states the proofs with no verdict against them
    /// and says nothing about what answered — which is a different fact from a proof
    /// that was asked and established nothing.
    #[tokio::test]
    async fn a_rehearsed_install_states_its_proofs_and_asks_none_of_them() {
        let ctx = {
            let mut ctx = proving(
                "unasked",
                Arc::new(Recording::answering(Ok(spoke("")))),
                Fake::silent(),
            );
            ctx.dry_run = true;
            ctx
        };
        let shown =
            report(installing(&ctx, &source("unasked", PROVING)).await).and_then(|one| one.install);

        assert_eq!(shown.as_ref().and_then(|one| one.against), None);
        let stated: Vec<Option<Verdict>> = shown
            .map(|one| one.proofs.into_iter().map(|proof| proof.came_to).collect())
            .unwrap_or_default();
        assert_eq!(stated.len(), 1, "the proof it would run is stated");
        assert_eq!(came_to(stated.first().and_then(Option::as_ref)), "unasked");
    }

    /// A stack directory is where a plugin's container has to go, so a machine
    /// without one is refused by name rather than installed half-way.
    #[tokio::test]
    async fn a_machine_with_no_stack_directory_refuses_the_install_naming_it() {
        let env_file = env_at("no-stack", &a_password());
        let ctx = a_context()
            .settings(crate::config::Settings {
                env_file: Some(env_file),
                stack_dir: None,
                ..crate::config::Settings::default()
            })
            .build();
        assert_eq!(
            refusal(installing(&ctx, &source("no-stack", MANIFEST)).await),
            "PLUGIN-6"
        );
    }

    /// And a rehearsal on that same machine is refused in the same words, because a
    /// rehearsal is the account the install then follows. One that answered here
    /// would be describing an operation this machine cannot carry out, and the
    /// operator would find that out on the run they thought they had checked. What
    /// answers with no machine at all is `plugin claims`, which is a different
    /// question.
    #[tokio::test]
    async fn a_rehearsal_is_refused_wherever_the_install_would_be() {
        let env_file = env_at("no-stack-rehearsed", &a_password());
        let mut ctx = a_context()
            .settings(crate::config::Settings {
                env_file: Some(env_file),
                stack_dir: None,
                ..crate::config::Settings::default()
            })
            .build();
        ctx.dry_run = true;
        assert_eq!(
            refusal(installing(&ctx, &source("no-stack-rehearsed", MANIFEST)).await),
            "PLUGIN-6"
        );
    }
}
