//! The certificate the media server presents at its internal TLS port, as a line of the
//! inventory: listed by the SHA-256 of the one in force, never its key, and replaced on
//! the operator's ask the way every replacement of it is made, read first and carried out
//! on the offer that reading named.
//!
//! A certificate kept that cannot be read is said to be so, and is never taken for one
//! that is not there: nothing is replaced over it, and a replacement whose outcome cannot
//! be read back is never reported as landed.

use crate::app::targets::MediaServer;
use crate::app::Ctx;
use crate::certificate::internal::{self, Replacing};
use crate::credential::{Held, Origin, Propagation, Rotation, Settled, State};
use crate::error::codes::cred::REPLACEMENT_MOVED;
use crate::error::{Problem, Remedy, State as Standing};
use crate::seed::run::replacing::{self, Stack};

/// What the inventory calls it.
pub(crate) const NAME: &str = "Media server internal certificate";

/// The name it is asked for by on every surface.
pub(crate) const SETTING: &str = crate::app::Asking::INTERNAL_CERTIFICATE;

/// The parts a replacement is offered over, as a refusal names them.
const OFFERED: [&str; 3] = [
    "the certificate in force",
    "who is handed the new one",
    "the media server",
];

/// The media server whose certificate this is, where lemonfiber writes one for it.
fn server(fillers: &crate::wiring::Fillers) -> Option<MediaServer> {
    MediaServer::of(fillers).filter(|server| server.served.confined_to.is_some())
}

/// Every certificate kept for `server`, or why they cannot be read.
fn standing(ctx: &Ctx, server: &MediaServer) -> Result<Replacing, String> {
    let config = server
        .served
        .confined_to
        .as_deref()
        .unwrap_or(std::path::Path::new(""));
    internal::replacing(ctx.seams.confined.as_ref(), config).map_err(|unkept| unkept.to_string())
}

/// The certificate's line in the inventory, where the stack's media server is one
/// lemonfiber writes a certificate for.
pub(crate) fn held(ctx: &Ctx, stack: Stack<'_>) -> Option<Held> {
    let server = server(stack.fillers)?;
    let location = server
        .served
        .confined_to
        .as_deref()
        .map(|config| internal::path(config).display().to_string())
        .unwrap_or_default();
    let read = standing(ctx, &server);
    let (state, advisory) = match &read {
        Err(why) => (
            State::Invalid,
            Some(format!(
                "What is kept cannot be read, so nothing is replaced over it: {why}"
            )),
        ),
        Ok(kept) if kept.current.is_none() => (
            State::Absent,
            Some(format!(
                "{} has no internal certificate yet: lemonfiber makes one before the stack \
                 starts.",
                server.name()
            )),
        ),
        Ok(kept) if kept.under_way() => (
            State::Rotating,
            Some(format!(
                "A replacement is under way: {}",
                replacing::rehearsed(stack, &server)
            )),
        ),
        Ok(_) => (State::Active, None),
    };
    Some(Held {
        name: NAME.to_owned(),
        setting: SETTING.to_owned(),
        consumers: replacing::holders(stack, &server)
            .into_iter()
            .map(str::to_owned)
            .collect(),
        location,
        origin: Origin::Lemonfiber,
        from: server.served.origin.clone(),
        fingerprint: read
            .ok()
            .and_then(|kept| kept.current)
            .map(|current| current.fingerprint),
        advisory,
        state,
    })
}

/// Replace the certificate, or say what replacing it would do where no offer is answered
/// or this is a rehearsal.
///
/// # Errors
///
/// [`REPLACEMENT_MOVED`] where the offer answered is not the one a fresh reading builds.
pub(crate) async fn rotate(
    ctx: &Ctx,
    held: &Held,
    stack: Stack<'_>,
    agreement: Option<&str>,
) -> Result<Rotation, Box<Problem>> {
    let Some(server) = server(stack.fillers) else {
        return Ok(unproven(
            held,
            "the stack runs no media server lemonfiber writes a certificate for".to_owned(),
        ));
    };
    let before = match standing(ctx, &server) {
        Ok(before) => before,
        Err(why) => return Ok(unproven(held, why)),
    };
    let in_force = before
        .current
        .as_ref()
        .map(|current| current.fingerprint.clone())
        .unwrap_or_default();
    let offer = crate::agreement::parted(&[
        &[&in_force],
        &[&replacing::holders(stack, &server).join("\n")],
        &[server.id()],
    ]);
    let Some(answered) = agreement.filter(|_| !ctx.dry_run) else {
        return Ok(Rotation::would(
            &held.name,
            &replacing::rehearsed(stack, &server),
            &held.location,
            Vec::new(),
        )
        .offering(&offer));
    };
    let moved = crate::agreement::differs(answered, &offer, &OFFERED);
    if !moved.is_empty() {
        return Err(Box::new(offer_moved(&moved, &offer)));
    }
    let wirings = replacing::replaced(ctx, stack, &server).await;
    Ok(settled(ctx, held, stack, &server, &wirings))
}

/// What a replacement that was carried out came to, read back from where the
/// certificates stand after it: landed only where the one it replaced is withdrawn, which
/// is only once every holder answered the new one's SHA-256 and the media server presented
/// it.
fn settled(
    ctx: &Ctx,
    held: &Held,
    stack: Stack<'_>,
    server: &MediaServer,
    wirings: &[crate::seed::Wiring],
) -> Rotation {
    let said = wirings
        .iter()
        .filter_map(|wiring| match &wiring.state {
            crate::seed::State::Failed { detail } => Some(detail.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("; ");
    let after = match standing(ctx, server) {
        Ok(after) => after,
        Err(why) => return unproven(held, why),
    };
    match (&after.next, &after.previous, &after.current) {
        (Some(_), ..) | (_, _, None) => return unproven(held, said),
        (None, Some(_), Some(_)) => {
            return Rotation::stopped(&held.name, Settled::ReplacedUnproven { detail: said });
        }
        (None, None, Some(_)) => {}
    }
    Rotation::landed(
        &held.name,
        &format!(
            "{} serves the new certificate, and the one it replaced is withdrawn",
            server.name()
        ),
        replacing::holders(stack, server)
            .into_iter()
            .map(Propagation::updated)
            .collect(),
    )
}

/// A replacement that did not get as far as serving the new certificate, for `why`.
fn unproven(held: &Held, why: String) -> Rotation {
    Rotation::stopped(&held.name, Settled::Unproven { detail: why })
}

/// The refusal of an answer to a replacement whose reading has since moved.
fn offer_moved(moved: &[&str], offer: &str) -> Problem {
    Problem::new(
        REPLACEMENT_MOVED,
        "the certificate replacement offered is not the one that stands now",
        format!(
            "Since it was read, {} changed, so nothing was replaced.",
            moved.join(" and ")
        ),
        Remedy::new("Read the replacement again, and answer the offer it gives now")
            .with_detail(format!("the offer that stands now is {offer}")),
    )
    .in_state(Standing::Actionable)
}
