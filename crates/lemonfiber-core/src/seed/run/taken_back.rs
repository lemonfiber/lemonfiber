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

use super::arrs::{reached_at, servarr_arrs};
use super::fulfilment::fetches;
use super::tokens::{through_the_gate, Kept};
use super::Ctx;
use crate::baseline::Baseline;
use crate::credential::{Reach, Settled};
use crate::jellyfin::{Jellyfin, SEERR_APP};
use crate::ports::service::{RegisteredTarget, Requests};
use crate::seed::{State, Wiring};

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
    for arr in servarr_arrs(services, project) {
        let Some((_, (host, port))) =
            fetches(&arr.media_types).zip(reached_at(services, &arr.target.id))
        else {
            continue;
        };
        if held
            .iter()
            .any(|one| one.at.host == host && one.at.port == port)
        {
            baseline.record(
                SEERR,
                &format!("{HELD_KEY}{}", arr.target.id),
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
    project: Option<&Path>,
    baseline: &mut Baseline,
) -> Option<Wiring> {
    let project = gated(services, project)?;
    let base = super::identity::seerr_service(services)?;
    let owed = baseline.named(SEERR, HELD_KEY);
    let jellyfin = jellyfin_admin(ctx, services);
    let minted = match &jellyfin {
        Some((client, _)) => client.filed_as(SEERR_APP).await.unwrap_or_default(),
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
    let route = jellyfin.as_ref().map(|(_, route)| route.as_str());
    let direct = match still_direct(ctx, &seerr, services, project, route).await {
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
        unsettled.extend(replaced(ctx, services, project, &arr, baseline).await);
    }
    if let Some((client, _)) = &jellyfin {
        for key in &minted {
            if let Err(failure) = client.revoke(key).await {
                unsettled.push(format!(
                    "Seerr's own Jellyfin key could not be revoked: {failure}. It still opens \
                     Jellyfin; the next run revokes it."
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
    project: &Path,
    arr: &str,
    baseline: &mut Baseline,
) -> Vec<String> {
    let field = format!("{HELD_KEY}{arr}");
    // An \*arr the stack no longer runs holds no key anybody can use.
    let Some(target) = services
        .iter()
        .find(|service| service.id == arr)
        .and_then(|service| crate::app::targets::target_for(service, project))
    else {
        baseline.forget(SEERR, &field);
        return Vec::new();
    };
    let rotation = crate::app::credentials::reset_arr(ctx, services, Some(project), target).await;
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
/// fulfilling \*arr whose target is not at the gate or does not pass Seerr's own test.
async fn still_direct(
    ctx: &Ctx,
    seerr: &dyn Requests,
    services: &[Service],
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
    for arr in servarr_arrs(services, Some(project)) {
        let Some((television, (host, _))) =
            fetches(&arr.media_types).zip(reached_at(services, &arr.target.id))
        else {
            continue;
        };
        if !gated_target(seerr, &held, &kept, &host, television).await {
            direct.push(arr.target.name.clone());
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
    television: bool,
) -> bool {
    let at = through_the_gate(host);
    let Some(one) = held
        .iter()
        .find(|one| one.at == at && one.television == television)
    else {
        return false;
    };
    kept.accepts(host, &one.key)
        && seerr
            .test_fulfilment_target(television, &one.at, &one.key)
            .await
            .is_ok()
}

/// The project directory, where the stack runs the gate.
fn gated<'a>(services: &[Service], project: Option<&'a Path>) -> Option<&'a Path> {
    project.filter(|_| crate::app::gating::service(services).is_some())
}

/// The media server as its administrator, with the route the gate reaches it on, where
/// lemonfiber holds the administrator's password.
fn jellyfin_admin(ctx: &Ctx, services: &[Service]) -> Option<(Jellyfin, String)> {
    let jellyfin = super::identity::jellyfin_service(services)?;
    let password = super::identity::recorded_jellyfin_password(ctx)?;
    let client = Jellyfin::authenticated(
        ctx.seams.http.clone(),
        &jellyfin.loopback,
        "jellyfin",
        crate::config::JELLYFIN_ADMIN_USER,
        password,
    );
    Some((client, jellyfin.id))
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
