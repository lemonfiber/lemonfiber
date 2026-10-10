//! The request service's connection to the media server: where it reaches the server for
//! everything after a sign-in, held at the request gate's route to the server under a
//! token the gate accepts.
//!
//! The sign-in that sets the request service up mints it no key: the gate answers that
//! mint itself, with a placeholder that opens nothing, because it holds tokens only as
//! hashes and could not hand over a real one. So the token is written to the request
//! service here, straight after, which it proves through the gate before it keeps it.

use std::path::Path;

use super::tokens::{through_the_gate, Kept, Presented};
use super::Ctx;
use crate::app::targets::MediaServer;
use crate::ports::service::Requests;
use crate::seed::{State, Wiring};
use crate::wiring::Filler;

/// Hold `requests`, the request service `asker` is, to reaching `server` at the gate's
/// route to it, under a token the gate accepts there.
pub(super) async fn seed_media_server_link(
    ctx: &Ctx,
    requests: &dyn Requests,
    asker: &Filler,
    server: &MediaServer,
    project: &Path,
) -> Wiring {
    let rests = |state| settled(asker, server, state);
    let route = server.id();
    let at = through_the_gate(route);
    let link = match requests.media_server_link().await {
        Ok(link) => link,
        Err(failure) => return rests(crate::seed::unreached(&failure)),
    };
    let kept = Kept::read(ctx, project).await;
    if link.at == at && kept.accepts(route, &link.key) {
        return rests(State::AlreadyWired);
    }
    if ctx.dry_run {
        // A request service pointed nowhere yet has no address to say it moves from.
        let moving = link.at != at && !link.at.host.is_empty();
        return rests(State::WouldWire {
            yours: moving.then(|| crate::seed::reached_at(&link.at)),
            ours: Some(crate::seed::reached_at(&at)),
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
    let beside = match kept.beside(&presented) {
        Ok(beside) => beside,
        Err(reason) => {
            return rests(State::Failed {
                detail: kept.unwritten(&reason),
            })
        }
    };
    if let Err(failure) = requests.link_media_server(&at, &token).await {
        return rests(State::Failed {
            detail: format!(
                "{} did not take the gate's {} token: {failure}",
                asker.name,
                server.name()
            ),
        });
    }
    match kept.only(&beside, &presented, &[true]) {
        Ok(()) => rests(State::Wired),
        Err(reason) => rests(State::Failed {
            detail: kept.unwritten(&reason),
        }),
    }
}

/// What a rehearsal says where the request service is not set up yet: it would be
/// pointed at the gate once it is.
pub(super) fn would_link(asker: &Filler, server: &MediaServer) -> Wiring {
    settled(
        asker,
        server,
        State::WouldWire {
            yours: None,
            ours: Some(crate::seed::reached_at(&through_the_gate(server.id()))),
        },
    )
}

/// The connection from the request service `asker` to `server`, resting in `state`.
fn settled(asker: &Filler, server: &MediaServer, state: State) -> Wiring {
    Wiring::settled(
        format!("{}'s {} connection", asker.name, server.name()),
        state,
    )
}
