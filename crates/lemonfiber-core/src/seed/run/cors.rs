//! The media server's cross-origin allow-list, held to the household front door's origin.
//!
//! Read and reconciled on every pass rather than written once, because the front
//! door's address is worked out from what this machine says about itself at the moment
//! of asking: a machine renamed, or an operator who chose another door or recorded
//! another address, moves the origin, and the list follows it the next time a pass
//! runs.
//!
//! **Never empty.** The media server reads an empty list as every origin, so where there is no
//! front door address to name the list is left as it stands and the pass says so,
//! rather than writing the one value that opens it.

use super::Ctx;
use crate::app::targets::MediaServer;
use crate::seed::{State, Wiring};

/// What the report calls this connection.
fn connection(server: &MediaServer) -> String {
    format!(
        "{}'s cross-origin reads, from the front door only",
        server.name()
    )
}

/// Hold the media server's allow-list to the front door's origin, where the stack has a
/// media server.
pub(super) async fn seed_cors(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
    server: Option<&MediaServer>,
) -> Option<Wiring> {
    let server = server?;
    // No administrator of this media server is recorded where lemonfiber did not set it
    // up — one no request service asks for — and a server lemonfiber holds no account on
    // is not one it configures. A rehearsal before the first run finds none recorded
    // either, because the identity step mints it, so where that step would it says what
    // the run would write.
    let client = server.administering(ctx).await;
    let minting = server.would_mint(ctx);
    if client.is_none() && !minting {
        return None;
    }
    let Some(origin) = front_door_origin(ctx, services).await else {
        let mut wiring = Wiring::settled(
            connection(server),
            State::Skipped {
                reason: format!(
                    "there is no front door address to name, and an empty list would let \
                     every origin read {}",
                    server.name()
                ),
            },
        );
        wiring.escalate(
            format!(
                "{} still answers a browser reading it from any origin.",
                server.name()
            ),
            "Give this machine an address the household reaches, with `lemonfiber config set \
             HOMEPAGE_VAR_LAN_HOST <address>`, then run seed again."
                .to_owned(),
        );
        return Some(wiring);
    };
    let Some(client) = client else {
        return Some(Wiring::settled(
            connection(server),
            State::WouldWire {
                yours: None,
                ours: Some(origin),
            },
        ));
    };
    let state = match client.allowed_origins().await {
        Err(failure) => crate::seed::unreached(&failure),
        Ok(held) if held == [origin.as_str()] => State::AlreadyWired,
        Ok(held) if ctx.dry_run => State::WouldWire {
            yours: Some(listed(&held)),
            ours: Some(origin),
        },
        Ok(_) => match client.allow_only(&origin).await {
            Ok(()) => State::Wired,
            Err(failure) => crate::seed::unreached(&failure),
        },
    };
    Some(Wiring::settled(connection(server), state))
}

/// The origin the household front door is reached at, where the door is the stack's
/// own.
///
/// The door the operator chose, or the one the stack begins at, at the address the
/// household is handed for it — the same derivation every surface that shows the door
/// uses, so the list names what the household was told.
///
/// **Over the stack's own services alone.** An origin on this list may read the media
/// server from a browser, and a plugin's page is a stranger's code: a plugin's service
/// standing as the door, or named as it, is never given that, and the list stays on the
/// door the stack's own services make. Nothing a plugin's service does as a door needs
/// to read the media server from a browser, so nothing is lost by it.
async fn front_door_origin(ctx: &Ctx, services: &[lemonfiber_manifest::Service]) -> Option<String> {
    let candidates = crate::door::candidates(services, &[]);
    let (_, door) = crate::door::chosen(&candidates, ctx.settings.front_door.as_deref());
    let (_, door) = door?;
    let named = ctx.site.name().await;
    crate::door::run::reached(door, &crate::door::run::place(ctx, named.as_deref()))
        .map(|address| address.url)
}

/// What an allow-list names, said as a reader would: every origin where it names none.
fn listed(held: &[String]) -> String {
    if held.is_empty() {
        "every origin".to_owned()
    } else {
        held.join(", ")
    }
}
