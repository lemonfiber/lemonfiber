//! The \*arrs the request service hands a request to.
//!
//! The request service does not discover them. Until it is told, a household member
//! asks for something, the ask is accepted, and no downloader ever hears about it.
//!
//! Only the \*arrs actually in the stack are offered, which is the half that decides
//! what the household may ask for at all: the request service offers what its
//! targets can deliver, so television is not offered where Sonarr is not running.
//!
//! Two of the four \*arrs are request targets. The request service fetches film and
//! television and nothing else, so the ones filing music and books are not targets —
//! not an omission, but the same rule applied: an \*arr that cannot fulfil a request
//! is not offered as somewhere to send one.

use std::path::Path;

use lemonfiber_manifest::Service;

use super::arrs::{reached_at, read_servarr_key, servarr_arrs};
use super::Ctx;
use crate::ports::service::{Client as _, Endpoint, FulfilmentTarget, QualityProfile, Requests};
use crate::seed::{State, Wiring};

/// The media type an \*arr must file for the request service to send it anything.
const TELEVISION: &str = "tv";
const FILM: &str = "movies";

/// Every \*arr in this stack the request service should hand requests to.
///
/// Each is read rather than assumed: the profile it fetches at and the folder it
/// files into are asked of the \*arr itself, because the request service must name
/// both when it hands over a request, and an operator may have renamed or replaced
/// what setup created.
///
/// An \*arr that cannot answer, or that has no profile or folder to name, is left
/// out rather than registered half-configured — a target the request service holds
/// but cannot fetch through is worse than one it does not hold, because the request
/// is accepted either way and only the second is visibly missing.
pub(super) async fn wanted_targets(
    ctx: &Ctx,
    services: &[Service],
    project: Option<&Path>,
) -> Vec<FulfilmentTarget> {
    let mut wanted = Vec::new();
    for arr in servarr_arrs(services, project) {
        // Taken together because only the first can actually decline: an \*arr that
        // reached this point came from a service that publishes a port, so there is
        // no separate way for the endpoint to be missing.
        let Some((television, (host, port))) =
            fetches(&arr.media_types).zip(reached_at(services, &arr.target.id))
        else {
            continue;
        };
        let Some(key) = read_servarr_key(ctx, &arr.target.config).await else {
            continue;
        };
        // Built from the key just read rather than opened again. Opening re-reads the
        // same file, so a second failure there could only happen if the first had.
        let client = crate::servarr::Servarr::new(
            ctx.seams.http.clone(),
            &arr.target.base,
            key.clone(),
            &arr.target.id,
            arr.target.version,
        );
        let Some(profile) = first_profile(&client).await else {
            continue;
        };
        let Some(folder) = first_folder(&client).await else {
            continue;
        };
        // Reached at its own address, and moved back there from the gate where the stack
        // no longer runs one; [`seed_fulfilment_targets`] turns it to the gate where it does.
        let moved_from = Some(super::tokens::through_the_gate(&host));
        wanted.push(FulfilmentTarget {
            name: arr.target.name.clone(),
            at: Endpoint {
                host,
                port,
                base: String::new(),
            },
            moved_from,
            key,
            television,
            profile,
            folder,
        });
    }
    wanted
}

/// Whether this \*arr fetches television, film, or neither.
///
/// `None` is not a failure: it is Lidarr or Bindery, which file media the request
/// service does not deal in at all.
pub(super) fn fetches(media_types: &[String]) -> Option<bool> {
    if media_types.iter().any(|kind| kind == TELEVISION) {
        return Some(true);
    }
    if media_types.iter().any(|kind| kind == FILM) {
        return Some(false);
    }
    None
}

/// The profile requests are fetched at.
///
/// The first the \*arr reports, which is what setup wired: a stack with several is
/// one the operator has arranged themselves, and the request service takes one
/// default rather than choosing between them.
async fn first_profile(client: &crate::servarr::Servarr) -> Option<QualityProfile> {
    client.quality_profiles().await.ok()?.into_iter().next()
}

/// Where what it fetches is filed — the first folder it reports, for the same reason.
async fn first_folder(client: &crate::servarr::Servarr) -> Option<String> {
    client
        .root_folders()
        .await
        .ok()?
        .into_iter()
        .map(|folder| folder.path)
        .next()
}

/// Hand the request service every \*arr this stack has that can fulfil a request.
///
/// Nothing to do where the stack has no request service: there is nobody to tell.
pub(super) async fn seed_fulfilment_targets(
    ctx: &Ctx,
    services: &[Service],
    project: Option<&Path>,
) -> Vec<Wiring> {
    // Asked before anything else, because what follows asks every \*arr what it holds
    // and there is no sense doing that with nobody to tell about it.
    let Some(base) = super::identity::seerr_service(services) else {
        return Vec::new();
    };
    let wanted = wanted_targets(ctx, services, project).await;
    if wanted.is_empty() {
        return Vec::new();
    }
    // Signed in, because every call that follows is an authenticated one: registering
    // a target reads what the service already holds and then writes. Unsigned, all of
    // it comes back as a refusal about a credential.
    let seerr = crate::app::targets::seerr_as_owner(ctx, services, base).await;
    match project.filter(|_| crate::app::gating::service(services).is_some()) {
        Some(project) => through_the_gate(ctx, &seerr, wanted, project).await,
        None => wired(ctx, &seerr, &wanted).await,
    }
}

/// Hand the request service `wanted` as they are.
async fn wired(ctx: &Ctx, seerr: &dyn Requests, wanted: &[FulfilmentTarget]) -> Vec<Wiring> {
    let mut journal = crate::journal::Journal::new();
    crate::seed::wire_fulfilment_targets(seerr, wanted, &mut journal, &ctx.stamp(), ctx.dry_run)
        .await
}

/// Hand the request service `wanted` at the request gate, each under a token of its
/// route that the gate accepts.
async fn through_the_gate(
    ctx: &Ctx,
    seerr: &dyn Requests,
    wanted: Vec<FulfilmentTarget>,
    project: &Path,
) -> Vec<Wiring> {
    let kept = super::tokens::Kept::read(ctx, project).await;
    let held = seerr.fulfilment_targets().await.unwrap_or_default();
    let (gated, mut wirings) = kept.targets(ctx, wanted, &held);
    if ctx.dry_run {
        wirings.extend(wired(ctx, seerr, &gated).await);
        return wirings;
    }
    let beside = match kept.beside(&gated) {
        Ok(beside) => beside,
        Err(reason) => {
            let detail = unwritten(&kept, &reason);
            wirings.extend(gated.iter().map(|target| {
                Wiring::settled(
                    crate::seed::described_target(target),
                    State::Failed {
                        detail: detail.clone(),
                    },
                )
            }));
            return wirings;
        }
    };
    let told = wired(ctx, seerr, &gated).await;
    let holding: Vec<bool> = told
        .iter()
        .map(|wiring| matches!(wiring.state, State::Wired | State::AlreadyWired))
        .collect();
    wirings.extend(told);
    // The old tokens stay accepted beside the new where this cannot be written, which
    // keeps every call working and is said rather than called done.
    if let Err(reason) = kept.only(&beside, &gated, &holding) {
        wirings.push(Wiring::settled(
            TOKENS.to_owned(),
            State::Failed {
                detail: unwritten(&kept, &reason),
            },
        ));
    }
    wirings
}

/// What the report calls the gate's tokens, where retiring the old ones fails.
const TOKENS: &str = "The request gate's tokens";

/// Why the tokens could not be handed to the gate.
fn unwritten(kept: &super::tokens::Kept, reason: &str) -> String {
    format!(
        "the tokens could not be written to {}: {reason}",
        kept.path().display()
    )
}

#[cfg(test)]
mod tests;
