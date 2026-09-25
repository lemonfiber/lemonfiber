//! Writing a reviewed setup to disk, recoverably.
//!
//! The wizard's one phase that is not read-only: it takes a reviewed set of
//! answers and lands them as configuration. It is arranged so that stopping
//! anywhere in it is recoverable rather than wedged — the lifecycle marker moves
//! to `applying` and is persisted before the first write, each write is journalled
//! before it is made, and only once every write is done does the marker move to
//! `applied`. A run that stops in between is found on the next start as a failed
//! apply, with the journal holding exactly what to unwind.
//!
//! Applied here are the environment settings, the operator's data directory, and
//! the stack Compose reads — everything the stack needs on disk before it starts.
//! What is reversible is journalled: the settings and the data directory the
//! operator would not want silently left behind. The stack is lemonfiber's own
//! regenerable output and is not, since the next apply simply rewrites it.
//!
//! Recovery here is between whole writes, not within one. Each file is written in
//! place, so a stop in the middle of a single write can still tear that one file;
//! making each write atomic — a temporary file renamed over the target — is a
//! hardening the shared writer will grow, and is called out where it bites.

use std::path::{Path, PathBuf};

use crate::alert::{Appetite, Wants};
use crate::app::reconfiguring::SETTINGS;
use crate::autostart::Returning;
use crate::baseline::Baseline;
use crate::config::paths::Paths;
use crate::config::store::{self, is_secret};
use crate::error::codes::setup::{DIR_NOT_MADE, NOT_REVIEWED};
use crate::error::{Amiss, Diagnose, Problem, Remedy, Severity};
use crate::journal::{Change, Journal, Kind, Seal};
use crate::ports::random::Random;
use crate::quality::{Preset, Selection};
use crate::stack::{self, Source};
use crate::wizard::{Phase, Plan, Wizard};

/// Everything an apply writes *with*, as against the answers it writes.
///
/// A bundle rather than four more arguments, and it is the same argument the seams
/// bundle makes: these four are always supplied together, they are handed unchanged
/// down through review and recovery to the writes themselves, and named one at a time
/// they turn every function they pass through into a longer signature than the thing
/// it does. They are not seams — nothing here reaches the world — they are where this
/// machine keeps its files, where its stack comes from, what time it is, and what a
/// credential is sealed under.
pub struct Applying<'a> {
    /// Where lemonfiber's own files live.
    pub paths: &'a Paths,
    /// Where the stack this apply materialises comes from.
    pub source: Source,
    /// The time to stamp the journal with, which the wizard has no clock to do itself.
    pub stamp: &'a str,
    /// Where the key the journal's credentials are sealed under is drawn from.
    pub random: &'a dyn Random,
}

/// Write a reviewed setup to disk, driving the lifecycle and recording each
/// reversible write so an interrupted run can be unwound.
///
/// The wizard must be at review — every applicable question answered and
/// confirmed — or there is nothing settled to apply. Everything lands under the
/// install paths: the settings in the environment file, the lifecycle marker and
/// the change journal beside it, and the stack where Compose reads it. The
/// marker reaches `applying` on disk before the first write, and each journal entry
/// lands before the write it describes, so a stop at any point leaves a state the
/// next run can recover rather than one it must guess at.
///
/// A setting whose name says it holds a credential is journalled sealed rather than as
/// itself, under a key made from the randomness the bundle carries — see
/// [`crate::journal::sealing`] for what that does and does not protect against.
///
/// # Errors
///
/// Returns a [`Problem`] where review has not been reached, where a file could not
/// be read or written, where the data directory could not be created, or where the
/// stack could not be materialised — leaving the marker at `applying` and the
/// journal holding what had been written, which the next run recovers from.
pub fn apply(wizard: &mut Wizard, applying: &Applying) -> Result<(), Box<Problem>> {
    if !wizard.transition(Phase::Applying) {
        return Err(Box::new(not_reviewed()));
    }
    // Every failure is boxed as one problem on the way out, because a `Problem` is
    // large beside the `()` this returns on success — so the writes themselves stay
    // a plain sequence, each stopping the rest.
    write(wizard, applying).map_err(|fault| Box::new(fault.problem()))
}

/// A failure part-way through applying, named finely enough to remedy: a file the
/// config store could not read or write, a data directory that could not be made,
/// or the stack that could not be written out. Carried so the one boxing site
/// above turns whichever it was into a problem, rather than each write growing its
/// own.
enum Fault {
    /// A configuration, progress, or journal file could not be read or written.
    Store(store::Failure),
    /// The operator's chosen data directory could not be created.
    DirNotMade {
        /// The directory that could not be made.
        path: PathBuf,
        /// The operating system's own words.
        reason: String,
    },
    /// The stack could not be materialised where Compose reads it.
    Stack(stack::Failure),
}

impl Fault {
    /// The problem to report, in the words that fit what actually failed.
    fn problem(&self) -> Problem {
        match self {
            Self::Store(failure) => failure.problem(),
            Self::Stack(failure) => failure.problem(),
            Self::DirNotMade { path, reason } => Problem::new(
                DIR_NOT_MADE,
                Severity::Error,
                "The data directory could not be created",
                "lemonfiber makes the directory the library and downloads live under before it starts anything. Setup has stopped, and the next run recovers it.",
                Remedy::new("Check the location is on a writable disk and try again"),
            )
            .with_detail(format!("{}: {reason}", path.display())),
        }
    }
}

/// Perform the writes of an apply, stopping at the first that fails.
///
/// Ordered for recovery: the applying marker is persisted first, each change is
/// journalled before it is written, and the applied marker is persisted last — so
/// a stop at any point leaves the marker and journal a later run reads.
fn write(wizard: &mut Wizard, applying: &Applying) -> Result<(), Fault> {
    let (paths, stamp, random) = (applying.paths, applying.stamp, applying.random);
    let (progress, env_file, journal) = (paths.setup_progress(), paths.env_file(), paths.journal());
    let seal = Seal::minted(&journal, random);

    store::write(&progress, &rendered(wizard)).map_err(Fault::Store)?;

    // The whole plan is diffed against the file as it stands before any write, so
    // every recorded `previous` is what was really there. The plan's keys are all
    // distinct, so no write moves a `previous` out from under a later one.
    let before = store::read(&env_file).map_err(Fault::Store)?;
    let plan = wizard.plan();
    let changes = plan.changes(&before, stamp);

    // What is already in the journal stays in it. A resumed apply, a recovered one and
    // setup run again all arrive here on a machine that may have a record of earlier
    // changes, and starting from an empty one would write that record over.
    let mut log = super::recover::journal_at(&journal).map_err(Fault::Store)?;

    // The library and downloads live under the operator's chosen location, so it
    // must exist before anything mounts it — but only where it does not already.
    // A location that is already there is the operator's own (a library to adopt),
    // left untouched and unrecorded, so unwinding never removes it. The chosen
    // location is iterated rather than matched, so a reviewed wizard that always
    // has one needs no unreachable "no location" branch.
    for root in data_root(wizard).into_iter().filter(|root| !root.exists()) {
        // Making the leaf makes every missing ancestor with it, so each one is
        // recorded — parent before child, to be unwound child-first — and none
        // lemonfiber creates is left without a way back. Journalled before the
        // directories are made, so a stop between records paths that may not exist
        // yet, whose removal is a harmless no-op.
        for ancestor in made_by(&root) {
            log.record(made(&ancestor, stamp));
        }
        store::write(&journal, &lines(&log, &seal, random)).map_err(Fault::Store)?;
        std::fs::create_dir_all(&root).map_err(|err| Fault::DirNotMade {
            path: root,
            reason: err.to_string(),
        })?;
    }

    // The stack is written where Compose reads it, through the same file-by-file
    // comparison every later run makes rather than round it. This was a straight
    // extraction over the top for a long time, on the stated grounds that an embedded
    // stack holds nothing an operator could lose. That is true of a first install and
    // of nothing else this function is reached by: a resumed apply, a recovered one,
    // and setup run again on a machine that already has a stack all arrive here with
    // files on disk that may be somebody's own, and an extraction cannot tell them
    // from a version that has not been upgraded yet.
    //
    // It stays out of the journal for the half of that reasoning that does hold: what
    // is written is regenerable, and its undo is that the next apply writes it again.
    // An edit the comparison declines to overwrite comes back with its diff and is not
    // carried further from here — nothing was overwritten, so nothing is owed a diff
    // at this point, and the next lifecycle command reports exactly these files and
    // exactly these diffs through the surface that already carries them.
    //
    // The default preset rather than no choice at all, because no choice means "leave
    // the quality config exactly as it is on disk", and at setup there may be nothing
    // there to leave. It rewrites the shipped config to itself, so an install that
    // chose nothing gets the file the stack ships.
    super::materialise::materialise(
        applying.source,
        Some(&paths.stack()),
        Some(&paths.materialised()),
        Some(&Selection::everywhere(Preset::default_preset())),
        // Read from the file as it stands rather than assumed empty. A first install
        // has declared nothing, and this function is reached by a resumed apply, a
        // recovered one, and setup run again on a machine that has been set up for
        // months — where a declaration may well be the reason a file is the way it is.
        &crate::unmanaged::parse(before.get(crate::config::UNMANAGED_KEY).unwrap_or_default()),
    )
    .map_err(Fault::Stack)?;

    // `changes` is one entry per setting in the same order, so each pairs with the
    // key and value it was built from; they are two views of the same list, walked
    // together.
    for (change, (key, value)) in changes.into_iter().zip(plan.settings()) {
        // Journalled before it is written: a run that dies between the two leaves a
        // record of a change that may not have landed, and undoing that restores
        // what was already there — harmless. The reverse would leave a real write
        // with nothing to unwind it.
        log.record(change);
        store::write(&journal, &lines(&log, &seal, random)).map_err(Fault::Store)?;
        store::set(&env_file, key, value).map_err(Fault::Store)?;
    }

    // What lemonfiber wrote, recorded as the expected state a later change compares
    // against. Without it, the first change made to a setting cannot tell an
    // operator's hand-edit from the value setup itself put there, and would
    // overwrite one without saying so.
    remembered(paths, &plan, stamp);

    // The notification appetite is its own file rather than an environment
    // setting, because it grows: the preset chosen here is one answer, and the
    // individual events switched on or off later live beside it. Written before
    // the applied marker so a completed apply always has one.
    let wants = Wants::preset(
        wizard
            .answers()
            .notifications
            .unwrap_or_else(Appetite::default_appetite),
    );
    store::write(
        &paths.notifications(),
        &serde_json::to_string(&wants).unwrap_or_default(),
    )
    .map_err(Fault::Store)?;

    // The autostart answer, for the reason the appetite is written: setup asks the
    // question and stated what declining costs, and an answer gathered under that
    // sentence and then dropped leaves the operator believing they decided
    // something. Its own record rather than an environment setting because Compose
    // has no use for it, and unanswered reads as declined — the direction that
    // starts nothing on a machine nobody asked to have started.
    //
    // Merged into whatever is already there rather than written over it, because the
    // rest of that record is written by running the stack: which form was last up, and
    // whether the last teardown was one the operator asked for. Setup run a second time
    // over a machine that has been used must not take those with it.
    let returning =
        Returning::at(&paths.autostart()).answering(wizard.answers().autostart.unwrap_or(false));
    store::write(
        &paths.autostart(),
        &serde_json::to_string(&returning).unwrap_or_default(),
    )
    .map_err(Fault::Store)?;

    // The applied marker lands only after every setting is on disk, so a stop
    // before it leaves `applying` over a complete file rather than `applied` over
    // an incomplete one — the next run treats that as a failed apply and offers to
    // resume, which keeps the writes, rather than trusting a half-written stack.
    wizard.transition(Phase::Applied);
    store::write(&progress, &rendered(wizard)).map_err(Fault::Store)
}

/// Record the settings this apply wrote, as what lemonfiber last put in the file.
///
/// Merged into whatever record is already there rather than written over it, so an
/// apply beside an already-seeded stack does not take the services' records with it.
/// A record that is there but unreadable is left alone for the reason seeding leaves
/// one: it may hold what a later run needs, and silently replacing it is worse than
/// not adding to it.
///
/// Credentials are deliberately left out — a second file holding a password would be
/// a second file to leak one — and best-effort, like every other record kept beside
/// the settings: an apply that could not write it still applied.
fn remembered(paths: &Paths, plan: &Plan, stamp: &str) {
    let path = paths.baseline();
    let mut baseline = match std::fs::read_to_string(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Baseline::new(),
        Err(_) => return,
        Ok(text) => match serde_json::from_str(&text) {
            Ok(baseline) => baseline,
            Err(_) => return,
        },
    };
    for (key, value) in plan.settings().iter().filter(|(key, _)| !is_secret(key)) {
        baseline.record(SETTINGS, key, value, stamp);
    }
    let _ = store::write(&path, &serde_json::to_string(&baseline).unwrap_or_default());
}

/// The data location the operator chose, where they chose one.
fn data_root(wizard: &Wizard) -> Option<PathBuf> {
    wizard.answers().data_location.clone()
}

/// The directories creating `root` will make: every ancestor from the first that
/// is missing down to `root`, parent before child.
///
/// `create_dir_all` makes the whole missing chain, not just the leaf, so all of it
/// is what a reversal must remove. Ordered parent-first here, it is recorded that
/// way and so unwound child-first. `root` is known not to exist when this is
/// called, so the walk always yields at least it.
fn made_by(root: &Path) -> Vec<PathBuf> {
    let mut making = Vec::new();
    let mut here = Some(root);
    while let Some(path) = here.filter(|path| !path.exists()) {
        making.push(path.to_path_buf());
        here = path.parent();
    }
    making.reverse();
    making
}

/// The journal entry for a directory apply created, so it can be removed again.
fn made(path: &Path, stamp: &str) -> Change {
    let path = path.display().to_string();
    Change {
        at: stamp.to_owned(),
        operation: "apply".to_owned(),
        target: path.clone(),
        kind: Kind::Made { path },
    }
}

/// The wizard's progress as the single JSON object the recovery frame reads back.
///
/// A `Progress` cannot fail to serialise, so the empty-string fallback is a shape
/// the type never takes rather than a loss to guard against.
fn rendered(wizard: &Wizard) -> String {
    serde_json::to_string(wizard.progress()).unwrap_or_default()
}

/// The journal as one JSON object per line, the form the recovery frame reads.
///
/// As with the progress, a `Change` cannot fail to serialise, so no line is ever
/// the empty-string fallback.
///
/// Sealed on the way out rather than where the change was made, so the record this run
/// holds in memory is exact and only the file is not: an apply that stops part-way
/// reverses its own writes from what it is holding, credentials and all, and what
/// outlives the run is what has them taken out of it.
fn lines(journal: &Journal, seal: &Seal, random: &dyn Random) -> String {
    journal
        .changes()
        .iter()
        .map(|change| serde_json::to_string(&seal.sealing(change, random)).unwrap_or_default())
        .collect::<Vec<_>>()
        .join("\n")
}

/// The problem of applying before review — nothing is settled to write.
///
/// Visible to setup because a rehearsal of an apply has to be refused in the same
/// words at the same point. A refusal written twice is two sentences that start the
/// same and stop matching.
pub(crate) fn not_reviewed() -> Problem {
    Problem::new(
        NOT_REVIEWED,
        Severity::Error,
        "Setup cannot be applied before it is reviewed",
        "Applying writes the answers to disk, so it runs only once they are all gathered and confirmed. Nothing has been written.",
        Remedy::new("Answer every question, then confirm the review before applying"),
    )
    .lies_in(Amiss::Asking)
}

#[cfg(test)]
mod tests;
