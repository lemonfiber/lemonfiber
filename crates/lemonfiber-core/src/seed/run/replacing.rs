//! Replacing the media server's internal certificate while every holder of a media server
//! credential keeps reaching it.
//!
//! The replacement is served only once each holder holds its SHA-256 beside the old one,
//! and the old one is withdrawn only once each holder answers the new one's SHA-256 among
//! those it holds, the media server answers ready, and a handshake to its internal port
//! presents the new one. A step that cannot be taken, or whose outcome cannot be read,
//! stops the replacement where it stands, with every holder still handed every
//! certificate kept, so no holder is left without the certificate it is served.

use std::path::Path;
use std::sync::Arc;

use futures_util::future::OptionFuture;
use lemonfiber_contract::capabilities::media::serve::Fills;

use super::encrypting::{inside, served};
use super::Ctx;
use crate::app::targets::MediaServer;
use crate::app::{gating, invite::declining};
use crate::certificate::internal;
use crate::journal::{Change, Kind};
use crate::ports::service::{Applied, Target};
use crate::seed::{State, Wiring};

/// The operation a replacement is journalled under.
pub(crate) const OPERATION: &str = "internal-certificate";

/// What the decline service is called where it is named as a holder.
const DECLINE: &str = "the decline service";

/// What the request gate is called where it is named as a holder.
const GATE: &str = "the request gate";

/// The stack a replacement reaches into: its services, who fills each of its asks and
/// where it was written to disk.
#[derive(Clone, Copy)]
pub(crate) struct Stack<'a> {
    /// The stack's own services.
    pub(crate) services: &'a [lemonfiber_manifest::Service],
    /// Who fills each ask.
    pub(crate) fillers: &'a crate::wiring::Fillers,
}

/// What the report calls this connection.
fn connection(server: &MediaServer) -> String {
    format!(
        "{} replacing the certificate its internal encrypted port presents",
        server.name()
    )
}

/// Every holder the core hands a credential of `server`, by what it is called: the
/// decline service where it may hold the server's key, and the request gate where it may
/// hold the stack's credentials.
pub(crate) fn holders(stack: Stack<'_>, server: &MediaServer) -> Vec<&'static str> {
    [
        (
            declining::service(stack.services).is_some()
                && declining::may_hold(stack.fillers, &server.filler),
            DECLINE,
        ),
        (
            gating::service(stack.services).is_some()
                && gating::may_hold(stack.fillers, crate::wiring::Holder::Stack),
            GATE,
        ),
    ]
    .into_iter()
    .filter_map(|(holds, holder)| holds.then_some(holder))
    .collect()
}

/// Each holder, and whether it answers `pin` among the SHA-256s it holds: not where it
/// cannot be asked.
async fn holding(
    ctx: &Ctx,
    stack: Stack<'_>,
    server: &MediaServer,
    pin: &str,
) -> Vec<(&'static str, bool)> {
    let mut holding = Vec::new();
    for holder in holders(stack, server) {
        let holds = match holder {
            DECLINE => decline_holds(ctx, stack.fillers, pin).await,
            _ => gate_holds(ctx, stack.fillers, pin).await,
        };
        holding.push((holder, holds));
    }
    holding
}

/// Every holder that does not answer `pin` among the SHA-256s it holds, or cannot be asked.
async fn unheld(ctx: &Ctx, stack: Stack<'_>, server: &MediaServer, pin: &str) -> Vec<&'static str> {
    holding(ctx, stack, server, pin)
        .await
        .into_iter()
        .filter_map(|(holder, holds)| (!holds).then_some(holder))
        .collect()
}

/// Whether the decline service holds `pin` with its key, or holds no key to strand.
async fn decline_holds(ctx: &Ctx, fillers: &crate::wiring::Fillers, pin: &str) -> bool {
    let decline = declining::asked(ctx, fillers).await;
    OptionFuture::from(decline.as_ref().map(|decline| decline.held()))
        .await
        .is_some_and(|held| {
            held.is_ok_and(|held| held.is_none_or(|held| held.pins.iter().any(|one| one == pin)))
        })
}

/// Whether the request gate's route to the media server holds `pin`, or it has none.
async fn gate_holds(ctx: &Ctx, fillers: &crate::wiring::Fillers, pin: &str) -> bool {
    let gate = gating::asked(ctx, fillers).await;
    OptionFuture::from(gate.as_ref().map(|gate| gate.admitted()))
        .await
        .is_some_and(|admitted| {
            admitted.is_ok_and(|admitted| {
                !admitted.iter().any(|route| {
                    matches!(&route.target, Target::MediaServer { pins } if !pins.iter().any(|one| one == pin))
                })
            })
        })
}

/// Replace the media server's internal certificate on the operator's ask: a replacement
/// made, handed to every holder beside the one in force, and taken as far as [`advanced`]
/// takes it.
pub(crate) async fn replaced(ctx: &Ctx, stack: Stack<'_>, server: &MediaServer) -> Vec<Wiring> {
    let Some(config) = server.served.confined_to.as_deref() else {
        return Vec::new();
    };
    if let Err(unkept) = internal::staged(ctx.seams.confined.as_ref(), config, &server.served.id) {
        return vec![Wiring::settled(
            connection(server),
            State::Failed {
                detail: unkept.to_string(),
            },
        )];
    }
    let mut wirings = handed(ctx, stack, server).await;
    wirings.extend(Box::pin(advanced(ctx, stack, Some(server))).await);
    wirings
}

/// Every holder handed every certificate kept for `server`, as the seed hands them.
async fn handed(ctx: &Ctx, stack: Stack<'_>, server: &MediaServer) -> Vec<Wiring> {
    let mut wirings: Vec<Wiring> =
        super::decline::seed_decline_key(ctx, stack.services, stack.fillers)
            .await
            .into_iter()
            .collect();
    wirings.extend(
        Box::pin(super::gate::seed_gate_routes(
            ctx,
            stack.services,
            stack.fillers,
            Some(server),
        ))
        .await,
    );
    wirings
}

/// Take a replacement of the media server's internal certificate as far as every holder
/// lets it go: served where each holder holds its SHA-256, and the one it replaced
/// withdrawn and every holder handed the new one alone where each reaches the media server
/// with it. Nothing where no replacement is under way.
pub(super) async fn advanced(
    ctx: &Ctx,
    stack: Stack<'_>,
    server: Option<&MediaServer>,
) -> Vec<Wiring> {
    let Some(server) = server else {
        return Vec::new();
    };
    let Some(config) = server.served.confined_to.as_deref() else {
        return Vec::new();
    };
    let settled = |state| vec![Wiring::settled(connection(server), state)];
    let failed = |detail| settled(State::Failed { detail });
    let standing = match internal::replacing(ctx.seams.confined.as_ref(), config) {
        Ok(standing) if standing.under_way() => standing,
        Ok(_) => return Vec::new(),
        Err(unkept) => return failed(unkept.to_string()),
    };
    if ctx.dry_run {
        return settled(State::WouldWire {
            yours: None,
            ours: Some(rehearsed(stack, server)),
        });
    }
    let asked = match asked(ctx, server).await {
        Ok(asked) => asked,
        Err(detail) => return failed(detail),
    };
    let state = match &standing.next {
        Some(next) => {
            let replacing = (standing.current.as_ref(), next);
            serving(ctx, stack, (server, &asked), config, replacing).await
        }
        None => served_again(ctx, (server, &asked), standing.current.as_ref()).await,
    };
    match state {
        Ok(State::Wired | State::Restarted { .. } | State::AlreadyWired) => {}
        Ok(state) => return settled(state),
        Err(detail) => return failed(detail),
    }
    if let Err(detail) = reached(ctx, stack, (server, &asked), config).await {
        return failed(detail);
    }
    if let Err(unkept) = internal::withdrawn(ctx.seams.confined.as_ref(), config) {
        return failed(unkept.to_string());
    }
    let mut wirings = settled(State::Wired);
    wirings.extend(handed(ctx, stack, server).await);
    wirings
}

/// Whether every holder reaches the media server with the certificate in force: each
/// answers its SHA-256 among those it holds, the media server answers ready, and a
/// handshake to its internal port presents it. Why not otherwise, which keeps the one it
/// replaced handed to every holder.
async fn reached(
    ctx: &Ctx,
    stack: Stack<'_>,
    (server, asked): (&MediaServer, &Asked),
    config: &Path,
) -> Result<(), String> {
    let current = internal::kept(ctx.seams.confined.as_ref(), config)
        .map_err(|unkept| unkept.to_string())?
        .ok_or_else(|| unkept(server))?;
    let mut unreached: Vec<String> = unheld(ctx, stack, server, &current.fingerprint)
        .await
        .into_iter()
        .map(|holder| format!("{holder} does not yet hold its SHA-256"))
        .collect();
    if !server.ready(ctx).await || !presents(asked, &current.fingerprint).await {
        unreached.push(format!("{} does not yet present it", server.name()));
    }
    if unreached.is_empty() {
        return Ok(());
    }
    Err(format!(
        "the new certificate is not yet reached: {}. The one it replaced is still handed to \
         every holder, and the next seed withdraws it",
        unreached.join("; ")
    ))
}

/// Whether a handshake to the media server's internal port already presents the
/// certificate whose SHA-256 is `pin`, as its adapter answers serving it again.
async fn presents((path, client): &Asked, pin: &str) -> bool {
    matches!(client.serve_tls(path, pin).await, Ok(Applied::Now))
}

/// What serving the certificate in force again came to, for a replacement taken up after
/// it was put in place: the media server started again where it serves it only then.
async fn served_again(
    ctx: &Ctx,
    (server, (path, client)): (&MediaServer, &Asked),
    current: Option<&internal::Internal>,
) -> Result<State, String> {
    let current = current.ok_or_else(|| unkept(server))?;
    Ok(served(ctx, server, client.as_ref(), (path, &current.fingerprint)).await)
}

/// Why a replacement cannot go on where `server` keeps no certificate in force.
fn unkept(server: &MediaServer) -> String {
    format!("{} keeps no certificate in force", server.name())
}

/// Where the media server's container holds its certificate, and its adapter asked over
/// its contract.
type Asked = (String, Arc<dyn Fills>);

/// The media server's [`Asked`]; why not where either is missing.
async fn asked(ctx: &Ctx, server: &MediaServer) -> Result<Asked, String> {
    let path = inside(server).ok_or_else(|| {
        format!(
            "{} says nowhere its container holds its certificate",
            server.name()
        )
    })?;
    let client = server.administering(ctx).await.ok_or_else(|| {
        format!(
            "{} cannot be asked over its contract, so it goes on serving the certificate in \
             force",
            server.name()
        )
    })?;
    Ok((path, client))
}

/// What serving the replacement `next` came to, where every holder holds its SHA-256:
/// journalled, put in place of the certificate in force, and served; why not otherwise,
/// with nothing changed.
async fn serving(
    ctx: &Ctx,
    stack: Stack<'_>,
    (server, (path, client)): (&MediaServer, &Asked),
    config: &Path,
    (previous, next): (Option<&internal::Internal>, &internal::Internal),
) -> Result<State, String> {
    let unheld = unheld(ctx, stack, server, &next.fingerprint).await;
    if !unheld.is_empty() {
        return Err(format!(
            "{} not yet hold the replacement's SHA-256, so {} goes on serving the certificate \
             in force",
            said(&unheld, "does", "do"),
            server.name()
        ));
    }
    journalled(
        ctx,
        &server.served.id,
        previous.map(|kept| kept.fingerprint.clone()),
        &next.fingerprint,
    )?;
    internal::promoted(ctx.seams.confined.as_ref(), config).map_err(|unkept| unkept.to_string())?;
    Ok(served(ctx, server, client.as_ref(), (path, &next.fingerprint)).await)
}

/// The replacement of `service`'s certificate written down, by the SHA-256 of each, before
/// it is made: a replacement nothing records is not made.
fn journalled(
    ctx: &Ctx,
    service: &str,
    previous: Option<String>,
    current: &str,
) -> Result<(), String> {
    let paths = crate::app::targets::layout(ctx).ok_or_else(|| {
        "there is nowhere to journal the replacement, so the certificate in force is still \
         served"
            .to_owned()
    })?;
    let change = Change {
        at: ctx.stamp(),
        operation: OPERATION.to_owned(),
        target: service.to_owned(),
        kind: Kind::Certified {
            service: service.to_owned(),
            previous,
            current: current.to_owned(),
        },
    };
    crate::app::recover::journalled(&paths.journal(), &[change]).map_err(|failure| {
        format!(
            "the replacement could not be journalled, so the certificate in force is still \
                 served: {failure}"
        )
    })
}

/// What a replacement would do, as a rehearsal says it: who is handed the new SHA-256 and
/// that the media server may restart.
pub(crate) fn rehearsed(stack: Stack<'_>, server: &MediaServer) -> String {
    let holders = holders(stack, server);
    let handed = if holders.is_empty() {
        "no service holds a media server credential to be handed it".to_owned()
    } else {
        format!(
            "{} handed the new certificate's SHA-256 beside the old one",
            said(&holders, "is", "are")
        )
    };
    format!(
        "{handed}; {} then serves the new certificate and may restart to do so, and the old \
         SHA-256 is withdrawn once each holder reaches it with the new",
        server.name()
    )
}

/// `named`, joined, with `one` after a single one and `many` after more.
fn said(named: &[&str], one: &str, many: &str) -> String {
    let verb = if named.len() == 1 { one } else { many };
    format!("{} {verb}", named.join(" and "))
}
