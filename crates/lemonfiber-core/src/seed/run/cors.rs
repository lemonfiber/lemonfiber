//! The media server's cross-origin allow-list, held to the household front door's origin
//! and the console's, where the server takes more than one.
//!
//! Read and reconciled on every pass rather than written once, because the front
//! door's address is worked out from what this machine says about itself at the moment
//! of asking: a machine renamed, or an operator who chose another door or recorded
//! another address, moves the origin, and the list follows it the next time a pass
//! runs.
//!
//! The console is where a member plays a title in a browser, from the media server
//! behind the front door, so its origin is allowed beside the door's: as the run that
//! last served it wrote down. A server whose adapter speaks no list is held to the door's
//! origin alone.
//!
//! **Never empty.** The media server reads an empty list as every origin, so where there is no
//! front door address to name the list is left as it stands and the pass says so,
//! rather than writing the one value that opens it.

use super::Ctx;
use crate::app::targets::MediaServer;
use crate::ports::service::Failure;
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
    fillers: &crate::wiring::Fillers,
    server: Option<&MediaServer>,
) -> Option<Wiring> {
    let server = server?;
    // No administrator of this media server is recorded where lemonfiber did not set it
    // up, and a server lemonfiber holds no account on is not one it configures. A rehearsal before the first run finds none recorded
    // either, because the identity step mints it, so where that step would it says what
    // the run would write.
    let client = server.administering(ctx).await;
    let minting = server.would_mint(ctx);
    if client.is_none() && !minting {
        return None;
    }
    let Some(origin) = front_door_origin(ctx, services, fillers).await else {
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
            "Give this machine an address the household reaches, with lemonfiber settings set \
             lan-host=<address>, then run seed again."
                .to_owned(),
        );
        return Some(wiring);
    };
    let mut wanted = vec![origin.clone()];
    wanted.extend(
        crate::companion::console_origin(ctx)
            .await
            .filter(|console| *console != origin),
    );
    let Some(client) = client else {
        return Some(Wiring::settled(
            connection(server),
            State::WouldWire {
                yours: None,
                ours: Some(listed(&wanted)),
            },
        ));
    };
    let state = match client.allowed_origins().await {
        Err(failure) => crate::seed::unreached(&failure),
        Ok(held) if alike(&held, &wanted) => State::AlreadyWired,
        Ok(held) if ctx.dry_run => State::WouldWire {
            yours: Some(listed(&held)),
            ours: Some(listed(&wanted)),
        },
        Ok(held) => allowed(client.as_ref(), &held, &wanted, &origin).await,
    };
    Some(Wiring::settled(connection(server), state))
}

/// Allow `wanted`, or the front door's `origin` alone where the server takes no list;
/// the server held `held` before.
async fn allowed(
    client: &dyn lemonfiber_contract::capabilities::media::serve::Fills,
    held: &[String],
    wanted: &[String],
    origin: &str,
) -> State {
    match client.allow_origins(wanted).await {
        Ok(()) => State::Wired,
        Err(Failure::Unsupported { .. }) if held == [origin] => State::AlreadyWired,
        Err(Failure::Unsupported { .. }) => match client.allow_only(origin).await {
            Ok(()) => State::Wired,
            Err(failure) => crate::seed::unreached(&failure),
        },
        Err(failure) => crate::seed::unreached(&failure),
    }
}

/// Whether `held` names exactly the origins `wanted` does, in any order.
fn alike(held: &[String], wanted: &[String]) -> bool {
    held.len() == wanted.len() && wanted.iter().all(|one| held.contains(one))
}

/// The origin the household front door is reached at, where the door is the stack's
/// own.
///
/// The door the operator chose, or the one the stack begins at, at the address the
/// household is handed for it — the same derivation every surface that shows the door
/// uses, so the list names what the household was told.
///
/// **Over the stack's own services and its first-party parts alone.** An origin on this
/// list may read the media server from a browser, and another plugin's page is a
/// stranger's code: such a plugin's service standing as the door, or named as it, is
/// never given that, and the list stays on the door the stack and its first-party parts
/// make. Nothing a stranger's service does as a door needs to read the media server from
/// a browser, so nothing is lost by it.
async fn front_door_origin(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
    fillers: &crate::wiring::Fillers,
) -> Option<String> {
    let first_party: Vec<crate::plugin::Installed> = crate::app::plugins::read(ctx)
        .map(|register| {
            register
                .installed()
                .iter()
                .filter(|one| {
                    crate::plugin::first_party::holds(crate::plugin::first_party::EMBEDDED, one)
                })
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    let candidates = crate::door::candidates(services, &first_party, fillers);
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
