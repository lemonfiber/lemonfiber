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

use super::connecting::{pairings, Connection, FILM, TELEVISION};
use super::Ctx;
use crate::ports::filesystem::Beneath;
use crate::ports::service::{Client as _, Endpoint, FulfilmentTarget, QualityProfile, Requests};
use crate::seed::{State, Wiring};
use crate::wiring::{Address, Filler, Fillers};

/// One curator the request service hands requests to: which it is, where the request
/// service reaches it, and whether it fetches television rather than film.
pub(super) struct Fulfils<'a> {
    /// The curator.
    pub(super) filler: &'a Filler,
    /// Where the request service reaches it beside the others.
    pub(super) at: &'a Address,
    /// Whether it fetches television rather than film.
    pub(super) television: bool,
}

/// Every curator the request service asks for and lemonfiber hands it, as the stack's
/// asks settle them — the one answer the request service's targets, the request gate's
/// routes and the credentials taken back from the request service all read.
pub(super) fn fulfilling(fillers: &Fillers) -> Vec<Fulfils<'_>> {
    pairings(fillers)
        .into_iter()
        .filter_map(|pairing| match pairing.made {
            Ok((Connection::Fulfilment { television }, at, _)) => Some(Fulfils {
                filler: pairing.filler,
                at,
                television,
            }),
            _ => None,
        })
        .collect()
}

/// Every \*arr the request service should hand requests to, and the wiring for each
/// whose credential file was refused, said on the target it would have been.
///
/// Each is read rather than assumed: the profile it fetches at and the folder it
/// files into are asked of the \*arr itself, because the request service must name
/// both when it hands over a request, and an operator may have renamed or replaced
/// what setup created.
///
/// An \*arr that cannot answer, that this machine cannot reach, or that has no profile
/// or folder to name, is left out rather than registered half-configured — a target the
/// request service holds but cannot fetch through is worse than one it does not hold,
/// because the request is accepted either way and only the second is visibly missing.
async fn wanted_targets(ctx: &Ctx, fillers: &Fillers) -> (Vec<FulfilmentTarget>, Vec<Wiring>) {
    let mut wanted = Vec::new();
    let mut refused = Vec::new();
    for fulfils in fulfilling(fillers) {
        let filler = fulfils.filler;
        let key = match super::arrs::servarr_key(ctx, filler).await {
            Beneath::Read(key) => key,
            Beneath::Absent => continue,
            Beneath::Escaped => {
                refused.push(super::arrs::refused(
                    crate::seed::as_request_target(&filler.name),
                    filler,
                ));
                continue;
            }
        };
        let (Some(published), Some(version)) = (
            filler.published,
            filler.adapter.as_ref().and_then(|api| api.version),
        ) else {
            continue;
        };
        let client = crate::servarr::Servarr::new(
            ctx.seams.http.clone(),
            crate::app::targets::loopback(published),
            key.clone(),
            &filler.id,
            version,
        );
        let Some(profile) = first_profile(&client).await else {
            continue;
        };
        let Some(folder) = first_folder(&client).await else {
            continue;
        };
        // Reached at its own address, and moved back there from the gate where the stack
        // no longer runs one; [`seed_fulfilment_targets`] turns it to the gate where it does.
        let moved_from = Some(super::tokens::through_the_gate(&fulfils.at.host));
        wanted.push(FulfilmentTarget {
            name: filler.name.clone(),
            at: Endpoint {
                host: fulfils.at.host.clone(),
                port: fulfils.at.port,
                base: String::new(),
            },
            moved_from,
            key,
            television: fulfils.television,
            profile,
            folder,
        });
    }
    (wanted, refused)
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
    fillers: &Fillers,
    project: Option<&Path>,
) -> Vec<Wiring> {
    // Asked before anything else, because what follows asks every \*arr what it holds
    // and there is no sense doing that with nobody to tell about it.
    let Some(base) = super::identity::seerr_service(services) else {
        return Vec::new();
    };
    let (wanted, refused) = wanted_targets(ctx, fillers).await;
    if wanted.is_empty() {
        return refused;
    }
    // Signed in, because every call that follows is an authenticated one: registering
    // a target reads what the service already holds and then writes. Unsigned, all of
    // it comes back as a refusal about a credential.
    let seerr = crate::app::targets::seerr_as_owner(ctx, services, base).await;
    let mut wirings = match project.filter(|_| crate::app::gating::service(services).is_some()) {
        Some(project) => through_the_gate(ctx, &seerr, wanted, project).await,
        None => wired(ctx, &seerr, &wanted).await,
    };
    wirings.extend(refused);
    wirings
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
    let presented: Vec<super::tokens::Presented> =
        gated.iter().map(super::tokens::Presented::by).collect();
    let beside = match kept.beside(&presented) {
        Ok(beside) => beside,
        Err(reason) => {
            let detail = kept.unwritten(&reason);
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
    if let Err(reason) = kept.only(&beside, &presented, &holding) {
        wirings.push(Wiring::settled(
            TOKENS.to_owned(),
            State::Failed {
                detail: kept.unwritten(&reason),
            },
        ));
    }
    wirings
}

/// What the report calls the gate's tokens, where retiring the old ones fails.
const TOKENS: &str = "The request gate's tokens";
