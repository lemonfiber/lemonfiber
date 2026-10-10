//! Taking back the credentials the request service held before the request gate.
//!
//! A stack whose bundled request service ran without the gate handed it each fulfilling
//! curator's own key, and the request service minted itself a media server key on the
//! sign-in that set it up. Moving it to the gate stops it using either, but its settings
//! file and every backup taken since still hold them. So once it reaches everything
//! through the gate, each curator key it held is replaced through the same reset a
//! rotation makes, and its own media server key is revoked.
//!
//! **What is owed is remembered across runs.** Which curator keys it held can only be
//! seen before its targets move, and the move and the reset may land in different runs,
//! so each one owed is kept in the baseline, under the request service's id, until its
//! reset lands. Its own media server key needs no memory: it is filed under its name
//! until it is revoked.

use std::path::Path;
use std::sync::Arc;

use lemonfiber_contract::capabilities::media::serve;
use lemonfiber_manifest::Service;

use super::fulfilment::fulfilling;
use super::tokens::{through_the_gate, Kept};
use super::Ctx;
use crate::baseline::Baseline;
use crate::credential::{Reach, Settled};
use crate::jellyfin::SEERR_APP;
use crate::ports::media::Kind;
use crate::ports::service::{RegisteredTarget, Requests};
use crate::seed::{State, Wiring};
use crate::wiring::{Filler, Fillers};

/// The stack's own request service among `fillers`, the one that ran before the gate,
/// which a plugin's never did, and where the host reaches it.
fn bundled(fillers: &Fillers) -> Option<(&Filler, String)> {
    let filler = crate::app::targets::request_service(fillers)?;
    Some((filler, crate::app::targets::bundled_requests(filler)?))
}

/// What the report calls this connection.
const CONNECTION: &str = "The credentials the request service held";

/// The field prefix of an owed curator key, followed by the curator's id.
const HELD_KEY: &str = "held-key:";

/// What an owed field holds. Its presence is the record; the value says what it is.
const OWED: &str = "owed";

/// Note every fulfilling curator whose own key the request service holds now, before its
/// target moves to the gate. Nothing where the stack runs no gate or no request service,
/// or where the request service will not say what it holds.
pub(super) async fn note_held(
    ctx: &Ctx,
    services: &[Service],
    fillers: &Fillers,
    project: Option<&Path>,
    baseline: &mut Baseline,
) {
    let Some((filler, base)) = gated(services, project).and(bundled(fillers)) else {
        return;
    };
    let requests = crate::app::targets::owned_requests(ctx, filler, base).await;
    let Ok(held) = requests.fulfilment_targets().await else {
        return;
    };
    for fulfils in fulfilling(fillers) {
        if held
            .iter()
            .any(|one| one.at.host == fulfils.at.host && one.at.port == fulfils.at.port)
        {
            baseline.record(
                &filler.id,
                &format!("{HELD_KEY}{}", fulfils.filler.id),
                OWED,
                &ctx.stamp(),
            );
        }
    }
}

/// Replace every curator key the request service held and revoke its own media server key,
/// once it reaches everything through the gate. Nothing where nothing is owed.
pub(super) async fn seed_taken_back(
    ctx: &Ctx,
    services: &[Service],
    fillers: &Fillers,
    project: Option<&Path>,
    baseline: &mut Baseline,
) -> Option<Wiring> {
    let project = gated(services, project)?;
    let (filler, base) = bundled(fillers)?;
    let owed = baseline.named(&filler.id, HELD_KEY);
    let media_server = media_server_admin(ctx, fillers).await;
    let minted = match &media_server {
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
    let requests = crate::app::targets::owned_requests(ctx, filler, base).await;
    let server = media_server
        .as_ref()
        .map(|(_, route, name)| (route.as_str(), name.as_str()));
    let direct = match still_direct(ctx, &requests, fillers, project, server).await {
        Ok(direct) => direct,
        Err(failure) => return Some(settled(crate::seed::unreached(&failure))),
    };
    if !direct.is_empty() {
        return Some(settled(State::Skipped {
            reason: format!(
                "{name} still reaches {} without the gate, so nothing it held is replaced yet. \
                 A later run replaces them once {name} reaches everything through the gate.",
                direct.join(", "),
                name = filler.name,
            ),
        }));
    }
    let mut unsettled = Vec::new();
    for curator in owed {
        unsettled.extend(
            replaced(
                ctx, services, fillers, project, &filler.id, &curator, baseline,
            )
            .await,
        );
    }
    if let Some((client, _, server)) = &media_server {
        for key in &minted {
            if let Err(failure) = client.revoke(key).await {
                unsettled.push(format!(
                    "{}'s own {server} key could not be revoked: {failure}. It still opens \
                     {server}; the next run revokes it.",
                    filler.name
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

/// Replace the key of the curator `curator`, owed by the request service `owed_by`,
/// forgetting it as owed once the reset lands, and say whatever did not land.
async fn replaced(
    ctx: &Ctx,
    services: &[Service],
    fillers: &Fillers,
    project: &Path,
    owed_by: &str,
    curator: &str,
    baseline: &mut Baseline,
) -> Vec<String> {
    let field = format!("{HELD_KEY}{curator}");
    let Some(target) = fillers.service(curator).and_then(Filler::target) else {
        baseline.forget(owed_by, &field);
        return Vec::new();
    };
    let rotation =
        crate::app::credentials::reset_curator(ctx, services, fillers, Some(project), target).await;
    if matches!(rotation.settled, Settled::Replaced { .. }) {
        baseline.forget(owed_by, &field);
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

/// What the request service still reaches without the gate, by name: the media server,
/// reached at the gate on the route and called the name in `server`, where its link is
/// not at that route under a token the gate accepts, and each
/// fulfilling curator whose target is not at the gate or does not pass the request
/// service's own test.
async fn still_direct(
    ctx: &Ctx,
    requests: &dyn Requests,
    fillers: &Fillers,
    project: &Path,
    server: Option<(&str, &str)>,
) -> Result<Vec<String>, crate::ports::service::Failure> {
    let kept = Kept::read(ctx, project).await;
    let mut direct = Vec::new();
    if let Some((route, name)) = server {
        let link = requests.media_server_link().await?;
        if link.at != through_the_gate(route) || !kept.accepts(route, &link.key) {
            direct.push(name.to_owned());
        }
    }
    let held = requests.fulfilment_targets().await?;
    for fulfils in fulfilling(fillers) {
        let host = &fulfils.at.host;
        if !gated_target(requests, &held, &kept, host, fulfils.kind).await {
            direct.push(fulfils.filler.name.clone());
        }
    }
    Ok(direct)
}

/// Whether the request service holds the curator reached at `host` at the gate, under a
/// token the gate accepts, and passes its own test of it there.
async fn gated_target(
    requests: &dyn Requests,
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
        && requests
            .test_fulfilment_target(kind, &one.at, &one.key)
            .await
            .is_ok()
}

/// The project directory, where the stack runs the gate.
fn gated<'a>(services: &[Service], project: Option<&'a Path>) -> Option<&'a Path> {
    project.filter(|_| crate::app::gating::service(services).is_some())
}

/// The media server as its administrator, with the route the gate reaches it on and what
/// it is called, where it can be asked as one.
async fn media_server_admin(
    ctx: &Ctx,
    fillers: &Fillers,
) -> Option<(Arc<dyn serve::Fills>, String, String)> {
    let server = crate::app::targets::MediaServer::of(fillers)?;
    let client = server.administering(ctx).await?;
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
