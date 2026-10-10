//! The media server keys lemonfiber mints for the services it builds, each filed under the
//! name of the service that holds it.
//!
//! What every such key has in common is here: minting one that is a single word,
//! revoking what nothing holds, and saying how many are filed under a name. Where each
//! is handed over is the service's own step.

use super::Ctx;
use crate::ports::service::{AppKeys, Failure};
use crate::seed::{State, Wiring};

/// Mint a key filed under `app`, refusing one that is not a single word.
///
/// A key that is not one word is revoked again at once, so nothing is left on the
/// server that nothing could hold.
pub(super) async fn mint(client: &dyn AppKeys, app: &str) -> Result<String, State> {
    let key = client
        .mint(app)
        .await
        .map_err(|failure| crate::seed::unreached(&failure))?;
    if key.split_whitespace().count() == 1 {
        return Ok(key);
    }
    let _ = client.revoke(&key).await;
    Err(State::Failed {
        detail: "the media server minted a key that is not one word, so it was revoked again"
            .to_owned(),
    })
}

/// Revoke every key in `keys`, stopping at the first the server refuses.
pub(super) async fn revoked<'a>(
    client: &dyn AppKeys,
    keys: impl IntoIterator<Item = &'a String>,
) -> Result<(), Failure> {
    for key in keys {
        client.revoke(key).await?;
    }
    Ok(())
}

/// What revoking `keys` comes to, as a state: wired, or the failure that stopped it.
pub(super) async fn revoking<'a>(
    client: &dyn AppKeys,
    keys: impl IntoIterator<Item = &'a String>,
) -> State {
    match revoked(client, keys).await {
        Ok(()) => State::Wired,
        Err(failure) => crate::seed::unreached(&failure),
    }
}

/// How many keys are filed under `app`, said as a reader would.
pub(super) fn count(app: &str, keys: usize) -> String {
    match keys {
        0 => format!("no key filed as {app}"),
        1 => format!("one key filed as {app}"),
        many => format!("{many} keys filed as {app}"),
    }
}

/// The stack no longer runs the service whose keys are filed under `app`: revoke
/// every one, reported as `connection`. Nothing where none is filed.
pub(super) async fn retired(
    ctx: &Ctx,
    client: &dyn AppKeys,
    app: &str,
    connection: &str,
    filed: &[String],
) -> Option<Wiring> {
    if filed.is_empty() {
        return None;
    }
    let state = if ctx.dry_run {
        State::WouldWire {
            yours: Some(count(app, filed.len())),
            ours: Some(count(app, 0)),
        }
    } else {
        revoking(client, filed).await
    };
    Some(Wiring::settled(connection.to_owned(), state))
}
