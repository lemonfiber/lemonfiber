//! The address the media server trusts to name the client, held to the guarded front
//! door's.
//!
//! Behind the door every request reaches the media server from the door, so unless the
//! door is trusted to say who the client was, every client looks like one on the
//! household's own network. The server reads the list when it starts, so a pass that
//! changes it starts the server again.

use std::path::Path;

use super::Ctx;
use crate::app::targets::MediaServer;
use crate::jellyfin::Jellyfin;
use crate::ports::service::{Failure, Fronted as _};
use crate::seed::{State, Wiring};

/// What the report calls this connection.
fn connection(server: &MediaServer) -> String {
    format!(
        "{} told which clients are not on the household's network",
        server.name()
    )
}

/// Hold the media server to trusting the door alone, where the stack runs a door in front
/// of a media server lemonfiber set up.
pub(super) async fn seed_proxies(
    ctx: &Ctx,
    project: Option<&Path>,
    server: Option<&MediaServer>,
) -> Option<Wiring> {
    let server = server?;
    let door = crate::screening::door::upstream(project?)?;
    let Some(password) = server.recorded_password(ctx) else {
        return ctx.dry_run.then(|| {
            Wiring::settled(
                connection(server),
                State::WouldWire {
                    yours: None,
                    ours: Some(door.clone()),
                },
            )
        });
    };
    let client = server.signed_in(ctx, password);
    let state = match client.known_proxies().await {
        Err(failure) => crate::seed::unreached(&failure),
        Ok(held) if held == [door.as_str()] => State::AlreadyWired,
        Ok(held) if ctx.dry_run => State::WouldWire {
            yours: Some(held.join(", ")),
            ours: Some(door),
        },
        Ok(_) => match trusted(&client, &door).await {
            Ok(()) => State::Wired,
            Err(failure) => crate::seed::unreached(&failure),
        },
    };
    Some(Wiring::settled(connection(server), state))
}

/// Trust `door` alone, start the server again so it reads what was written, and wait
/// until it answers again — every pass after this one talks to it, and a server that is
/// still starting would read as one that refuses.
async fn trusted(client: &Jellyfin, door: &str) -> Result<(), Failure> {
    client.trust_only(door).await?;
    client.restart().await?;
    crate::patience::RESTART
        .until(|| client.accepts(), Result::is_ok)
        .await
}
