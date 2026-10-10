//! Taking back the credentials the request service held before the request gate.
//!
//! A stack that ran Seerr without the gate handed it each fulfilling \*arr's own key,
//! and Seerr minted itself a Jellyfin key on the sign-in that set it up. Moving Seerr to
//! the gate stops it using either, but its settings file and every backup taken since
//! still hold them. So once Seerr reaches everything through the gate, each \*arr key it
//! held is replaced through the same reset a rotation makes, and Seerr's own Jellyfin
//! key is revoked.
//!
//! **What is owed is remembered across runs.** Which \*arr keys Seerr held can only be
//! seen before its targets move, and the move and the reset may land in different
//! runs, so each one owed is kept in the baseline until its reset lands. Seerr's own
//! Jellyfin key needs no memory: it is filed under Seerr's name until it is revoked.

use std::path::Path;

use lemonfiber_manifest::Service;

use super::fulfilment::fulfilling;
use super::tokens::{through_the_gate, Kept};
use super::Ctx;
use crate::baseline::Baseline;
use crate::credential::{Reach, Settled};
use crate::jellyfin::{Jellyfin, SEERR_APP};
use crate::ports::media::Kind;
use crate::ports::service::{AppKeys as _, RegisteredTarget, Requests};
use crate::seed::{State, Wiring};
use crate::wiring::Fillers;

/// What the report calls this connection.
const CONNECTION: &str = "The credentials the request service held";

/// The service the owed work is recorded under in the baseline.
const SEERR: &str = "seerr";

/// The field prefix of an owed \*arr key, followed by the \*arr's id.
const HELD_KEY: &str = "held-key:";

/// What an owed field holds. Its presence is the record; the value says what it is.
const OWED: &str = "owed";

/// Note every fulfilling \*arr whose own key the request service holds now, before its
/// target moves to the gate. Nothing where the stack runs no gate or no request service,
/// or where the request service will not say what it holds.
pub(super) async fn note_held(
    ctx: &Ctx,
    services: &[Service],
    fillers: &Fillers,
    project: Option<&Path>,
    baseline: &mut Baseline,
) {
    let Some(base) = gated(services, project).and(super::identity::seerr_service(services)) else {
        return;
    };
    let seerr = crate::app::targets::seerr_as_owner(ctx, services, base).await;
    let Ok(held) = seerr.fulfilment_targets().await else {
        return;
    };
    for fulfils in fulfilling(fillers) {
        if held
            .iter()
            .any(|one| one.at.host == fulfils.at.host && one.at.port == fulfils.at.port)
        {
            baseline.record(
                SEERR,
                &format!("{HELD_KEY}{}", fulfils.filler.id),
                OWED,
                &ctx.stamp(),
            );
        }
    }
}

/// Replace every \*arr key the request service held and revoke its own Jellyfin key,
/// once it reaches everything through the gate. Nothing where nothing is owed.
pub(super) async fn seed_taken_back(
    ctx: &Ctx,
    services: &[Service],
    fillers: &Fillers,
    project: Option<&Path>,
    baseline: &mut Baseline,
) -> Option<Wiring> {
    let project = gated(services, project)?;
    let base = super::identity::seerr_service(services)?;
    let owed = baseline.named(SEERR, HELD_KEY);
    let jellyfin = jellyfin_admin(ctx, fillers);
    let minted = match &jellyfin {
        Some((client, ..)) => client.filed_as(SEERR_APP).await.unwrap_or_default(),
        None => Vec::new(),
    };
    let count = owed.len() + minted.len();
    if count == 0 {
        return None;
    }
    if ctx.dry_run {
        return Some(settled(State::WouldWire {
            yours: Some(held_count(count)),
            ours: Some("none".to_owned()),
        }));
    }
    let seerr = crate::app::targets::seerr_as_owner(ctx, services, base).await;
    let route = jellyfin.as_ref().map(|(_, route, _)| route.as_str());
    let direct = match still_direct(ctx, &seerr, fillers, project, route).await {
        Ok(direct) => direct,
        Err(failure) => return Some(settled(crate::seed::unreached(&failure))),
    };
    if !direct.is_empty() {
        return Some(settled(State::Skipped {
            reason: format!(
                "Seerr still reaches {} without the gate, so nothing it held is replaced yet. \
                 A later run replaces them once Seerr reaches everything through the gate.",
                direct.join(", ")
            ),
        }));
    }
    let mut unsettled = Vec::new();
    for arr in owed {
        unsettled.extend(replaced(ctx, services, fillers, project, &arr, baseline).await);
    }
    if let Some((client, _, server)) = &jellyfin {
        for key in &minted {
            if let Err(failure) = client.revoke(key).await {
                unsettled.push(format!(
                    "Seerr's own {server} key could not be revoked: {failure}. It still opens \
                     {server}; the next run revokes it."
                ));
            }
        }
    }
    Some(settled(if unsettled.is_empty() {
        State::Wired
    } else {
        State::Failed {
            detail: crate::config::store::withheld_text(&unsettled.join(" ")),
        }
    }))
}

/// Replace the key of the \*arr `arr`, forgetting it as owed once the reset lands, and
/// say whatever did not land.
async fn replaced(
    ctx: &Ctx,
    services: &[Service],
    fillers: &Fillers,
    project: &Path,
    arr: &str,
    baseline: &mut Baseline,
) -> Vec<String> {
    let field = format!("{HELD_KEY}{arr}");
    // An \*arr that is no longer on this machine holds no key anybody can use.
    let Some(target) = fillers.service(arr).and_then(crate::wiring::Filler::target) else {
        baseline.forget(SEERR, &field);
        return Vec::new();
    };
    let rotation =
        crate::app::credentials::reset_arr(ctx, services, fillers, Some(project), target).await;
    if matches!(rotation.settled, Settled::Replaced { .. }) {
        baseline.forget(SEERR, &field);
        return rotation
            .consumers
            .iter()
            .filter_map(|one| match &one.reach {
                Reach::Failed { detail } => {
                    Some(format!("{} could not be given it: {detail}.", one.consumer))
                }
                Reach::Updated | Reach::Pending { .. } => None,
            })
            .collect();
    }
    let detail = rotation.settled.detail().unwrap_or_default();
    vec![format!(
        "{} could not be replaced: {detail}",
        rotation.credential
    )]
}

/// What the request service still reaches without the gate, by name: the media server
/// where its link is not at the gate's route under a token the gate accepts, and each
/// fulfilling \*arr whose target is not at the gate or does not pass the request
/// service's own test.
async fn still_direct(
    ctx: &Ctx,
    seerr: &dyn Requests,
    fillers: &Fillers,
    project: &Path,
    route: Option<&str>,
) -> Result<Vec<String>, crate::ports::service::Failure> {
    let kept = Kept::read(ctx, project).await;
    let mut direct = Vec::new();
    if let Some(route) = route {
        let link = seerr.media_server_link().await?;
        if link.at != through_the_gate(route) || !kept.accepts(route, &link.key) {
            direct.push("Jellyfin".to_owned());
        }
    }
    let held = seerr.fulfilment_targets().await?;
    for fulfils in fulfilling(fillers) {
        let host = &fulfils.at.host;
        if !gated_target(seerr, &held, &kept, host, fulfils.kind).await {
            direct.push(fulfils.filler.name.clone());
        }
    }
    Ok(direct)
}

/// Whether the request service holds the \*arr reached at `host` at the gate, under a
/// token the gate accepts, and passes its own test of it there.
async fn gated_target(
    seerr: &dyn Requests,
    held: &[RegisteredTarget],
    kept: &Kept,
    host: &str,
    kind: Kind,
) -> bool {
    let at = through_the_gate(host);
    let Some(one) = held.iter().find(|one| one.at == at && one.kind == kind) else {
        return false;
    };
    kept.accepts(host, &one.key)
        && seerr
            .test_fulfilment_target(kind, &one.at, &one.key)
            .await
            .is_ok()
}

/// The project directory, where the stack runs the gate.
fn gated<'a>(services: &[Service], project: Option<&'a Path>) -> Option<&'a Path> {
    project.filter(|_| crate::app::gating::service(services).is_some())
}

/// The media server as its administrator, with the route the gate reaches it on and what
/// it is called, where lemonfiber holds the administrator's password.
fn jellyfin_admin(ctx: &Ctx, fillers: &Fillers) -> Option<(Jellyfin, String, String)> {
    let server = crate::app::targets::MediaServer::of(fillers)?;
    let client = server.administered(ctx)?;
    Some((client, server.id().to_owned(), server.name().to_owned()))
}

/// How many credentials the request service holds, as a rehearsal says it.
fn held_count(count: usize) -> String {
    if count == 1 {
        "1 credential held".to_owned()
    } else {
        format!("{count} credentials held")
    }
}

/// This connection, resting in `state`.
fn settled(state: State) -> Wiring {
    Wiring::settled(CONNECTION.to_owned(), state)
}
