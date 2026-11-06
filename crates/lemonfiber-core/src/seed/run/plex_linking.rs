//! The request service linked to a Plex media server in the mode `PLEX_LINK_MODE` names,
//! through the request gate's Plex route.
//!
//! Plex is claimed over `owner.claim`, so lemonfiber holds no administrator for it. The
//! request service's owner signs in at the gate's Plex route instead: with the request
//! service's own managed account token in `managed`, which the gate holds as its key for
//! Plex; with a gate token minted for it alone in `fronted`; and by the operator's own
//! sign-in on its page in `owner`, where lemonfiber carries nothing.

use futures_util::future::OptionFuture;
use lemonfiber_contract::capabilities::media::serve;

use super::tokens::{Kept, Presented};
use super::Ctx;
use crate::app::targets::MediaServer;
use crate::app_keys::GATE_APP;
use crate::certificate::fronted::{kept, kept_or_made, withdrawn, Beneath, HOSTS};
use crate::plex_link::Mode;
use crate::ports::digest::sha256;
use crate::ports::service::{
    Credential, Endpoint, Failure, IdentitySource, Ownership, PlexLink, PlexMember, Protocol,
    Requests, PLEX_PORT,
};
use crate::seed::{State, Wiring};
use crate::wiring::{Filler, Fillers};

/// What the report calls plex.tv answered by the request gate.
const FRONTING: &str = "plex.tv answered by the request gate";

/// What is said where the request service is linked to a claimed server without the
/// gate's Plex route.
const UNROUTED: &str = "the media server is claimed against an account outside the house, \
                        and the request service is linked to it only through the request \
                        gate's Plex route, which this stack does not answer";

/// What is said where the gate holds no account of the request service's own yet.
const UNHELD: &str = "the request gate holds no account of the request service's own in the \
                      media server yet, so the request service is not signed in";

/// What is said where the account that claimed Plex cannot be named.
const UNCLAIMED: &str = "the media server names no account that claimed it yet, so the \
                         request gate is handed no Plex route";

/// What is said where the household cannot be read.
const UNREAD: &str = "the media server cannot be asked who is in its household, so the \
                      request gate is handed no Plex route";

/// The authority the request service trusts for plex.tv and the certificate the gate
/// answers for it under, kept in `fronted` mode and taken away in every other; the gate
/// and the request service started again where they are written anew, so each reads them.
pub(super) async fn seed_fronting(ctx: &Ctx, fillers: &Fillers) -> Option<Wiring> {
    let parts = crate::plex_link::fronting(fillers)?;
    let at = crate::plex_link::beneath(&parts)?;
    let confined = ctx.seams.confined.as_ref();
    if crate::plex_link::mode(ctx) != Some(Mode::Fronted) {
        let left = (!ctx.dry_run).then(|| withdrawn(confined, at));
        return left?.err().map(|why| fronting_rests(failed(&why)));
    }
    let state = if ctx.dry_run {
        kept(confined, at)
            .map(rehearsed)
            .map_err(|why| why.to_string())
    } else {
        written_anew(ctx, &parts, at).await
    };
    Some(fronting_rests(state.unwrap_or_else(|why| failed(&why))))
}

/// What a rehearsal says of the files, where they are `held` already.
fn rehearsed(held: bool) -> State {
    let ours = Some(HOSTS.join(", "));
    if held {
        State::AlreadyWired
    } else {
        State::WouldWire { yours: None, ours }
    }
}

/// The files beneath `at` kept, or written anew and the gate and the request service
/// `parts` names started again so each reads them.
async fn written_anew(
    ctx: &Ctx,
    parts: &crate::plex_link::Fronting<'_>,
    at: Beneath<'_>,
) -> Result<State, String> {
    let written = kept_or_made(ctx.seams.confined.as_ref(), at).map_err(|why| why.to_string())?;
    if !written {
        return Ok(State::AlreadyWired);
    }
    for id in [&parts.gate.id, &parts.requests.id] {
        super::restarting::restart(ctx, id).await?;
    }
    Ok(State::Wired)
}

/// plex.tv's front, resting in `state`.
fn fronting_rests(state: State) -> Wiring {
    Wiring::settled(FRONTING.to_owned(), state)
}

/// A failure, as the report says it.
fn failed(why: &impl std::fmt::Display) -> State {
    State::Failed {
        detail: why.to_string(),
    }
}

/// Where the request service reaches Plex: the gate's Plex route, at a bare host and port.
pub(crate) fn route() -> Endpoint {
    Endpoint {
        host: crate::app::gating::SERVICE.to_owned(),
        port: PLEX_PORT,
        base: String::new(),
    }
}

/// The request service, `requests`, signed in at the gate's Plex route as the mode
/// recorded says: the identity's second half, for a media server claimed over
/// `owner.claim`.
pub(super) async fn signed_in(
    ctx: &Ctx,
    fillers: &Fillers,
    requests: &dyn Requests,
    server: &MediaServer,
    gate: bool,
) -> Wiring {
    let state = if gate && server.plex() {
        signing(ctx, fillers, requests, server).await
    } else {
        skipped(UNROUTED)
    };
    Wiring::settled(crate::seed::IDENTITY.to_owned(), state)
}

/// What signing the request service in at the gate's Plex route comes to.
async fn signing(
    ctx: &Ctx,
    fillers: &Fillers,
    requests: &dyn Requests,
    server: &MediaServer,
) -> State {
    match crate::plex_link::mode(ctx) {
        Some(Mode::Owner) => signed(requests, None, ctx.dry_run).await,
        Some(Mode::Managed) => match held_key(ctx, fillers, server).await {
            Some(key) => signed(requests, Some(Credential::ApiKey(key)), ctx.dry_run).await,
            None => skipped(UNHELD),
        },
        Some(Mode::Fronted) => fronted(ctx, fillers, requests, server)
            .await
            .unwrap_or_else(|state| state),
        None => unnamed(ctx),
    }
}

/// The request service signed in with the gate token, minted where the gate's Plex route
/// admits none, so its owner holds it; and with no credential where the route already
/// admits one, which only its owner holds.
///
/// # Errors
///
/// What the identity rests in where no token could be minted or handed to the gate.
async fn fronted(
    ctx: &Ctx,
    fillers: &Fillers,
    requests: &dyn Requests,
    server: &MediaServer,
) -> Result<State, State> {
    let held = held_key(ctx, fillers, server).await;
    let (Some(kept), Some(key)) = (Kept::read(ctx, fillers).await, held) else {
        return Ok(skipped(UNHELD));
    };
    let route = server.id();
    let admitted = gate_tokens(kept.admitted(), route, &key);
    if ctx.dry_run && admitted.is_empty() {
        return Ok(State::WouldWire {
            yours: None,
            ours: Some(url()),
        });
    }
    if !admitted.is_empty() {
        return Ok(signed(requests, None, ctx.dry_run).await);
    }
    let unwritten = |reason: String| failed(&Kept::unwritten(&reason));
    let random = ctx.seams.random.as_ref();
    let token = crate::secret::generate(random)
        .ok_or_else(|| failed(&crate::secret::NO_RANDOMNESS_FOR_TOKEN))?;
    let presented = [Presented {
        route: route.to_owned(),
        token: token.clone(),
    }];
    let beside = kept.beside(&presented).await.map_err(unwritten)?;
    let state = signed(requests, Some(Credential::ApiKey(token)), false).await;
    let took = matches!(state, State::Wired | State::AlreadyWired);
    let handed = if took {
        kept.only(&beside, &presented, &[true]).await
    } else {
        kept.restore().await
    };
    handed.map_err(unwritten)?;
    Ok(state)
}

/// The request service signed in with `credential`, or none.
async fn signed(
    requests: &dyn Requests,
    credential: Option<Credential>,
    rehearsing: bool,
) -> State {
    let source = IdentitySource {
        at: url(),
        protocol: Protocol(crate::plex_link::API.to_owned()),
        credential,
    };
    crate::seed::wire_request_source(requests, &source, rehearsing)
        .await
        .state
}

/// What the gate's Plex route holds of the link in the mode recorded: the account that
/// claimed Plex in `owner`, and in `fronted` the request service's own account, the
/// household and the server, as `client`, Plex over `media.serve`, and `server` answer.
pub(super) async fn link(
    ctx: &Ctx,
    server: &MediaServer,
    client: &dyn serve::Fills,
) -> Result<PlexLink, State> {
    let unreached = |failure: Failure| crate::seed::unreached(&failure);
    match crate::plex_link::mode(ctx) {
        Some(Mode::Managed) => Ok(PlexLink::Managed),
        Some(Mode::Owner) => {
            let owning = server.owning(ctx).await;
            let asked = OptionFuture::from(owning.as_ref().map(Ownership::claimed_by)).await;
            let account = asked.transpose().map_err(unreached)?.flatten();
            account
                .map(|account| PlexLink::Owner { account })
                .ok_or(skipped(UNCLAIMED))
        }
        Some(Mode::Fronted) => {
            let account = client.account(GATE_APP).await.map_err(unreached)?;
            let listed = client.listed_as().await.map_err(unreached)?;
            let source = server.identifying(ctx).await.ok_or(skipped(UNREAD))?;
            let household = source.household().await.map_err(unreached)?;
            let members = household
                .into_iter()
                .map(|member| PlexMember {
                    id: member.id,
                    name: member.name,
                })
                .collect();
            Ok(PlexLink::Fronted {
                account,
                members,
                server: listed,
            })
        }
        None => Err(unnamed(ctx)),
    }
}

/// The tokens the gate's Plex route `route` admits among `admitted` other than the hash of
/// `key`, the request service's own managed account token: the gate tokens.
pub(crate) fn gate_tokens(
    admitted: &[crate::ports::service::Admitted],
    route: &str,
    key: &str,
) -> Vec<String> {
    let own = sha256(key);
    admitted
        .iter()
        .filter(|one| one.path == route)
        .flat_map(|one| one.tokens.iter())
        .filter(|token| **token != own)
        .cloned()
        .collect()
}

/// The tokens the gate's Plex route admits under `link`, presenting `key`, where it
/// admits `admitted` on it now: that key's hash in `managed`, none in `owner`, where the
/// gate asks plex.tv whose a token is, and the gate tokens it admits now in `fronted`.
pub(crate) fn admitting(link: &PlexLink, key: &str, admitted: &[String]) -> Vec<String> {
    let own = sha256(key);
    match link {
        PlexLink::Managed => vec![own],
        PlexLink::Owner { .. } => Vec::new(),
        PlexLink::Fronted { .. } => admitted
            .iter()
            .filter(|token| **token != own)
            .cloned()
            .collect(),
    }
}

/// The key filed under the gate's name that the gate presents on its Plex route: the
/// request service's own managed account token.
async fn held_key(ctx: &Ctx, fillers: &Fillers, server: &MediaServer) -> Option<String> {
    let gate = crate::app::gating::asked(ctx, fillers).await?;
    let admitted = gate.admitted().await.ok()?;
    let client = server.administering(ctx).await?;
    let filed = client.filed_as(GATE_APP).await.ok()?;
    super::gate::held(&admitted, &filed)
}

/// Where the request service is told Plex is: the gate's Plex route.
fn url() -> String {
    let at = route();
    format!("http://{}:{}", at.host, at.port)
}

/// Skipped, for `reason`.
fn skipped(reason: &str) -> State {
    State::Skipped {
        reason: reason.to_owned(),
    }
}

/// What is said where the mode recorded names none.
fn unnamed(ctx: &Ctx) -> State {
    let held = crate::settings::recorded_value(ctx, crate::plex_link::SETTING);
    failed(&crate::plex_link::refusal(held.as_deref().unwrap_or_default()).unwrap_or_default())
}

/// The request service `asker` is held to the gate's Plex route, as the identity resting
/// in `state` leaves it: said as a rehearsal where it would be set up, held where it is,
/// and nothing where it is not.
pub(super) async fn held_to_route(
    ctx: &Ctx,
    requests: &dyn Requests,
    asker: &Filler,
    server: &MediaServer,
    state: &State,
) -> Option<Wiring> {
    match state {
        State::WouldWire { .. } => Some(would_link(asker, server)),
        State::Wired | State::AlreadyWired => Some(linked(ctx, requests, asker, server).await),
        _ => None,
    }
}

/// Hold `requests`, the request service `asker` is, to reaching `server` at the gate's
/// Plex route, which it reaches with its owner's token and so with no key of its own.
pub(super) async fn linked(
    ctx: &Ctx,
    requests: &dyn Requests,
    asker: &Filler,
    server: &MediaServer,
) -> Wiring {
    let state = match requests.media_server_link().await {
        Ok(link) => relinked(ctx, requests, &link.at).await,
        Err(failure) => crate::seed::unreached(&failure),
    };
    super::linking::settled(asker, server, state)
}

/// The request service, reaching Plex at `was`, held to the gate's Plex route.
async fn relinked(ctx: &Ctx, requests: &dyn Requests, was: &Endpoint) -> State {
    let at = route();
    if *was == at {
        return State::AlreadyWired;
    }
    if ctx.dry_run {
        let yours = (!was.host.is_empty()).then(|| crate::seed::reached_at(was));
        let ours = Some(crate::seed::reached_at(&at));
        return State::WouldWire { yours, ours };
    }
    let written = requests.link_media_server(&at, "").await;
    written.map_or_else(
        |failure| crate::seed::unreached(&failure),
        |()| State::Wired,
    )
}

/// What a rehearsal says where the request service is not set up yet: it would be
/// pointed at the gate's Plex route once it is.
pub(super) fn would_link(asker: &Filler, server: &MediaServer) -> Wiring {
    let ours = Some(crate::seed::reached_at(&route()));
    super::linking::settled(asker, server, State::WouldWire { yours: None, ours })
}

#[cfg(test)]
mod tests;
