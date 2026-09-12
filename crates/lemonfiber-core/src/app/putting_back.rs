//! Putting back one run of changes, named by the stamp the history shows.
//!
//! Between `doctor --undo`, which reverses the last repair and nothing else, and
//! `rewind`, which unwinds the whole journal, sat the thing an operator actually asks
//! for: *that* seed, *that* reconfigure — the one they can see in the history and now
//! regret. This is that middle, and it is the same executor underneath: the changes that
//! live inside a service go back through that service, and what is left goes back on the
//! host.
//!
//! A run, never a change on its own. The operation and the stamp together are the unit
//! an operator agreed to, and undoing half of one leaves a machine in a state nobody
//! chose — so naming any change of a run names the run, which is what the history's
//! "alongside" count tells them before they ask.
//!
//! Nothing is put back until every change of the run has been judged. A reversal that
//! carried out three of five and then met one it could not is a machine in a state
//! nobody has been told about, and the judgement is already available without touching
//! anything.

use crate::error::{Code, Diagnose as _, Problem, Remedy, Severity, State};
use crate::journal::{Change, Undo};
use crate::rollback::{standing, together, Reversal as Judgement};

use super::Ctx;

/// What putting a run back came to.
///
/// A report rather than a bare list, because it is what an envelope carries and an
/// envelope carries a document. Two lists, and the second is the one that matters when
/// it is not empty: what went back, and what did not with the reason it did not.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, schemars::JsonSchema)]
pub struct Reversal {
    /// What was put back, in the order it was.
    pub reversed: Vec<Undo>,
    /// What was not put back, each with the reason it was not.
    ///
    /// A reversal an operator asked for by name has to say what it did *not* do. Five
    /// changes asked back and three carried out is a machine in a state nobody has been
    /// told about, and "some of it worked" is the sentence that makes somebody go
    /// looking by hand. Empty where everything went back, which is the common case.
    pub left: Vec<Left>,
}

/// One change a reversal did not put back, and why it did not.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
pub struct Left {
    /// What the change was against — a service, or lemonfiber's own environment file.
    pub target: String,
    /// Why it is still standing, in the operator's terms.
    pub because: String,
}

/// Raised when no run carries the stamp a reversal was asked for.
pub const NO_SUCH_RUN: Code = Code::new("UNDO-1");

/// Raised when a stamp names more than one run, so which to put back is not settled.
pub const MORE_THAN_ONE_RUN: Code = Code::new("UNDO-2");

/// Raised when a run cannot be put back, carrying the reason it cannot.
pub const CANNOT_SUCCEED: Code = Code::new("UNDO-3");

/// Raised when a run cannot say where lemonfiber's own files are.
pub const NOWHERE_TO_LOOK: Code = Code::new("UNDO-4");

/// The operation a reversal records its own work under, so it can be put back in turn.
pub const OPERATION: &str = "undo";

/// Put back the last repair, or the run a stamp names.
///
/// # Errors
///
/// Returns a [`Problem`] where the run cannot be found, where the stamp names more than
/// one, where the judgement says the run cannot be put back, or for any reason the
/// executor underneath gives.
pub async fn undo(ctx: &Ctx, run: Option<String>) -> Result<super::Outcome, Box<Problem>> {
    reversing(ctx, run.as_deref())
        .await
        .map(super::Outcome::Undo)
}

/// Put back the last repair, or the run a stamp names.
///
/// # Errors
///
/// Returns a [`Problem`] where the run cannot be found, where the stamp names more than
/// one, where the judgement says the run cannot be put back, or for any reason the
/// executor underneath gives.
pub async fn reversing(ctx: &Ctx, run: Option<&str>) -> Result<Reversal, Box<Problem>> {
    match run {
        None => super::repair::reversing(ctx).await,
        Some(at) => named(ctx, at).await,
    }
}

/// Put back the run stamped `at`.
async fn named(ctx: &Ctx, at: &str) -> Result<Reversal, Box<Problem>> {
    let paths = super::targets::layout(ctx).ok_or_else(|| Box::new(nowhere_to_look()))?;
    let journal = super::recover::journal_at(&paths.journal());
    let changes = journal.changes();

    let operation = operation_at(changes, at)?;
    let run: Vec<&Change> = together(changes, &operation, at);

    // Read once rather than per change: the drift question asks the same file as many
    // times as there are entries otherwise.
    let holds = |key: &str| -> Option<String> {
        let file = ctx.settings.env_file.as_ref()?;
        crate::config::store::read(file)
            .ok()?
            .get(key)
            .map(str::to_owned)
    };

    // Judged whole before anything is touched. The refusal carries the reason the
    // judgement gave and what to do instead, which for a change nothing here can put
    // back is the only useful half of the answer.
    for change in &run {
        let position = changes
            .iter()
            .position(|held| held == *change)
            .unwrap_or(changes.len());
        let later = changes.get(position + 1..).unwrap_or_default();
        let verdict = standing(change, later, &holds);
        if verdict.reversal == Judgement::None {
            // The reason and what to do instead, joined rather than branched on: a
            // refusal carrying no remedy is a shape this judgement does not produce, and
            // a branch for it would be a line no test could ever reach.
            let why = verdict
                .refusal
                .map(|refusal| {
                    [Some(refusal.because), refusal.instead]
                        .into_iter()
                        .flatten()
                        .collect::<Vec<String>>()
                        .join(" — ")
                })
                .unwrap_or_default();
            return Err(Box::new(cannot_succeed(at, &change.target, &why)));
        }
    }

    let undos: Vec<Undo> = run.iter().map(|change| change.undo()).collect();
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| Box::new(err.problem()))?;
    let project = super::targets::project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref());
    let reached =
        super::recover::reconfigured(ctx, &undos, &manifest.services, project.as_deref()).await;
    let carried = super::recover::carrying_out(&reached.left, &paths.env_file(), Vec::new())?;

    let mut reversed = reached.put_back;
    reversed.extend(carried.done.iter().cloned());

    // Recorded before the report is built, so a reversal that is reported is a reversal
    // that is in the record. The changes it writes are the inverse of the ones it put
    // back, which is what makes this run answerable to the same command.
    super::recover::journalled(
        &paths.journal(),
        &recording(&run, &reversed, &ctx.stamp()),
        ctx.random.as_ref(),
    );

    Ok(Reversal {
        reversed,
        left: standing_after(&reached.unreached, &carried),
    })
}

/// The one operation a stamp names, refusing where it names none or several.
fn operation_at(changes: &[Change], at: &str) -> Result<String, Box<Problem>> {
    let mut named: Vec<&str> = changes
        .iter()
        .filter(|change| change.at == at)
        .map(|change| change.operation.as_str())
        .collect();
    named.sort_unstable();
    named.dedup();
    match named.as_slice() {
        [] => Err(Box::new(no_such_run(at))),
        [one] => Ok((*one).to_owned()),
        several => Err(Box::new(more_than_one(at, several))),
    }
}

/// What a reversal left standing, as the report says it.
///
/// Four lists and three reasons, walked as one: a change only a service can undo where
/// that service did not answer, a setting somebody has chosen since, and a sealed record
/// that would not open. The last two cannot arise on this path — the judgement above
/// refuses a run holding either before anything is touched — but they are carried rather
/// than dropped, because the executor can still meet one and a report that silently lost
/// it would be the thing this field exists to prevent.
fn standing_after(unreached: &[String], carried: &super::recover::Carried) -> Vec<Left> {
    [
        (unreached, "the service that made it did not answer"),
        (
            carried.beyond_reach.as_slice(),
            "the service that made it did not answer",
        ),
        (
            carried.theirs.as_slice(),
            "it holds something chosen since, which putting this back would discard",
        ),
        (
            carried.unread.as_slice(),
            "the record of what it held is sealed under a key this machine no longer has",
        ),
    ]
    .into_iter()
    .flat_map(|(targets, because)| {
        targets.iter().map(move |target| Left {
            target: target.clone(),
            because: because.to_owned(),
        })
    })
    .collect()
}

/// The record a reversal keeps of itself, so it can be put back in turn.
///
/// One entry per change actually put back, with the values the other way round: what the
/// original change wrote is what this one found, and what it restored is what stands now.
/// Asking for *this* run then puts the original values back, which is what makes a
/// reversal answerable to the same command as everything else.
///
/// Two of the four kinds record nothing, and the reason is the journal's vocabulary
/// rather than an omission. A path that was made is gone once the reversal removes it,
/// and `Made` would say it had been created; what a service created is never put back at
/// all, so there is nothing to record. Neither has an inverse this record can spell, and
/// inventing one would put a line in the journal that replaying would not reproduce.
fn recording(run: &[&Change], reversed: &[Undo], at: &str) -> Vec<Change> {
    run.iter()
        .filter(|change| reversed.iter().any(|undo| undo.target == change.target))
        .filter_map(|change| {
            inverted(&change.kind).map(|kind| Change {
                at: at.to_owned(),
                operation: OPERATION.to_owned(),
                target: change.target.clone(),
                kind,
            })
        })
        .collect()
}

/// One change with the values it moved between swapped, where that can be said at all.
fn inverted(kind: &crate::journal::Kind) -> Option<crate::journal::Kind> {
    use crate::journal::Kind;
    match kind {
        Kind::Set {
            key,
            previous,
            current,
        } => Some(Kind::Set {
            key: key.clone(),
            previous: Some(current.clone()),
            current: previous.clone().unwrap_or_default(),
        }),
        Kind::Configured {
            resource,
            id,
            field,
            previous,
            current,
        } => Some(Kind::Configured {
            resource: resource.clone(),
            id: id.clone(),
            field: field.clone(),
            previous: Some(current.clone()),
            current: previous.clone().unwrap_or_default(),
        }),
        Kind::Created { .. } | Kind::Made { .. } | Kind::Pinned { .. } => None,
    }
}

fn nowhere_to_look() -> Problem {
    Problem::new(
        NOWHERE_TO_LOOK,
        Severity::Error,
        "This run has nowhere it knows to look for what was changed",
        "What lemonfiber changed is recorded in its own directory, and this machine \
         would not say where that is. Nothing was put back.",
        Remedy::new("Set a home directory for this user and run it again"),
    )
    .in_state(State::Guided)
}

fn no_such_run(at: &str) -> Problem {
    Problem::new(
        NO_SUCH_RUN,
        Severity::Error,
        format!("Nothing was changed at {at}"),
        format!(
            "No run in the record carries the stamp {at}. It may have fallen outside the \
             horizon the record keeps, or the stamp may be mistyped. Nothing was put back."
        ),
        Remedy::new("Run `lemonfiber history` and take the stamp from the entry you want"),
    )
    .in_state(State::Actionable)
}

fn more_than_one(at: &str, operations: &[&str]) -> Problem {
    Problem::new(
        MORE_THAN_ONE_RUN,
        Severity::Error,
        format!("More than one run is stamped {at}"),
        format!(
            "{at} names {}, and putting back the wrong one is not something to guess at. \
             Nothing was put back.",
            operations.join(" and ")
        ),
        Remedy::new("Ask for one of them by name once the surfaces carry it"),
    )
    .in_state(State::Actionable)
}

fn cannot_succeed(at: &str, target: &str, why: &str) -> Problem {
    Problem::new(
        CANNOT_SUCCEED,
        Severity::Error,
        format!("The run stamped {at} cannot be put back"),
        format!(
            "One of its changes, against {target}, cannot be reversed: {why}. A run goes \
             back whole or not at all, so nothing was put back."
        ),
        Remedy::new("Deal with that change first, or restore from a backup"),
    )
    .in_state(State::Actionable)
}
