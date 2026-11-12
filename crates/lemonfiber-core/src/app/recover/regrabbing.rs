//! A dropped stuck download grabbed again, as putting its drop back asks.

use crate::journal::Undo;

use super::super::Ctx;

/// Ask the curator `undo` names to grab the release it dropped again, the kind and title
/// it filed it under and its fingerprint being `asked`.
pub(super) async fn regrabbed(
    ctx: &Ctx,
    fillers: &crate::wiring::Fillers,
    undo: &Undo,
    (kind, title, release): (crate::ports::media::TitleKind, i64, &str),
    reached: &mut super::Reached,
) {
    let curator = super::super::targets::target_named(ctx, fillers, &undo.target)
        .await
        .and_then(|target| target.asked);
    let grabbed = match curator {
        Some(curator) => curator.grab(kind, title, release).await.ok(),
        None => None,
    };
    match grabbed {
        Some(crate::ports::service::Grabbed::Grabbed) => reached.put_back.push(undo.clone()),
        Some(crate::ports::service::Grabbed::Gone) => reached.unreached.push(format!(
            "the release {} dropped, which no indexer offers any more",
            undo.target
        )),
        None => reached
            .unreached
            .push(format!("the release {} dropped", undo.target)),
    }
}
