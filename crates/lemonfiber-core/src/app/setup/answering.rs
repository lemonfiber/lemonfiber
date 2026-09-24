//! Driving the wizard one answer at a time, for a surface that does not hold the
//! walk.
//!
//! A terminal keeps the whole conversation in one process: it asks, waits, and
//! asks again, and the wizard lives on its stack for the length of it. A request
//! cannot. So the walk is taken a step per call — where am I, here is one answer,
//! back one, apply — and between calls the accumulated answers live in the one
//! file setup is already allowed to write before review.
//!
//! **The progress file is the state, and nothing else is.** It is what a resumed
//! terminal run reads, so a setup begun in a browser is one a terminal finishes
//! and the other way round, and two windows open on the same machine are looking
//! at one run rather than two. A copy held in the server would die with the
//! process; a copy carried by the caller would be a second store beside this one,
//! and would put every gathered secret back on the wire on every call.
//!
//! Nothing here decides anything the wizard decides. Which question comes next,
//! which apply at all, what an answer may be and what the answers add up to are
//! all read off the wizard; this reads the file, hands the wizard what arrived,
//! and writes the file back.

use crate::app::apply::Applying;
use crate::app::targets::layout;
use crate::app::Ctx;
use crate::config::paths::Paths;
use crate::config::store;
use crate::error::{Amiss, Code, Diagnose, Problem, Remedy, Severity, State};
use crate::model::{SettingReport, WizardReport};
use crate::validate::{Credential, Validation, Validator};
use crate::wizard::{
    offer_setup, Answer, Choice, Indexer, Phase, Progress, Provider, Status, Wizard,
};

/// What a setup request asks of the wizard.
///
/// The whole of the walk a surface drives, and no more of it: the informing steps
/// are passed with [`Self::Next`] because they have no answer to give, and
/// applying is its own request because it is the one that writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SetupAction {
    /// Where setup stands. Changes nothing, and refuses nothing.
    Where,
    /// Record one answer, and move on to the next question that applies.
    Answer(Answer),
    /// Move on without answering — how a step that only informs is passed.
    Next,
    /// Move back to the previous question that applies.
    Back,
    /// Write the answers, once every applicable question has one.
    Apply,
    /// Take an apply that stopped part-way the way the operator chose out of it.
    ///
    /// Its own request rather than a kind of apply, because the three ways out are
    /// not three ways of applying: one carries on, one reverses what was written and
    /// applies again, and one reverses it and forgets the answers. What was already
    /// written is on the report, so the choice is made by somebody who has seen it.
    Recover(Choice),
}

/// Carry out one step of setup and say where that leaves it.
///
/// # Errors
///
/// Returns a [`Problem`] where there is nowhere to keep configuration, where this
/// machine is already set up and nothing is part-way through, where the answer does
/// not apply on this platform, or where applying failed — the marker left for
/// recovery in that last case, exactly as a terminal run leaves it.
pub async fn setting_up(ctx: &Ctx, action: SetupAction) -> Result<WizardReport, Box<Problem>> {
    let Some(paths) = layout(ctx) else {
        return Err(Box::new(store::Failure::Nowhere.problem()));
    };
    let saved = super::progress_at(&paths.setup_progress());

    // Asked before anything is read into a wizard: a machine that is already set
    // up has no questions left to put, and answering them again would walk a
    // working stack back to its first one. A read is exempt because whether setup
    // is on offer is precisely what it answers.
    if !matches!(action, SetupAction::Where) && !open(saved.as_ref(), &paths) {
        return Err(Box::new(already_set_up()));
    }

    let mut wizard = saved.map_or_else(
        || Wizard::new(ctx.environment),
        |progress| Wizard::resume(ctx.environment, progress),
    );

    let mut proof = None;
    match action {
        SetupAction::Where => {}
        SetupAction::Answer(answer) => {
            let (answer, came_to) = proven(ctx.validator.as_ref(), answer).await;
            proof = came_to;
            wizard
                .answer(answer)
                .map_err(|rejected| Box::new(super::does_not_apply(rejected)))?;
            wizard.advance();
            kept(ctx, &wizard, &paths);
        }
        SetupAction::Next => {
            wizard.advance();
            kept(ctx, &wizard, &paths);
        }
        SetupAction::Back => {
            wizard.back();
            kept(ctx, &wizard, &paths);
        }
        // The same apply a terminal run reaches, at the same gate: review is
        // entered only from a complete set of answers, and applying anything else
        // is refused there rather than judged again here. A rehearsal meets that gate
        // and stops at it, so what it answers with is the reviewed plan — every
        // setting apply would write, with the credentials among them withheld — and
        // not one file of it is on disk.
        SetupAction::Apply if ctx.dry_run => super::would_apply(&mut wizard)?,
        SetupAction::Apply => {
            let stamp = ctx.stamp();
            super::resume(&mut wizard, &applying(ctx, &paths, &stamp))?;
        }
        // Refused where nothing is part-way through: the three ways out are about a
        // half-written apply, and offering them for a run that has not begun one
        // would reverse changes nothing made and discard answers nobody replaced.
        //
        // That refusal is the half a rehearsal keeps, because it is a fact about the
        // machine rather than a consequence of acting. What it leaves out is the
        // reversal and the apply behind it, and what it answers with is the list the
        // choice is being made about: an interrupted apply's own record of what it
        // wrote, which is on the report either way.
        SetupAction::Recover(choice) => {
            if wizard.phase() != Phase::Applying {
                return Err(Box::new(nothing_to_recover()));
            }
            if !ctx.dry_run {
                let stamp = ctx.stamp();
                super::recovered(&mut wizard, &applying(ctx, &paths, &stamp), choice)?;
                // Starting over forgot the answers, so the walk is back at its
                // beginning — and a report still reading them off the wizard in hand
                // would describe a run that no longer exists anywhere.
                if choice == Choice::StartOver {
                    wizard = Wizard::new(ctx.environment);
                }
            }
        }
    }

    Ok(reported(&wizard, &paths, proof))
}

/// Keep what has been answered so far, unless this run is only saying what it would do.
///
/// The progress file is the state and nothing else is, so writing it is what makes an
/// answer an answer. A rehearsal walks the wizard the same step and leaves the file
/// where it was, so the report says where that answer would put the walk while the next
/// run still finds the question unanswered — which is what somebody asking what an
/// answer *would* do has asked for.
fn kept(ctx: &Ctx, wizard: &Wizard, paths: &Paths) {
    if !ctx.dry_run {
        super::save(wizard, paths);
    }
}

/// What an apply reached from a request writes with.
///
/// Built here rather than carried, because everything in it is already on the context
/// or beside it: a request has no more to say about where these files live, where the
/// stack comes from, or what time it is than a terminal run does.
///
/// The stamp is taken by the caller and lent in. A bundle that stamped itself would
/// have a value of its own to keep alive, and this is handed to one call and dropped.
fn applying<'a>(ctx: &'a Ctx, paths: &'a Paths, stamp: &'a str) -> Applying<'a> {
    Applying {
        paths,
        source: ctx.stack,
        stamp,
        random: ctx.random.as_ref(),
    }
}

/// Whether setup may still be answered or applied on this machine.
///
/// A run to pick up beats a machine that looks configured, and is asked first for
/// that reason: an apply that stopped part-way has written settings that the
/// configured-yet question reads as a finished install.
fn open(saved: Option<&Progress>, paths: &Paths) -> bool {
    Status::of(saved).unfinished() || offer_setup(paths.env_file().exists())
}

/// Where the walk stands now, read off the wizard and the files rather than
/// remembered.
///
/// The progress is read back rather than taken from the wizard in hand, because a
/// finished apply removes it — so what this reports is what the next run will
/// find, which is the thing a surface is deciding on.
fn reported(wizard: &Wizard, paths: &Paths, proof: Option<Validation>) -> WizardReport {
    let saved = super::progress_at(&paths.setup_progress());
    WizardReport {
        // Withheld here as well as where the outcome was made, because a validator is a
        // port and whoever supplies one decides what it says. This is first-run setup —
        // the minutes in which the credentials are entered — and what it answers is
        // serialised to a caller that may log it.
        proof: proof.map(Validation::withheld),
        written: if wizard.phase() == Phase::Applying {
            super::written_so_far(paths)
        } else {
            Vec::new()
        },
        offered: open(saved.as_ref(), paths),
        phase: wizard.phase(),
        at: wizard.at(),
        asks: wizard.at().is_question(),
        unanswered: wizard.unanswered(),
        ready_for_review: wizard.ready_for_review(),
        // Withheld the way `config show` withholds, through the same rule: this is
        // the one place setup's answers are read back out, and an indexer key or a
        // provider password in it would be one a caller could log.
        plan: wizard
            .plan()
            .settings()
            .iter()
            // The operator's, and said so before the write rather than after: a plan
            // is what their own answers came to, so the origin they will read back
            // off the listing afterwards is the one they are shown here.
            .map(|(key, value)| {
                SettingReport::of(store::showing(key, value), crate::origin::Origin::Operator)
            })
            .collect(),
    }
}

/// The answer as this path may record it, and what proving it came to.
///
/// A credential is tested against its live service before it is kept, the way a
/// terminal run tests one the moment it is entered — so `validated` records what a
/// test established rather than what a caller asserted. Taken as submitted it would
/// let a caller claim a key works by saying it does, and a later diagnosis reads
/// that flag to decide whether to trust one.
///
/// A test that does not prove it is not a refusal. The answer is kept unproven and
/// what the service said comes back beside it, which leaves whoever asked the three
/// ways out a terminal run offers: send a different credential, send none, or go on
/// with one that is recorded as unverified.
///
/// Every other answer is about this machine rather than about a service, so there is
/// nothing to ask and nothing comes back.
async fn proven(validator: &dyn Validator, answer: Answer) -> (Answer, Option<Validation>) {
    match answer {
        Answer::Credentials(Some(indexer)) => {
            let came_to = validator
                .validate(&Credential::Indexer {
                    url: indexer.url.clone(),
                    key: indexer.key.clone(),
                })
                .await;
            let validated = matches!(came_to, Validation::Valid { .. });
            (
                Answer::Credentials(Some(Indexer {
                    validated,
                    ..indexer
                })),
                Some(came_to),
            )
        }
        Answer::Provider(Some(provider)) => {
            let came_to = validator
                .validate(&Credential::Usenet {
                    host: provider.host.clone(),
                    port: provider.port,
                    secure: provider.tls,
                    user: provider.user.clone(),
                    pass: provider.pass.clone(),
                })
                .await;
            let validated = matches!(came_to, Validation::Valid { .. });
            (
                Answer::Provider(Some(Provider {
                    validated,
                    ..provider
                })),
                Some(came_to),
            )
        }
        other => (other, None),
    }
}

/// The problem of recovering an apply that is not part-way through.
fn nothing_to_recover() -> Problem {
    Problem::new(
        NOTHING_TO_RECOVER,
        Severity::Error,
        "No setup here stopped part-way through applying",
        "Recovering chooses what to do about a half-written apply, and there is none to \
         choose about. Nothing has been changed.",
        Remedy::new("Ask where setup stands before offering a way out of it"),
    )
    .in_state(State::Guided)
    .lies_in(Amiss::Asking)
}

pub(crate) use crate::error::codes::setup::NOTHING_TO_RECOVER;

/// The problem of answering setup on a machine that already holds configuration.
fn already_set_up() -> Problem {
    Problem::new(
        ALREADY_SET_UP,
        Severity::Error,
        "This machine is already set up",
        "Setup asks what a fresh install needs to be told, so answering it again would walk a working stack back to its first question. Nothing has been changed.",
        Remedy::new("Change a setting rather than running setup again")
            .with_detail("lemonfiber config set <key> <value>"),
    )
    .in_state(State::Guided)
    .lies_in(Amiss::Asking)
}

pub use crate::error::codes::setup::ALREADY_SET_UP;

#[cfg(test)]
mod tests;
