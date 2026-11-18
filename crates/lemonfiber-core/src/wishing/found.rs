//! The token a member's search hands each title it offers, and a title it vouches for
//! looked up again before it is called unknown.
//!
//! A title the member's own search just found that then comes back unknown is far more
//! likely a slow lookup than a title that is not there, so the ask that hands its token
//! back looks it up again and, still unknown, says to try again. The token is signed with
//! the secret store's key and binds the member, the kind, the id and when it lapses, so
//! nothing about anybody's searches is written down and nobody can vouch for a title their
//! search never offered.

use std::future::Future;
use std::time::Duration;

use super::{no_such_title, not_working, refused};
use crate::app::Ctx;
use crate::error::codes::wish::NOT_CHECKED;
use crate::error::Problem;
use crate::model::Askable;
use crate::patience::Patience;
use crate::ports::media::TitleKind;

/// What a token is signed for, so no other signature the store's key makes is one.
const PURPOSE: &str = "lemonfiber found by a member's search";

/// How long a token vouches for a title after the search that offered it, in seconds.
const VOUCHES_FOR: u64 = 3_600;

/// A title vouched for coming back unknown is looked up again this often, after a pause.
const AGAIN: Patience = Patience {
    between: Duration::from_secs(2),
    asks: 2,
};

/// Give each of `titles` the token vouching that `member`'s search offered it, or none
/// where the secret store has no key to sign with.
pub(super) fn vouched(ctx: &Ctx, member: &str, titles: &mut [Askable]) {
    let Some(secrets) = crate::settings::secrets_at(ctx)
        .and_then(|path| crate::settings::keyed_at(&path, ctx.seams.random.as_ref()).ok())
    else {
        return;
    };
    let lapses = ctx.stamp().later(VOUCHES_FOR).seconds();
    for title in titles {
        title.found = secrets
            .signed(PURPOSE, &message(member, title.kind, &title.id, lapses))
            .map(|signed| format!("{lapses}.{signed}"));
    }
}

/// Whether `token` vouches that `member`'s search offered the title `id` of `kind` and has
/// not lapsed.
pub(super) fn vouches(
    ctx: &Ctx,
    token: Option<&str>,
    member: &str,
    kind: TitleKind,
    id: &str,
) -> bool {
    let Some((lapses, signed)) = token.and_then(|token| token.split_once('.')) else {
        return false;
    };
    let Ok(at) = lapses.parse::<u64>() else {
        return false;
    };
    if at <= ctx.stamp().seconds() {
        return false;
    }
    crate::settings::secrets_at(ctx)
        .and_then(|path| crate::settings::Secrets::at(&path).ok())
        .and_then(|secrets| secrets.signed(PURPOSE, &message(member, kind, id, at)))
        .is_some_and(|expected| crate::keys::same(&expected, signed))
}

/// What `first` looked up comes to: what it found, or where it found nothing and the title
/// is `vouched` for, what looking it up again comes to, still nothing being a try-again
/// rather than no such title.
///
/// # Errors
///
/// No such title where nothing was found and nothing vouches for it, the try-again where
/// it is vouched for and still unknown, and asking not working where a lookup failed.
pub(super) async fn looked_up<T, E, Looking, Looked>(
    vouched: bool,
    first: Result<Option<T>, E>,
    again: Looking,
) -> Result<T, Box<Problem>>
where
    Looking: FnMut() -> Looked,
    Looked: Future<Output = Result<Option<T>, E>>,
{
    let answer = match first {
        Ok(None) if vouched => {
            AGAIN
                .until(again, |looked| !matches!(looked, Ok(None)))
                .await
        }
        other => other,
    };
    match answer {
        Ok(Some(found)) => Ok(found),
        Ok(None) if vouched => Err(refused(NOT_CHECKED, "It couldn't be checked just now.")),
        Ok(None) => Err(no_such_title()),
        Err(_) => Err(not_working()),
    }
}

/// What a token signs: the member, the kind, the id and when it lapses, written so no two
/// of them read as one.
fn message(member: &str, kind: TitleKind, id: &str, lapses: u64) -> String {
    serde_json::json!([member, kind.media_type(), id, lapses]).to_string()
}

#[cfg(test)]
mod tests;
