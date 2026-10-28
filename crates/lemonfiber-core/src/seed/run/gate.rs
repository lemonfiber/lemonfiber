//! The request gate's routes: each upstream it answers for, and what it presents there.
//!
//! The gate holds the credentials the request service would otherwise hold: each
//! fulfilling curator's key, the one lemonfiber minted for it, and a key minted for the
//! gate alone in the media server, whichever service fills the identity source, and filed
//! under its name. They are handed over through `request.gate`, which the stack's own gate
//! is told in owner-only files in its configuration directory, never its environment, and
//! again whenever what they should hold has moved — a curator key minted anew reaches the
//! gate on the next pass.
//!
//! **A curator the request service fulfils through that the gate holds no route to is
//! said**, on a line of its own beside the gate's: one whose credential the gate may not
//! hold, and one lemonfiber holds no minted key for.
//!
//! **The gate is what holds the media server's key.** A key filed under the gate's name
//! whose SHA-256 the gate does not answer is one nothing holds, and is revoked, as the
//! decline service's are.

use std::path::Path;
use std::sync::Arc;

use super::minted;
use super::Ctx;
use crate::app::gating;
use crate::app::targets::{MediaServer, Pairing};
use crate::app_keys::GATE_APP;
use crate::ports::service::{Admitted, AppKeys, Gated, Gating, Target};
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
) -> Vec<Wiring> {
    let routed = server.and_then(|server| server.gated().map(|gated| (server, gated)));
    match routed {
        Some((server, (_, pairing))) if gating::may_hold(fillers, server.filler.holder()) => {
            with_the_media_server(ctx, services, fillers, (server, pairing), project).await
        }
        Some((server, _)) => {
            let mut wirings = curators_alone(ctx, services, fillers, project).await;
            wirings.extend(withheld(ctx, server).await);
            wirings
        }
        None => curators_alone(ctx, services, fillers, project).await,
    }
}

/// Every key filed under the gate's name in `server`, revoked: the service filling
/// `request.gate` is not one the server's credentials may be handed to, so nothing holds
/// them.
async fn withheld(ctx: &Ctx, server: &MediaServer) -> Option<Wiring> {
    let client = server.administering(ctx).await?;
    let filed = client.filed_as(GATE_APP).await.ok()?;
    minted::retired(ctx, client.as_ref(), GATE_APP, CONNECTION, &filed).await
}

/// The gate as the core asks it, and every route it admits now — or why there is neither.
async fn asked(
    ctx: &Ctx,
    fillers: &crate::wiring::Fillers,
    project: Option<&Path>,
) -> Result<(Arc<dyn Gating>, Vec<Admitted>), State> {
    let project = project.ok_or_else(no_project)?;
    if !gating::may_hold(fillers, crate::wiring::Holder::Stack) {
        return Err(State::Skipped {
            reason: "the service filling request.gate is not one the stack's credentials may \
                     be handed to, so it is handed no routes"
                .to_owned(),
        });
    }
    let gate = gating::asked(ctx, fillers, project)
        .await
        .ok_or_else(|| State::Skipped {
            reason: "the request gate cannot be asked over its contract, so it is handed no \
                     routes"
                .to_owned(),
        })?;
    let admitted = gate
        .admitted()
        .await
        .map_err(|failure| crate::seed::unreached(&failure))?;
    Ok((gate, admitted))
}

/// The gate's routes where it answers for no media server: one for each fulfilling
/// curator, and no key minted for it anywhere.
async fn curators_alone(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
    fillers: &crate::wiring::Fillers,
    project: Option<&Path>,
) -> Vec<Wiring> {
    if gating::service(services).is_none() {
        return Vec::new();
    }
    let (gate, admitted) = match asked(ctx, fillers, project).await {
        Ok(asked) => asked,
        Err(state) => return vec![settled(state)],
    };
    let (routes, unrouted) = routing(ctx, fillers);
    let wanted = kept_tokens(routes, &admitted);
    let state = if holds(&admitted, &wanted) {
        State::AlreadyWired
    } else if ctx.dry_run {
        State::WouldWire {
            yours: Some(listed(&admitted)),
            ours: Some(named(&wanted)),
        }
    } else {
        match gate.admit(&wanted).await {
            Ok(()) => State::Wired,
            Err(failure) => unadmitted(&failure),
        }
    };
    said_with(state, unrouted)
}

/// The gate's routes with one for the media server, as `routed` reaches it, under a key
/// minted for the gate alone and filed under its name.
async fn with_the_media_server(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
    fillers: &crate::wiring::Fillers,
    (server, pairing): (&MediaServer, &Pairing),
    project: Option<&Path>,
) -> Vec<Wiring> {
    let gating = gating::service(services).is_some();
    // Minted with the administrator's session, which lemonfiber holds only on a server
    // it set up; a rehearsal before the first run finds none recorded yet.
    let Some(client) = server.administering(ctx).await else {
        let minting = server.would_mint(ctx);
        return (gating && minting)
            .then(|| {
                settled(State::WouldWire {
                    yours: None,
                    ours: None,
                })
            })
            .into_iter()
            .collect();
    };
    let client = client.as_ref();
    let filed = client.filed_as(GATE_APP).await;
    // A stack that does not run the gate asked for nothing here: a key list that could
    // not be read is left for the next run to retire from, not reported.
    if !gating {
        let Ok(filed) = filed else {
            return Vec::new();
        };
        return minted::retired(ctx, client, GATE_APP, CONNECTION, &filed)
            .await
            .into_iter()
            .collect();
    }
    let filed = match filed {
        Ok(filed) => filed,
        Err(failure) => return vec![settled(crate::seed::unreached(&failure))],
    };
    let Some(internal) = client
        .declared()
        .await
        .ok()
        .and_then(|declared| declared.ports.internal_tls)
    else {
        let mut wirings = curators_alone(ctx, services, fillers, project).await;
        wirings.push(Wiring::settled(
            format!("{} routed through the request gate", server.name()),
            State::Unmatched {
                reason: format!(
                    "{} declares no internal encrypted port, the one port the gate may reach \
                     it at",
                    server.name()
                ),
            },
        ));
        return wirings;
    };
    let routed = &Routed {
        server,
        pairing,
        internal,
        pins: super::encrypting::pins(ctx, server),
    };
    let (gate, admitted) = match asked(ctx, fillers, project).await {
        Ok(asked) => asked,
        Err(state) => return vec![settled(state)],
    };
    let (curators, unrouted) = routing(ctx, fillers);
    let state = match held(&admitted, &filed) {
        Some(key) => {
            let wanted = kept_tokens(with(&curators, to_media(routed, &key)), &admitted);
            let others: Vec<&String> = filed.iter().filter(|one| **one != key).collect();
            kept(ctx, client, gate.as_ref(), &admitted, &wanted, others).await
        }
        None if ctx.dry_run => State::WouldWire {
            yours: Some(listed(&admitted)),
            ours: Some(named(&with(&curators, to_media(routed, "")))),
        },
        None => {
            let key = match minted::mint(client, GATE_APP).await {
                Ok(key) => key,
                Err(state) => return said_with(state, unrouted),
            };
            let wanted = kept_tokens(with(&curators, to_media(routed, &key)), &admitted);
            if let Err(failure) = gate.admit(&wanted).await {
                let _ = client.revoke(&key).await;
                return said_with(
                    State::Failed {
                        detail: format!(
                            "the routes could not be handed to the request gate, so the new \
                             key was revoked again: {failure}"
                        ),
                    },
                    unrouted,
                );
            }
            minted::revoking(client, &filed).await
        }
    };
    said_with(state, unrouted)
}

/// The key filed under the gate's name that the gate presents on its media server route,
/// told by its SHA-256.
pub(crate) fn held(admitted: &[Admitted], filed: &[String]) -> Option<String> {
    let presented = admitted
        .iter()
        .find(|route| matches!(route.target, Target::MediaServer { .. }))?;
    filed
        .iter()
        .find(|key| crate::ports::digest::sha256(key) == presented.credential)
        .cloned()
}

/// `wanted`, each holding the tokens the gate admits on its route now.
fn kept_tokens(wanted: Vec<Gated>, admitted: &[Admitted]) -> Vec<Gated> {
    wanted
        .into_iter()
        .map(|route| Gated {
            tokens: admitted
                .iter()
                .find(|one| one.path == route.path)
                .map(|one| one.tokens.clone())
                .unwrap_or_default(),
            ..route
        })
        .collect()
}

/// Whether the gate admits `wanted` and nothing else, every credential the same.
fn holds(admitted: &[Admitted], wanted: &[Gated]) -> bool {
    let mut answered: Vec<Admitted> = wanted.iter().map(Admitted::of).collect();
    let mut admitted = admitted.to_vec();
    answered.sort_by(|one, other| one.path.cmp(&other.path));
    admitted.sort_by(|one, other| one.path.cmp(&other.path));
    answered == admitted
}

/// Why the gate would not take its routes, for the report.
fn unadmitted(failure: &crate::ports::service::Failure) -> State {
    State::Failed {
        detail: format!("the routes could not be handed to the request gate: {failure}"),
    }
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
        .into_iter()
        .next()
        .map(|wiring| wiring.state)
}

/// The gate already holds a key the media server lists: bring its routes up to what they
/// should be, and revoke whatever else is filed under its name.
async fn kept(
    ctx: &Ctx,
    client: &dyn AppKeys,
    gate: &dyn Gating,
    admitted: &[Admitted],
    wanted: &[Gated],
    others: Vec<&String>,
) -> State {
    let moved = !holds(admitted, wanted);
    if !moved && others.is_empty() {
        return State::AlreadyWired;
    }
    // Said by what changes: the routes where they move, and otherwise the keys.
    if ctx.dry_run && moved {
        return State::WouldWire {
            yours: Some(listed(admitted)),
            ours: Some(named(wanted)),
        };
    }
    if ctx.dry_run {
        return State::WouldWire {
            yours: Some(minted::count(GATE_APP, others.len() + 1)),
            ours: Some(minted::count(GATE_APP, 1)),
        };
    }
    if moved {
        if let Err(failure) = gate.admit(wanted).await {
            return unadmitted(&failure);
        }
    }
    minted::revoking(client, others).await
}

/// A route for each curator the request service fulfils through, with the key lemonfiber
/// minted for it. A curator holding no minted key has no route.
pub(crate) fn curator_routes(ctx: &Ctx, fillers: &crate::wiring::Fillers) -> Vec<Gated> {
    routing(ctx, fillers).0
}

/// [`curator_routes`], and a line for each curator the request service fulfils through
/// that the gate is handed no route to, saying why: one whose credential the gate may not
/// hold, and one holding no minted key.
fn routing(ctx: &Ctx, fillers: &crate::wiring::Fillers) -> (Vec<Gated>, Vec<Wiring>) {
    let mut routes = Vec::new();
    let mut unrouted = Vec::new();
    for fulfils in super::fulfilling(fillers) {
        let filler = fulfils.filler;
        let connection = format!("{} routed through the request gate", filler.name);
        if !gating::may_hold(fillers, filler.holder()) {
            unrouted.push(Wiring::settled(
                connection,
                State::Unmatched {
                    reason: format!(
                        "{} is a third-party plugin's service, whose credential is not handed \
                         to the service filling request.gate",
                        filler.name
                    ),
                },
            ));
            continue;
        }
        let Some(key) = super::keys::minted_key(ctx, filler) else {
            unrouted.push(super::skipped(connection, &filler.name));
            continue;
        };
        routes.push(Gated {
            target: Target::Curator {
                media_type: fulfils.kind.media_type().to_owned(),
            },
            path: filler.id.clone(),
            address: fulfils.at.url(),
            majors: Vec::new(),
            credential: key,
            tokens: Vec::new(),
        });
    }
    (routes, unrouted)
}

/// The media server as the gate routes to it: where the API it speaks is answered, the
/// internal TLS port it is reached at and the certificates that port may present.
struct Routed<'a> {
    server: &'a MediaServer,
    pairing: &'a Pairing,
    internal: u16,
    pins: Vec<String>,
}

/// The route to the media server, named by its id, reaching the API other services sign
/// the household in through it in at its internal TLS port, accepting only the
/// certificates it may present, forwarding to the majors of it the stack supports and
/// presenting `key`.
fn to_media(routed: &Routed<'_>, key: &str) -> Gated {
    Gated {
        target: Target::MediaServer {
            pins: routed.pins.clone(),
        },
        path: routed.server.id().to_owned(),
        address: routed.pairing.at.encrypted_at(routed.internal),
        majors: routed.pairing.majors.clone(),
        credential: key.to_owned(),
        tokens: Vec::new(),
    }
}

/// `routes`, and the media server's route after them.
fn with(routes: &[Gated], media: Gated) -> Vec<Gated> {
    let mut all = routes.to_vec();
    all.push(media);
    all
}

/// What a set of routes answers for, said by their names and never their credentials.
fn named(routes: &[Gated]) -> String {
    said(routes.iter().map(|route| route.path.as_str()))
}

/// What the gate admits now, said the same way.
fn listed(admitted: &[Admitted]) -> String {
    said(admitted.iter().map(|route| route.path.as_str()))
}

/// Routes by their names.
fn said<'a>(paths: impl Iterator<Item = &'a str>) -> String {
    let names: Vec<&str> = paths.collect();
    if names.is_empty() {
        return "no routes".to_owned();
    }
    format!("routes to {}", names.join(", "))
}

/// This connection, resting in `state`.
fn settled(state: State) -> Wiring {
    Wiring::settled(CONNECTION.to_owned(), state)
}

/// This connection, resting in `state`, and each curator the gate is handed no route to.
fn said_with(state: State, unrouted: Vec<Wiring>) -> Vec<Wiring> {
    std::iter::once(settled(state)).chain(unrouted).collect()
}
