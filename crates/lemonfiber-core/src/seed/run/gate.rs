//! The request gate's routes: each upstream it answers for, and what it presents there.
//!
//! The gate holds a key of its own on each fulfilling curator's adapter, which opens only
//! the operations a curator answers the gate, under the route named for the media the
//! curator files, and a key minted for the gate alone in the media server, whichever
//! service fills the identity source, and filed under its name. They are handed over
//! through `request.gate`, never its environment, and again whenever what they should hold
//! has moved.
//!
//! **A curator the request service fulfils through that the gate holds no route to is
//! said**, on a line of its own beside the gate's, with why its key could not be written.
//!
//! **The gate is what holds the media server's key.** A key filed under the gate's name
//! whose SHA-256 the gate does not answer is one nothing holds, and is revoked, as the
//! decline service's are.

use std::sync::Arc;

use lemonfiber_contract::capabilities::media::serve;

use super::minted;
use super::Ctx;
use crate::app::gating;
use crate::app::targets::{MediaServer, Pairing};
use crate::app_keys::GATE_APP;
use crate::ports::service::{Admitted, AppKeys, Gated, Gating, PlexLink, Target};
use crate::seed::{State, Wiring};

/// What the report calls this connection.
const CONNECTION: &str = "The request gate's routes";

/// Hold the gate to a route for each fulfilling curator and, where it asks the media
/// server in the API the server is paired in and may hold its key, one for the media
/// server under a key of its own — or, where it cannot route the server, to no key at all.
pub(super) async fn seed_gate_routes(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
    fillers: &crate::wiring::Fillers,
    server: Option<&MediaServer>,
) -> Vec<Wiring> {
    let Some(server) = server else {
        return curators_alone(ctx, services, fillers).await;
    };
    match server.gated() {
        Some(pairing) if gating::may_hold(fillers, server.filler.holder()) => {
            with_the_media_server(ctx, services, fillers, (server, pairing)).await
        }
        _ => {
            let mut wirings = curators_alone(ctx, services, fillers).await;
            wirings.extend(withheld(ctx, server).await);
            wirings
        }
    }
}

/// Every key filed under the gate's name in `server`, revoked: no gate routes the server,
/// or the one filling `request.gate` may not be handed its credentials, so nothing holds
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
) -> Result<(Arc<dyn Gating>, Vec<Admitted>), State> {
    if !gating::may_hold(fillers, crate::wiring::Holder::Stack) {
        return Err(State::Skipped {
            reason: "the service filling request.gate is not one the stack's credentials may \
                     be handed to, so it is handed no routes"
                .to_owned(),
        });
    }
    let gate = gating::asked(ctx, fillers)
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
) -> Vec<Wiring> {
    if gating::service(services).is_none() {
        return Vec::new();
    }
    let (gate, admitted) = match asked(ctx, fillers).await {
        Ok(asked) => asked,
        Err(state) => return vec![settled(state)],
    };
    let (routes, unrouted) = routing(ctx, fillers, &admitted, Minting::Minted);
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
        let mut wirings = curators_alone(ctx, services, fillers).await;
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
    let pins = match super::encrypting::pins(ctx, server) {
        Ok(pins) => pins,
        Err(detail) => return vec![settled(State::Failed { detail })],
    };
    let routed = &Routed {
        server,
        pairing,
        internal,
        pins,
    };
    let (gate, admitted) = match asked(ctx, fillers).await {
        Ok(asked) => asked,
        Err(state) => return vec![settled(state)],
    };
    let (curators, unrouted) = routing(ctx, fillers, &admitted, Minting::Minted);
    let state = match held(&admitted, &filed) {
        Some(key) => {
            let wanted = match wanted(ctx, &curators, routed, (client, &key), &admitted).await {
                Ok(wanted) => wanted,
                Err(state) => return said_with(state, unrouted),
            };
            let others: Vec<&String> = filed.iter().filter(|one| **one != key).collect();
            kept(ctx, client, gate.as_ref(), &admitted, &wanted, others).await
        }
        None if ctx.dry_run => State::WouldWire {
            yours: Some(listed(&admitted)),
            ours: Some(named(&with(&curators, to_media(routed, "", None)))),
        },
        None => match minted_into(ctx, (client, gate.as_ref()), &curators, routed, &admitted).await
        {
            Ok(()) => minted::revoking(client, &filed).await,
            Err(state) => state,
        },
    };
    said_with(state, unrouted)
}

/// A key minted for the gate alone, handed to it on its route to the media server beside
/// `curators`' routes; revoked again where the routes could not be handed over.
async fn minted_into(
    ctx: &Ctx,
    (client, gate): (&dyn serve::Fills, &dyn Gating),
    curators: &[Gated],
    routed: &Routed<'_>,
    admitted: &[Admitted],
) -> Result<(), State> {
    let key = minted::mint(client, GATE_APP).await?;
    let wanted = match wanted(ctx, curators, routed, (client, &key), admitted).await {
        Ok(wanted) => wanted,
        Err(state) => {
            let _ = client.revoke(&key).await;
            return Err(state);
        }
    };
    if let Err(failure) = gate.admit(&wanted).await {
        let _ = client.revoke(&key).await;
        return Err(State::Failed {
            detail: format!(
                "the routes could not be handed to the request gate, so the new key was \
                 revoked again: {failure}"
            ),
        });
    }
    Ok(())
}

/// The key filed under the gate's name that the gate presents on its media server route,
/// told by its SHA-256.
pub(crate) fn held(admitted: &[Admitted], filed: &[String]) -> Option<String> {
    let presented = admitted
        .iter()
        .find(|route| gating::media_pins(&route.target).is_some())?;
    filed
        .iter()
        .find(|key| crate::ports::digest::sha256(key) == presented.credential)
        .cloned()
}

/// The routes the gate is to admit: `curators`, and the route to the media server
/// presenting `key`, each holding the tokens it admits now, save a Plex route, which holds
/// those its link admits.
async fn wanted(
    ctx: &Ctx,
    curators: &[Gated],
    routed: &Routed<'_>,
    (client, key): (&dyn serve::Fills, &str),
    admitted: &[Admitted],
) -> Result<Vec<Gated>, State> {
    let link = if routed.server.plex() {
        Some(super::plex_linking::link(ctx, routed.server, client).await?)
    } else {
        None
    };
    let mut all = kept_tokens(
        with(curators, to_media(routed, key, link.clone())),
        admitted,
    );
    if let (Some(link), Some(media)) = (&link, all.last_mut()) {
        media.tokens = super::plex_linking::admitting(link, key, &media.tokens);
    }
    Ok(all)
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

/// A route for each curator the request service fulfils through, presenting the key on
/// its adapter the gate presents now, as `admitted` says; a curator whose adapter holds no
/// such key has no route.
pub(crate) fn curator_routes(
    ctx: &Ctx,
    fillers: &crate::wiring::Fillers,
    admitted: &[Admitted],
) -> Vec<Gated> {
    routing(ctx, fillers, admitted, Minting::Never).0
}

/// Each curator the gate routes to: the route it answers under, the curator, and the
/// service speaking `library.curate` for it, which the gate asks.
pub(crate) fn routed_curators(
    fillers: &crate::wiring::Fillers,
) -> Vec<(&'static str, &crate::wiring::Filler, &crate::wiring::Filler)> {
    super::fulfilling(fillers)
        .into_iter()
        .map(|fulfils| {
            (
                super::tokens::route_of(fulfils.kind),
                fulfils.filler,
                fulfils.adapter,
            )
        })
        .collect()
}

/// Whether a curator's route whose adapter holds no key the gate presents is given one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Minting {
    /// It is given a key minted for it, written beside the adapter's configuration.
    Minted,
    /// It is given none, and has no route.
    Never,
}

/// The routes [`curator_routes`] answers, minting a key where `minting` says, and a line
/// for each curator the request service fulfils through that the gate is handed no route
/// to, saying why.
fn routing(
    ctx: &Ctx,
    fillers: &crate::wiring::Fillers,
    admitted: &[Admitted],
    minting: Minting,
) -> (Vec<Gated>, Vec<Wiring>) {
    let mut routes = Vec::new();
    let mut unrouted = Vec::new();
    for fulfils in super::fulfilling(fillers) {
        let path = super::tokens::route_of(fulfils.kind);
        let connection = format!("{} routed through the request gate", fulfils.filler.name);
        let presented = admitted
            .iter()
            .find(|route| route.path == path)
            .map(|route| route.credential.as_str());
        let credential = match curators_key(ctx, fulfils.adapter, presented, minting) {
            Ok(Some(credential)) => credential,
            Ok(None) => continue,
            Err(detail) => {
                unrouted.push(Wiring::settled(connection, State::Failed { detail }));
                continue;
            }
        };
        let at = fulfils.at;
        routes.push(Gated {
            target: Target::Curator {
                media_type: fulfils.kind.media_type().to_owned(),
            },
            path: path.to_owned(),
            address: format!(
                "http://{}:{}",
                at.host,
                fulfils.adapter.contract.unwrap_or(at.port)
            ),
            majors: Vec::new(),
            credential,
            tokens: Vec::new(),
        });
    }
    (routes, unrouted)
}

/// The key the gate is to present to `adapter`: the one written there whose SHA-256 the
/// gate presents now, or, where `minting` allows, a new one written beside the rest; none
/// where it allows none, and nothing written on a rehearsal, which presents no key.
fn curators_key(
    ctx: &Ctx,
    adapter: &crate::wiring::Filler,
    presented: Option<&str>,
    minting: Minting,
) -> Result<Option<String>, String> {
    let mut keys = super::request_keys::Keys::of(ctx, adapter)?;
    if let Some(key) = presented.and_then(|presented| keys.presented(presented)) {
        return Ok(Some(key.to_owned()));
    }
    match minting {
        Minting::Never => Ok(None),
        Minting::Minted if ctx.dry_run => Ok(Some(String::new())),
        Minting::Minted => keys.fresh(ctx).map(Some),
    }
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
/// presenting `key`: a Plex route holding `link` where there is one, which the gate
/// answers at a port of its own rather than under the name.
fn to_media(routed: &Routed<'_>, key: &str, link: Option<PlexLink>) -> Gated {
    let pins = routed.pins.clone();
    Gated {
        target: match link {
            Some(link) => Target::Plex { pins, link },
            None if routed.pairing.protocol.0 == gating::EMBY_API => Target::Emby { pins },
            None => Target::MediaServer { pins },
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
    said(
        &routes
            .iter()
            .map(|route| route.path.as_str())
            .collect::<Vec<_>>(),
    )
}

/// What the gate admits now, said the same way.
fn listed(admitted: &[Admitted]) -> String {
    said(
        &admitted
            .iter()
            .map(|route| route.path.as_str())
            .collect::<Vec<_>>(),
    )
}

/// Routes by their names.
fn said(names: &[&str]) -> String {
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
