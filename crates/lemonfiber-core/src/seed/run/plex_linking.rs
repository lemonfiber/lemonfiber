//! The request service linked to a Plex media server in the mode `PLEX_LINK_MODE` names,
//! through the request gate's Plex route.
//!
//! Plex is claimed over `owner.claim`, so lemonfiber holds no administrator for it. The
//! request service's owner signs in at the gate's Plex route instead: with the request
//! service's own managed account token in `managed`, which the gate holds as its key for
//! Plex; with a gate token minted for it alone in `fronted`; and by the operator's own
//! sign-in on its page in `owner`, where lemonfiber carries nothing.

use lemonfiber_contract::capabilities::media::serve;

use super::tokens::{Kept, Presented};
use super::Ctx;
use crate::app::targets::MediaServer;
use crate::app_keys::GATE_APP;
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

/// The authority the request service trusts for plex.tv and the certificate the gate
/// answers for it under, kept in `fronted` mode and taken away in every other; the gate
/// and the request service started again where they are written anew, so each reads them.
pub(super) async fn seed_fronting(ctx: &Ctx, fillers: &Fillers) -> Option<Wiring> {
    use crate::certificate::fronted::{kept, kept_or_made, withdrawn, HOSTS};
    let parts = crate::plex_link::fronting(fillers)?;
    let at = crate::plex_link::beneath(&parts)?;
    let confined = ctx.seams.confined.as_ref();
    let rests = |state| Wiring::settled(FRONTING.to_owned(), state);
    let failed = |why: crate::certificate::Unkept| {
        rests(State::Failed {
            detail: why.to_string(),
        })
    };
    if crate::plex_link::mode(ctx) != Some(Mode::Fronted) {
        if ctx.dry_run {
            return None;
        }
        return withdrawn(confined, at).err().map(failed);
    }
    if ctx.dry_run {
        return Some(match kept(confined, at) {
            Ok(true) => rests(State::AlreadyWired),
            Ok(false) => rests(State::WouldWire {
                yours: None,
                ours: Some(HOSTS.join(", ")),
            }),
            Err(why) => failed(why),
        });
    }
    match kept_or_made(confined, at) {
        Ok(false) => Some(rests(State::AlreadyWired)),
        Ok(true) => {
            for id in [&parts.gate.id, &parts.requests.id] {
                if let Err(detail) = super::restarting::restart(ctx, id).await {
                    return Some(rests(State::Failed { detail }));
                }
            }
            Some(rests(State::Wired))
        }
        Err(why) => Some(failed(why)),
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
    let rests = |state| Wiring::settled(crate::seed::IDENTITY.to_owned(), state);
    if !gate || !server.plex() {
        return rests(State::Skipped {
            reason: format!(
                "{} is claimed against an account outside the house, and the request service \
                 is linked to it only through the request gate's Plex route, which this stack \
                 does not answer",
                server.name()
            ),
        });
    }
    let Some(mode) = crate::plex_link::mode(ctx) else {
        return rests(unnamed(ctx));
    };
    match mode {
        Mode::Owner => crate::seed::wire_request_source(requests, &source(None), ctx.dry_run).await,
        Mode::Managed => match held_key(ctx, fillers, server).await {
            Some(key) => {
                let signing = source(Some(Credential::ApiKey(key)));
                crate::seed::wire_request_source(requests, &signing, ctx.dry_run).await
            }
            None => rests(unheld(server)),
        },
        Mode::Fronted => fronted(ctx, fillers, requests, server).await,
    }
}

/// The request service signed in with the gate token, minted where the gate's Plex route
/// admits none, so its owner holds it; and with no credential where the route already
/// admits one, which only its owner holds.
async fn fronted(
    ctx: &Ctx,
    fillers: &Fillers,
    requests: &dyn Requests,
    server: &MediaServer,
) -> Wiring {
    let rests = |state| Wiring::settled(crate::seed::IDENTITY.to_owned(), state);
    let (Some(kept), Some(key)) = (
        Kept::read(ctx, fillers).await,
        held_key(ctx, fillers, server).await,
    ) else {
        return rests(unheld(server));
    };
    let route = server.id();
    if !gate_tokens(kept.admitted(), route, &key).is_empty() {
        return crate::seed::wire_request_source(requests, &source(None), ctx.dry_run).await;
    }
    if ctx.dry_run {
        return rests(State::WouldWire {
            yours: None,
            ours: Some(url()),
        });
    }
    let Some(token) = crate::secret::generate(ctx.seams.random.as_ref()) else {
        return rests(State::Failed {
            detail: crate::secret::NO_RANDOMNESS_FOR_TOKEN.to_owned(),
        });
    };
    let presented = [Presented {
        route: route.to_owned(),
        token: token.clone(),
    }];
    let beside = match kept.beside(&presented).await {
        Ok(beside) => beside,
        Err(reason) => {
            return rests(State::Failed {
                detail: Kept::unwritten(&reason),
            })
        }
    };
    let wiring =
        crate::seed::wire_request_source(requests, &source(Some(Credential::ApiKey(token))), false)
            .await;
    let signed = matches!(wiring.state, State::Wired | State::AlreadyWired);
    let handed = if signed {
        kept.only(&beside, &presented, &[true]).await
    } else {
        kept.restore().await
    };
    match handed {
        Ok(()) => wiring,
        Err(reason) => rests(State::Failed {
            detail: Kept::unwritten(&reason),
        }),
    }
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
    match crate::plex_link::mode(ctx).ok_or_else(|| unnamed(ctx))? {
        Mode::Managed => Ok(PlexLink::Managed),
        Mode::Owner => {
            let owning = server.owning(ctx).await.ok_or_else(|| State::Skipped {
                reason: format!(
                    "{} cannot be asked over owner.claim who claimed it, so the request gate \
                     is handed no Plex route",
                    server.name()
                ),
            })?;
            match owning.claimed_by().await.map_err(unreached)? {
                Some(account) => Ok(PlexLink::Owner { account }),
                None => Err(State::Skipped {
                    reason: format!(
                        "{} names no account that claimed it yet, so the request gate is \
                         handed no Plex route",
                        server.name()
                    ),
                }),
            }
        }
        Mode::Fronted => {
            let account = client.account(GATE_APP).await.map_err(unreached)?;
            let listed = client.listed_as().await.map_err(unreached)?;
            let household = server
                .identifying(ctx)
                .await
                .ok_or_else(|| State::Skipped {
                    reason: format!(
                        "{} cannot be asked over identity.source for its household, so the \
                     request gate is handed no Plex route",
                        server.name()
                    ),
                })?;
            let members = household
                .household()
                .await
                .map_err(unreached)?
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

/// The identity source the request service's owner signs in through: Plex's API at the
/// gate's Plex route, carrying `credential`.
fn source(credential: Option<Credential>) -> IdentitySource {
    IdentitySource {
        at: url(),
        protocol: Protocol(crate::plex_link::API.to_owned()),
        credential,
    }
}

/// Where the request service is told Plex is: the gate's Plex route.
fn url() -> String {
    let at = route();
    format!("http://{}:{}", at.host, at.port)
}

/// What is said where the gate holds no Plex account of the request service's own yet.
fn unheld(server: &MediaServer) -> State {
    State::Skipped {
        reason: format!(
            "the request gate holds no account of the request service's own in {} yet, so \
             the request service is not signed in",
            server.name()
        ),
    }
}

/// What is said where the mode recorded names none.
fn unnamed(ctx: &Ctx) -> State {
    let held = crate::settings::recorded_value(ctx, crate::plex_link::SETTING);
    State::Failed {
        detail: crate::plex_link::refusal(held.as_deref().unwrap_or_default()).unwrap_or_default(),
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
    let rests = |state| super::linking::settled(asker, server, state);
    let at = route();
    let link = match requests.media_server_link().await {
        Ok(link) => link,
        Err(failure) => return rests(crate::seed::unreached(&failure)),
    };
    if link.at == at {
        return rests(State::AlreadyWired);
    }
    if ctx.dry_run {
        return rests(State::WouldWire {
            yours: (!link.at.host.is_empty()).then(|| crate::seed::reached_at(&link.at)),
            ours: Some(crate::seed::reached_at(&at)),
        });
    }
    match requests.link_media_server(&at, "").await {
        Ok(()) => rests(State::Wired),
        Err(failure) => rests(crate::seed::unreached(&failure)),
    }
}

/// What a rehearsal says where the request service is not set up yet: it would be
/// pointed at the gate's Plex route once it is.
pub(super) fn would_link(asker: &Filler, server: &MediaServer) -> Wiring {
    super::linking::settled(
        asker,
        server,
        State::WouldWire {
            yours: None,
            ours: Some(crate::seed::reached_at(&route())),
        },
    )
}

#[cfg(test)]
mod tests;
