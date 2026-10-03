//! Seerr's Jellyfin connection: where the request service reaches the media server for
//! everything after a sign-in, held at the request gate's Jellyfin route under a token
//! the gate accepts.
//!
//! The sign-in that sets the request service up mints it no key: the gate answers that
//! mint itself, with a placeholder that opens nothing, because it holds tokens only as
//! hashes and could not hand over a real one. So the token is written to the request
//! service here, straight after, which it proves through the gate before it keeps it.

use std::path::Path;

use super::tokens::{through_the_gate, Kept, Presented};
use super::Ctx;
use crate::ports::service::Requests;
use crate::seed::{State, Wiring};

/// What the report calls this connection.
const CONNECTION: &str = "Seerr's Jellyfin connection";

/// Hold the request service to reaching the media server at the gate's `route`, under
/// a token the gate accepts there.
pub(super) async fn seed_media_server_link(
    ctx: &Ctx,
    seerr: &dyn Requests,
    route: &str,
    project: &Path,
) -> Wiring {
    let at = through_the_gate(route);
    let link = match seerr.media_server_link().await {
        Ok(link) => link,
        Err(failure) => return settled(crate::seed::unreached(&failure)),
    };
    let kept = Kept::read(ctx, project).await;
    if link.at == at && kept.accepts(route, &link.key) {
        return settled(State::AlreadyWired);
    }
    if ctx.dry_run {
        // A request service pointed nowhere yet has no address to say it moves from.
        let moving = link.at != at && !link.at.host.is_empty();
        return settled(State::WouldWire {
            yours: moving.then(|| crate::seed::reached_at(&link.at)),
            ours: Some(crate::seed::reached_at(&at)),
        });
    }
    let Some(token) = crate::secret::generate(ctx.seams.random.as_ref()) else {
        return settled(State::Failed {
            detail: "no randomness was available to generate a token".to_owned(),
        });
    };
    let presented = [Presented {
        route: route.to_owned(),
        token: token.clone(),
    }];
    let beside = match kept.beside(&presented) {
        Ok(beside) => beside,
        Err(reason) => {
            return settled(State::Failed {
                detail: kept.unwritten(&reason),
            })
        }
    };
    if let Err(failure) = seerr.link_media_server(&at, &token).await {
        return settled(State::Failed {
            detail: format!("Seerr did not take the gate's Jellyfin token: {failure}"),
        });
    }
    match kept.only(&beside, &presented, &[true]) {
        Ok(()) => settled(State::Wired),
        Err(reason) => settled(State::Failed {
            detail: kept.unwritten(&reason),
        }),
    }
}

/// What a rehearsal says where the request service is not set up yet: it would be
/// pointed at the gate once it is.
pub(super) fn would_link(route: &str) -> Wiring {
    settled(State::WouldWire {
        yours: None,
        ours: Some(crate::seed::reached_at(&through_the_gate(route))),
    })
}

/// This connection, resting in `state`.
fn settled(state: State) -> Wiring {
    Wiring::settled(CONNECTION.to_owned(), state)
}
