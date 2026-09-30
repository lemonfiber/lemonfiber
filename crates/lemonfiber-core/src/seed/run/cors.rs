//! Jellyfin's cross-origin allow-list, held to the household front door's origin.
//!
//! Read and reconciled on every pass rather than written once, because the front
//! door's address is worked out from what this machine says about itself at the moment
//! of asking: a machine renamed, or an operator who chose another door or recorded
//! another address, moves the origin, and the list follows it the next time a pass
//! runs.
//!
//! **Never empty.** Jellyfin reads an empty list as every origin, so where there is no
//! front door address to name the list is left as it stands and the pass says so,
//! rather than writing the one value that opens it.

use super::Ctx;
use crate::seed::{State, Wiring};

/// What the report calls this connection.
const CONNECTION: &str = "Jellyfin's cross-origin reads, from the front door only";

/// Hold Jellyfin's allow-list to the front door's origin, where the stack has Jellyfin.
pub(super) async fn seed_cors(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
) -> Option<Wiring> {
    let jellyfin = super::identity::jellyfin_service(services)?;
    // No administrator of this media server is recorded where lemonfiber did not set it
    // up — a stack with no request service, or one whose identity source is something
    // else — and a server lemonfiber holds no account on is not one it configures. A
    // rehearsal before the first run finds none recorded either, because the identity
    // step mints it, so where that step would it says what the run would write.
    let recorded = super::identity::recorded_jellyfin_password(ctx);
    let minting = ctx.dry_run && super::identity::seerr_service(services).is_some();
    if recorded.is_none() && !minting {
        return None;
    }
    let Some(origin) = front_door_origin(ctx, services).await else {
        let mut wiring = Wiring::settled(
            CONNECTION.to_owned(),
            State::Skipped {
                reason: "there is no front door address to name, and an empty list would \
                         let every origin read Jellyfin"
                    .to_owned(),
            },
        );
        wiring.escalate(
            "Jellyfin still answers a browser reading it from any origin.".to_owned(),
            "Give this machine an address the household reaches, with `lemonfiber config set \
             HOMEPAGE_VAR_LAN_HOST <address>`, then run seed again."
                .to_owned(),
        );
        return Some(wiring);
    };
    let Some(password) = recorded else {
        return Some(Wiring::settled(
            CONNECTION.to_owned(),
            State::WouldWire {
                yours: None,
                ours: Some(origin),
            },
        ));
    };
    let client = crate::jellyfin::Jellyfin::authenticated(
        ctx.seams.http.clone(),
        &jellyfin.loopback,
        "jellyfin",
        crate::config::JELLYFIN_ADMIN_USER,
        password,
    );
    let state = match client.cors_hosts().await {
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
    Some(Wiring::settled(CONNECTION.to_owned(), state))
}

/// The origin the household front door is reached at, where there is one.
///
/// The door the operator chose, or the one the stack begins at, at the address the
/// household is handed for it — the same derivation every surface that shows the door
/// uses, so the list names what the household was told.
async fn front_door_origin(ctx: &Ctx, services: &[lemonfiber_manifest::Service]) -> Option<String> {
    let (_, door) = crate::door::chosen(services, ctx.settings.front_door.as_deref());
    let (_, service) = door?;
    crate::app::invite::household_address(ctx, service.port?)
        .await
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
