//! The asks kept beside other members' requests waiting for approval, settled once those
//! are ruled on: fulfilled where they were approved, and asked for as the member's own
//! where they were declined or are gone.

use lemonfiber_contract::capabilities::request::intake::Fills;

use super::ask::{filed, Filer, Named};
use crate::app::Ctx;
use crate::config::paths::ASKED_AS_OWNER;
use crate::household::run::as_owner;
use crate::household::run::as_owner::beside::{self, Beside, CameTo, Settled};
use crate::ports::service::{Member, Wish};

/// What a member reads where their ask could not be asked for again once the request it
/// waited beside was turned down. Any reason kept for that request was written to the
/// member who made it, so it is never passed on.
pub(super) const NOT_ASKED_AGAIN: &str =
    "It couldn't be asked for, so ask whoever runs the house about it.";

/// Settle each ask kept beside other members' requests that `requests` no longer holds
/// waiting for approval. Under a rehearsal nothing is asked for or written.
pub(crate) async fn settled(ctx: &Ctx, requests: &dyn Fills) {
    if ctx.dry_run {
        return;
    }
    let Some(path) = crate::app::targets::beside_env(ctx, ASKED_AS_OWNER) else {
        return;
    };
    let waiting: Vec<Beside> = as_owner::read(&path)
        .map(|kept| kept.beside.into_iter().filter(Beside::waits).collect())
        .unwrap_or_default();
    if waiting.is_empty() {
        return;
    }
    let Ok(asked) = requests.requests().await else {
        return;
    };
    for ask in waiting {
        let settled = match ask.come_to(&asked) {
            CameTo::Waiting => continue,
            CameTo::Approved => Settled::Fulfilled,
            CameTo::Declined => asked_again(ctx, requests, &path, &ask).await,
        };
        if beside::settled(&path, &ask, &settled, ctx.stamp()).is_err() {
            return;
        }
    }
}

/// Ask for `ask` as its member's own, as any ask of theirs is asked, and say what came of
/// it: declined as a fresh ask is where the house has no room, and declined saying to ask
/// whoever runs the house where it could not be asked for.
async fn asked_again(
    ctx: &Ctx,
    requests: &dyn Fills,
    path: &std::path::Path,
    ask: &Beside,
) -> Settled {
    let member = Member {
        id: ask.member.clone(),
        name: ask.name.clone(),
        ..Member::default()
    };
    let kept = as_owner::read(path).unwrap_or_default();
    let declined = || Settled::Declined(NOT_ASKED_AGAIN.to_owned());
    let Ok(theirs) =
        crate::household::run::allowance_of(requests, &member, ctx.stamp(), &kept).await
    else {
        return declined();
    };
    if crate::space::run::admits(ctx).await.is_err() {
        return Settled::Declined(super::NO_ROOM_SAID.to_owned());
    }
    let filer = Filer {
        requests,
        member: &member,
        kept: path,
    };
    let wish = Wish {
        id: ask.title.clone(),
        kind: ask.kind,
        seasons: ask.seasons.clone(),
    };
    let named = Named {
        called: ask.called.as_deref(),
        year: ask.year,
    };
    match filed(ctx, &filer, &theirs, (&wish, named), &ask.seasons).await {
        Ok(_) => Settled::AskedAgain,
        Err(_) => declined(),
    }
}
