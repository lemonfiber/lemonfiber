//! Adding one thing, end to end, with the operator watching.
//!
//! Setup leaves a machine where sixteen services are green and nothing has been proved.
//! This proves it: pick something, search for it, grab it, download it, import it, and see
//! it in the library — narrating each step so that afterwards the operator understands
//! what their stack does, because they watched it do it once.
//!
//! Every link it touches is a link that can be broken, and a broken one shows here, at the
//! one moment the operator is engaged and willing to fix things, rather than as a
//! mysterious absence three days later. That is why the failure paths carry as much care
//! as the happy one: failure is the useful case.
//!
//! The running is split by concern — whether to offer at all, what to walk, asking for it,
//! waiting on it, and what to say at the end — because each of those is a different set of
//! services and a different set of ways to go wrong.

#[cfg(test)]
mod fixtures;

mod acquire;
mod choose;
mod library;
mod offer;
mod settle;
mod walk;
mod watch;

use crate::app::targets::open_servarrs;
use crate::app::Ctx;
use crate::error::{Diagnose, Problem};
use crate::model::WalkthroughReport;
use crate::walkthrough::{Narrator, Shape, Why};

pub(crate) use walk::Walk;

/// Walk one thing through the whole pipeline, saying what happens as it happens.
///
/// `term` is what the operator asked for, or nothing — in which case something safe is
/// suggested, because a first attempt that fails on an obscure choice teaches the wrong
/// lesson entirely.
///
/// # Errors
///
/// Returns a [`Problem`] where the stack itself cannot be read. Everything else — no
/// indexers, nothing found, a tunnel that is down, an import that would not run — is a
/// walkthrough that stopped, which is a report rather than an error: the operator needs
/// the narration up to the stop as much as they need the stop.
pub async fn walkthrough(
    ctx: &Ctx,
    term: Option<&str>,
    narrator: &dyn Narrator,
) -> Result<WalkthroughReport, Box<Problem>> {
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| Box::new(err.problem()))?;
    let arrs = open_servarrs(ctx, &manifest.services).await;
    let mut walk = Walk::new(ctx, narrator);

    match offer::offered(ctx, &arrs).await {
        // Asked for by a stack that cannot search: not a walk that failed, but the one
        // thing missing before there could be one, said with what to do about it.
        Why::Not(reason) => Ok(walk.stopped(Shape::Pipeline, None, reason)),
        Why::Offer(Shape::LibraryOnly) => {
            Ok(library::walk(&mut walk, &manifest.services, term).await)
        }
        Why::Offer(Shape::Pipeline) => pipeline(&mut walk, &arrs, &manifest.services, term).await,
    }
}

/// The full walk: choose, ask for it, wait on it, and see it land.
async fn pipeline(
    walk: &mut Walk<'_>,
    arrs: &[crate::app::targets::OpenArr],
    services: &[lemonfiber_manifest::Service],
    term: Option<&str>,
) -> Result<WalkthroughReport, Box<Problem>> {
    let chosen = match choose::choose(walk, arrs, term).await {
        Ok(chosen) => chosen,
        Err(choose::NotChosen::Stopped(reason)) => {
            return Ok(walk.stopped(Shape::Pipeline, None, reason))
        }
        Err(choose::NotChosen::AlreadyHere(title)) => return Ok(walk.already_here(&title, arrs)),
    };

    let item = match acquire::acquire(walk, &chosen).await {
        Ok(item) => item,
        Err(reason) => return Ok(walk.stopped(Shape::Pipeline, Some(chosen.named), reason)),
    };

    match watch::watch(walk, &chosen, &item).await {
        watch::Landed::Imported => Ok(settle::settle(walk, services, &chosen).await),
        // Left running rather than abandoned: the operator gets their terminal back and
        // the download keeps going, which is the promise the narration just made them.
        watch::Landed::StillGoing => Ok(walk.handed_off(&chosen.named)),
        watch::Landed::Stopped(reason) => {
            let logs = watch::what_was_said(walk.ctx, services, &chosen.named).await;
            Ok(walk.stopped_quoting(Shape::Pipeline, Some(chosen.named.clone()), reason, logs))
        }
    }
}

/// Whether a walkthrough is worth offering this stack — asked by setup, which offers it,
/// as well as by the walk itself.
///
/// Setup needs the answer before it offers rather than after, because offering a walk that
/// must stop at its first step is worse than not offering one.
///
/// # Errors
///
/// Returns a [`Problem`] where the stack itself cannot be read.
pub async fn worth_offering(ctx: &Ctx) -> Result<Why, Box<Problem>> {
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| Box::new(err.problem()))?;
    let arrs = open_servarrs(ctx, &manifest.services).await;
    Ok(offer::offered(ctx, &arrs).await)
}

#[cfg(test)]
mod tests;
