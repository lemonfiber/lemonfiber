//! A member looking for a title the house does not hold yet, and asking for it.
//!
//! **Every title comes through the core, under the member's own limits.** Whatever
//! fills `request.intake` is asked, over the contract where it speaks it and through
//! this build's adapter otherwise, and nothing else is. A title outside the member's
//! limits is left out of a search, never shown as unavailable, and an ask naming one is
//! answered as no such title, so asking reveals nothing a search hides.
//!
//! **Only kinds the house can fetch now are offered.** The core decides from the curators
//! running, and says which in its answer; the member chooses no kind.
//!
//! **Every refusal is said to the household.** Each carries a sentence and a remedy written
//! for whoever is asking, naming no service, size, address or state of the stack; a fault
//! of the stack reads as asking not working right now, with whoever runs the house told.

mod ask;
mod find;
mod limits;
mod place;

use std::collections::BTreeSet;
use std::sync::Arc;

use lemonfiber_contract::capabilities::request::intake;

use crate::app::targets::{fillers_here, identity, request_service, requests_as_owner, IDENTITY};
use crate::app::{Ctx, Outcome, Wishing};
use crate::error::codes::wish::{
    ALREADY_HERE, CANNOT_FETCH, NOBODY_ASKING, NOTHING_LEFT, NOT_ASKABLE, NOT_A_MEMBER,
    NOT_A_PLACE, NOT_A_SEARCH, NOT_ITS_SEASON, NOT_SET_UP, NOT_WORKING, NO_ROOM,
};
use crate::error::{Code, Problem, Remedy, State};
use crate::ports::docker::Lifecycle;
use crate::ports::media::Kind;
use crate::ports::service::Member;
use crate::wiring::Fillers;

use limits::Limits;

/// Every refusal a member's search or ask answers with, which a client names by its code.
pub const REFUSALS: [Code; 12] = [
    NOT_A_SEARCH,
    NOT_A_PLACE,
    NOT_ASKABLE,
    CANNOT_FETCH,
    ALREADY_HERE,
    NOTHING_LEFT,
    NO_ROOM,
    NOT_WORKING,
    NOT_SET_UP,
    NOT_ITS_SEASON,
    NOBODY_ASKING,
    NOT_A_MEMBER,
];

/// What a member's search or ask comes to.
///
/// # Errors
///
/// The [`Problem`] the request is refused with, in the household's words.
pub(crate) async fn wished(ctx: &Ctx, wishing: Wishing) -> Result<Outcome, Box<Problem>> {
    match wishing {
        Wishing::Find {
            member,
            term,
            after,
        } => find::found(ctx, &member, &term, after.as_deref())
            .await
            .map(Outcome::Askable),
        Wishing::Ask { member, wish } => ask::asked(ctx, &member, wish).await.map(Outcome::Asked),
    }
}

/// Everything one member's search or ask is answered from.
struct Reached {
    /// Whatever fills `request.intake`, asked as its owner.
    requests: Arc<dyn intake::Fills>,
    /// The member, as the media server holds their account.
    member: Member,
    /// What the member may watch, and so may be offered.
    limits: Limits,
    /// The kinds the house can fetch now.
    kinds: Vec<Kind>,
}

/// The member `named`, their limits, the kinds the house can fetch and the service that
/// answers for titles beyond the house.
///
/// # Errors
///
/// A [`Problem`] where nobody is named, where the house has no request service or media
/// server, where either cannot be asked, where nobody in the household goes by `named`,
/// and where the member is held to a limit that cannot be read.
async fn reached(ctx: &Ctx, named: &str) -> Result<Reached, Box<Problem>> {
    if named.trim().is_empty() {
        return Err(refused(NOBODY_ASKING, "Nobody was named."));
    }
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|_| not_working())?;
    let fillers = fillers_here(ctx, &manifest);
    let (Some(filler), Some(_)) = (request_service(&fillers), fillers.filling(IDENTITY)) else {
        return Err(refused(
            NOT_SET_UP,
            "This house isn't set up for asking for new things.",
        ));
    };
    let requests = requests_as_owner(ctx, filler)
        .await
        .ok_or_else(not_working)?;
    let server = identity(ctx, &manifest).await.ok_or_else(not_working)?;
    let accounts = server.household().await.map_err(|_| not_working())?;
    let member = crate::app::held::account(&accounts, named)
        .cloned()
        .ok_or_else(|| refused(NOT_A_MEMBER, "Nobody in this household is known by that."))?;
    let limits = Limits::of(server.as_ref(), &member.access)
        .await
        .ok_or_else(not_working)?;
    Ok(Reached {
        requests,
        member,
        limits,
        kinds: deliverable(ctx, &fillers).await,
    })
}

/// The kinds of title a curator that is running now files, television first.
///
/// Nothing where the engine cannot say what is running: a kind nobody can be shown to
/// fetch is not offered.
async fn deliverable(ctx: &Ctx, fillers: &Fillers) -> Vec<Kind> {
    let Ok(containers) = ctx.seams.engine.list(&ctx.settings.project).await else {
        return Vec::new();
    };
    let running: BTreeSet<&str> = containers
        .iter()
        .filter(|container| container.lifecycle == Lifecycle::Running)
        .map(|container| container.service.as_str())
        .collect();
    let curators = crate::seed::run::curators(fillers);
    Kind::ALL
        .into_iter()
        .filter(|kind| {
            curators.iter().any(|curator| {
                running.contains(curator.id())
                    && curator
                        .media_types()
                        .iter()
                        .any(|media| media == kind.media_type())
            })
        })
        .collect()
}

/// A refusal under `code`, said to the household, with the registry's remedy.
fn refused(code: Code, summary: &str) -> Box<Problem> {
    let remedy = code.declaration().map_or("", |declared| declared.remedy());
    refused_with(code, summary, remedy)
}

/// A refusal under `code`, said to the household, with `remedy`.
fn refused_with(code: Code, summary: &str, remedy: &str) -> Box<Problem> {
    let meaning = code.declaration().map_or("", |declared| declared.meaning());
    Box::new(Problem::new(code, summary, meaning, Remedy::new(remedy)).in_state(State::Guided))
}

/// The refusal for whatever answers the household's asks not answering.
fn not_working() -> Box<Problem> {
    refused(NOT_WORKING, "Asking isn't working right now.")
}

/// `text` begun with a capital letter.
fn capitalised(text: &str) -> String {
    let mut letters = text.chars();
    letters.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(letters).collect()
    })
}

#[cfg(test)]
mod tests;
