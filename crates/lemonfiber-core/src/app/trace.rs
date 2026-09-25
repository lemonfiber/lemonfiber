//! Following one item across the services, to answer "where is my show?".
//!
//! The question a household actually asks is answerable only by correlating fragments
//! four services each hold. This is the first cut: the \*arr that monitors an item and
//! records its history is the spine, and this reads it into one view — how far the item
//! got, and, where the \*arr's own record plainly shows it stopped, why.
//!
//! What the \*arr cannot see alone — whether an indexer found releases, whether the
//! download client took it, whether the media server can play it — is left to the
//! services that can; this never over-claims the reason for a stall it cannot prove.
//!
//! Two of those stalls are absences, and an absence has a cause no \*arr can see: nothing
//! found, and nothing taken. Both are what a lapsed account looks like from here — an
//! indexer with its allowance spent answers every search with an empty list, and an
//! account that is refusing or empty takes nothing it is handed. So where the trace stops
//! at one of those two, it asks the accounts, and what they say travels beside the stall
//! rather than replacing it: how far the item got is still the answer to the question
//! that was asked.
//!
//! Three questions, one per file: what each service holds, what the fragments amount to,
//! and why it stopped. The entry points stay here, because they are the errand.

mod assembling;
mod explaining;
mod reading;

use assembling::assemble;
use explaining::{not_matched, searched, unexplained};
use reading::{
    account_explainable, asking, beside, library_presence, providers, troubles, Fragments, Reads,
};

use super::targets::{jellyfin_reader, open_servarrs};
use super::Ctx;
use crate::error::{Diagnose, Problem};
use crate::model::{StuckEntry, StuckReport, TraceReport};
use crate::ports::service::Pipeline;

/// Trace one item by a human term, across the resolution services.
///
/// Searches each \*arr's library for a title matching the term; the first match is
/// followed. No match at all is itself the answer — nobody asked for it, the first stage
/// the trace tells apart.
///
/// Every read here is a read until `searching`. That one asks the indexers what they
/// carry, which spends a real search against the daily allowance they hold the operator
/// to — the one thing a trace can do that reaches past this machine — so it is made only
/// where it was asked for, and only where the trace has a silence it could explain.
pub(crate) async fn trace(
    ctx: &Ctx,
    term: &str,
    season: Option<u32>,
    searching: bool,
) -> Result<TraceReport, Box<Problem>> {
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| Box::new(err.problem()))?;
    // The media server is resolved once, ahead of the match: the last stage of a trace is
    // the same read whichever \*arr the item turns up in.
    let jellyfin = jellyfin_reader(ctx, &manifest.services);

    for arr in open_servarrs(ctx, &manifest.services).await {
        let (kind, service) = (arr.kind, arr.service);
        let Ok(items) = service.find_items(kind, term).await else {
            continue;
        };
        let Some(item) = items.into_iter().next() else {
            continue;
        };
        // Each read that can fail is kept as read-or-not, never collapsed to empty: a
        // history or queue that could not be read is not "nothing happened", so the trace
        // must not infer a stall from a silence it never actually heard.
        let (events, history) = match service.item_history(kind, item.id).await {
            Ok(events) => (events, true),
            Err(_) => (Vec::new(), false),
        };
        let (queue, queue_read) = match service.item_queue(kind, item.id).await {
            Ok(queue) => (queue, true),
            Err(_) => (Vec::new(), false),
        };
        // An item made of parts — a series — is aggregated per part, so "the show is
        // imported" cannot stand on one episode having landed. A service whose items have
        // no parts answers with none, and the trace reads as it always did.
        let (parts, parts_read) = match service.item_parts(kind, item.id, season).await {
            Ok(parts) => (parts, true),
            Err(_) => (Vec::new(), false),
        };
        let reads = Reads {
            history,
            queue: queue_read,
            parts: parts_read,
        };
        let library = library_presence(jellyfin.as_ref(), kind, &item.title).await;
        let mut report = assemble(
            &arr.name,
            &item.title,
            item.monitored,
            Fragments {
                events,
                queue,
                parts,
                library,
                reads,
            },
        );
        if searching && unexplained(&report) {
            searched(&mut report, &arr.name, asking(&service, kind).await);
        }
        if let Some(reason) = account_explainable(&report) {
            let said = troubles(providers(ctx, &manifest.services).await);
            report.stall = Some(beside(reason, &said));
        }
        return Ok(report);
    }

    Ok(not_matched(term))
}

/// The items whose downloads are stuck, across the \*arrs — the landing point queue
/// health leads to, so "N items stuck" becomes a named list each entry of which traces on
/// its own. An \*arr that answered but whose queue would not read marks the list
/// incomplete rather than being read as nothing stuck — the same honesty a trace keeps
/// about a silence it did not hear. One that has not finished starting, its key not yet
/// readable, is skipped as it is everywhere else: a service still coming up holds nothing
/// stuck, so its absence understates nothing.
pub(crate) async fn stuck(ctx: &Ctx) -> Result<StuckReport, Box<Problem>> {
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| Box::new(err.problem()))?;
    let mut items = Vec::new();
    let mut incomplete = false;
    for arr in open_servarrs(ctx, &manifest.services).await {
        match arr.service.stuck_items(arr.kind).await {
            Ok(stuck) => items.extend(stuck.into_iter().map(|item| StuckEntry {
                title: item.title,
                service: arr.name.clone(),
                stage: item.stage,
            })),
            Err(_) => incomplete = true,
        }
    }
    // The queues that were never asked, as against the ones that were and would not
    // answer. A service declaring an API this build cannot speak or reach holds a queue
    // nothing here can read, and leaving it out makes this list exactly as short as an
    // unreadable queue does — without the sentence that says so.
    let project = super::targets::project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref());
    let unsupported = super::targets::unsupported_here(&manifest.services, project.as_deref());

    Ok(StuckReport {
        items,
        incomplete,
        unsupported,
    })
}

#[cfg(test)]
mod tests;
