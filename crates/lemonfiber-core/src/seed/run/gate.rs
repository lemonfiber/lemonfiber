//! The request gate's routes: each upstream it answers for, and what it presents there.
//!
//! The gate holds the credentials the request service would otherwise hold: each
//! fulfilling curator's own key, read from where the curator wrote it, and a key minted for
//! the gate alone in the media server, whichever service fills the identity source, and
//! filed under its name. They are handed over in one
//! owner-only file in the gate's configuration directory, never its environment, and
//! written again whenever what they should hold has moved — a curator that regenerated
//! its key reaches the gate on the next pass.
//!
//! **The file is what holds the media server's key.** A key filed under the gate's name that
//! the file does not hold is one nothing holds, and is revoked, as the decline
//! service's are.

use std::path::Path;

use lemonfiber_sidecar::gate::{Credential, File, Kind, Upstream, Upstreams};

use super::minted;
use super::Ctx;
use crate::app::gating;
use crate::app::targets::MediaServer;
use crate::app_keys::GATE_APP;
use crate::ports::service::AppKeys;
use crate::seed::{State, Wiring};

/// What the report calls this connection.
const CONNECTION: &str = "The request gate's routes";

/// Hold the gate to a route for each fulfilling curator and, where it speaks the media
/// server's API, one for the media server under a key of its own — or, where the stack no
/// longer runs the gate, to no key at all.
pub(super) async fn seed_gate_routes(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
    fillers: &crate::wiring::Fillers,
    server: Option<&MediaServer>,
    project: Option<&Path>,
) -> Option<Wiring> {
    let routed = server.and_then(|server| server.gate_kind().map(|kind| (server, kind)));
    match routed {
        Some((server, kind)) => {
            with_the_media_server(ctx, services, fillers, server, kind, project).await
        }
        None => curators_alone(ctx, services, fillers, project).await,
    }
}

/// The gate's routes where it answers for no media server: one for each fulfilling
/// curator, and no key minted for it anywhere.
async fn curators_alone(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
    fillers: &crate::wiring::Fillers,
    project: Option<&Path>,
) -> Option<Wiring> {
    gating::service(services)?;
    let Some(project) = project else {
        return Some(settled(no_project()));
    };
    let (path, current) = routes_file(ctx, project).await;
    let wanted = Upstreams::of(curator_routes(ctx, fillers).await);
    let state = if current.as_ref() == Some(&wanted) {
        State::AlreadyWired
    } else if ctx.dry_run {
        State::WouldWire {
            yours: Some(listed(current.as_ref())),
            ours: Some(listed(Some(&wanted))),
        }
    } else {
        match crate::config::store::write(&path, &wanted.written()) {
            Ok(()) => State::Wired,
            Err(failure) => State::Failed {
                detail: format!(
                    "the routes could not be written to {}: {failure}",
                    path.display()
                ),
            },
        }
    };
    Some(settled(state))
}

/// The gate's routes with one for the media server, a route of `kind`, under a key minted
/// for the gate alone and filed under its name.
async fn with_the_media_server(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
    fillers: &crate::wiring::Fillers,
    server: &MediaServer,
    kind: Kind,
    project: Option<&Path>,
) -> Option<Wiring> {
    let gating = gating::service(services).is_some();
    // Minted with the administrator's session, which lemonfiber holds only on a server
    // it set up; a rehearsal before the first run finds none recorded yet.
    let Some(client) = server.administering(ctx).await else {
        let minting = server.would_mint(ctx);
        return (gating && minting).then(|| {
            settled(State::WouldWire {
                yours: None,
                ours: None,
            })
        });
    };
    let client = client.as_ref();
    let filed = client.filed_as(GATE_APP).await;
    // A stack that does not run the gate asked for nothing here: a key list that could
    // not be read is left for the next run to retire from, not reported.
    if !gating {
        return minted::retired(ctx, client, GATE_APP, CONNECTION, &filed.ok()?).await;
    }
    let filed = match filed {
        Ok(filed) => filed,
        Err(failure) => return Some(settled(crate::seed::unreached(&failure))),
    };
    let Some(project) = project else {
        return Some(settled(no_project()));
    };
    let (path, current) = routes_file(ctx, project).await;
    let routes = curator_routes(ctx, fillers).await;
    let held = current
        .as_ref()
        .and_then(|upstreams| upstreams.route(server.id()))
        .map(|route| route.credential.reveal().to_owned())
        .filter(|key| filed.contains(key));
    let state = match held {
        Some(key) => {
            let wanted = Upstreams::of(with(&routes, to_media(server, kind, &key)));
            let others: Vec<&String> = filed.iter().filter(|one| **one != key).collect();
            kept(ctx, client, current.as_ref(), &wanted, others, &path).await
        }
        None if ctx.dry_run => State::WouldWire {
            yours: Some(listed(current.as_ref())),
            ours: Some(named(&with(&routes, to_media(server, kind, "")))),
        },
        None => {
            let key = match minted::mint(client, GATE_APP).await {
                Ok(key) => key,
                Err(state) => return Some(settled(state)),
            };
            let wanted = Upstreams::of(with(&routes, to_media(server, kind, &key)));
            if let Err(failure) = crate::config::store::write(&path, &wanted.written()) {
                let _ = client.revoke(&key).await;
                return Some(settled(State::Failed {
                    detail: format!(
                        "the routes could not be written to {}, so the new key was revoked \
                         again: {failure}",
                        path.display()
                    ),
                }));
            }
            minted::revoking(client, &filed).await
        }
    };
    Some(settled(state))
}

/// Where the gate's routes file sits under `project`, and the routes it holds now.
async fn routes_file(ctx: &Ctx, project: &Path) -> (std::path::PathBuf, Option<Upstreams>) {
    let path = gating::path(project, File::Upstreams);
    let current = crate::app::targets::read_owned(
        ctx.seams.filesystem.as_ref(),
        &path,
        crate::within::directory_of(&path),
    )
    .await
    .and_then(|text| Upstreams::read(&text).ok());
    (path, current)
}

/// What is said where there is no stack directory to hand the gate its routes in.
fn no_project() -> State {
    State::Skipped {
        reason: "there is no stack directory to hand the request gate its routes in".to_owned(),
    }
}

/// Write the gate's routes again after the curator `curator_id` replaced its key, so its
/// route presents the new one: what replacing that key owes the gate.
///
/// Nothing where the stack runs no gate or the request service does not fulfil
/// through `curator_id`.
pub(crate) async fn reroute(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
    fillers: &crate::wiring::Fillers,
    project: Option<&Path>,
    curator_id: &str,
) -> Option<State> {
    gating::service(services)?;
    let routed = super::fulfilling(fillers)
        .iter()
        .any(|fulfils| fulfils.filler.id == curator_id);
    routed.then_some(())?;
    // Boxed, because it is carried across the routes being written.
    let server = crate::app::targets::MediaServer::of(fillers).map(Box::new);
    seed_gate_routes(ctx, services, fillers, server.as_deref(), project)
        .await
        .map(|wiring| wiring.state)
}

/// The gate already holds a key the media server lists: bring its routes up to what they
/// should be, and revoke whatever else is filed under its name.
async fn kept(
    ctx: &Ctx,
    client: &dyn AppKeys,
    current: Option<&Upstreams>,
    wanted: &Upstreams,
    others: Vec<&String>,
    path: &Path,
) -> State {
    let moved = current != Some(wanted);
    if !moved && others.is_empty() {
        return State::AlreadyWired;
    }
    // Said by what changes: the routes where they move, and otherwise the keys.
    if ctx.dry_run && moved {
        return State::WouldWire {
            yours: Some(listed(current)),
            ours: Some(listed(Some(wanted))),
        };
    }
    if ctx.dry_run {
        return State::WouldWire {
            yours: Some(minted::count(GATE_APP, others.len() + 1)),
            ours: Some(minted::count(GATE_APP, 1)),
        };
    }
    if moved {
        if let Err(failure) = crate::config::store::write(path, &wanted.written()) {
            return State::Failed {
                detail: format!(
                    "the routes could not be written to {}: {failure}",
                    path.display()
                ),
            };
        }
    }
    minted::revoking(client, others).await
}

/// A route for each curator the request service fulfils through, with the key it wrote for
/// itself. A curator that has written none yet has no route until it has.
async fn curator_routes(ctx: &Ctx, fillers: &crate::wiring::Fillers) -> Vec<Upstream> {
    let mut routes = Vec::new();
    for fulfils in super::fulfilling(fillers) {
        let crate::ports::filesystem::Beneath::Read(key) =
            super::keys::curator_key(ctx, fulfils.filler).await
        else {
            continue;
        };
        routes.push(Upstream {
            route: fulfils.filler.id.clone(),
            kind: gating::route_for(fulfils.kind),
            address: fulfils.at.url(),
            credential: Credential::new(key),
            majors: Vec::new(),
        });
    }
    routes
}

/// The route to the media server, a route of `kind` at its address on the stack's
/// network, forwarding to the lines its image runs and presenting `key`.
fn to_media(server: &MediaServer, kind: Kind, key: &str) -> Upstream {
    Upstream {
        route: server.id().to_owned(),
        kind,
        address: server.network.url(),
        credential: Credential::new(key),
        majors: server.filler.majors.clone(),
    }
}

/// `routes`, and the media server's route after them.
fn with(routes: &[Upstream], media: Upstream) -> Vec<Upstream> {
    let mut all = routes.to_vec();
    all.push(media);
    all
}

/// What a set of routes answers for, said by their names and never their credentials.
fn named(routes: &[Upstream]) -> String {
    if routes.is_empty() {
        return "no routes".to_owned();
    }
    let names: Vec<&str> = routes.iter().map(|route| route.route.as_str()).collect();
    format!("routes to {}", names.join(", "))
}

/// What the routes file holds now, said the same way.
fn listed(upstreams: Option<&Upstreams>) -> String {
    named(upstreams.map_or(&[], |upstreams| upstreams.upstreams.as_slice()))
}

/// This connection, resting in `state`.
fn settled(state: State) -> Wiring {
    Wiring::settled(CONNECTION.to_owned(), state)
}
