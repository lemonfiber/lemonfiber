//! Reading what adding or dropping a way of downloading would come to.
//!
//! The stack says what a protocol runs, the walk says what it asks for, and the
//! download clients say what is still coming down under it. All three are read
//! here and decided in [`crate::reconfigure::protocols`].

use crate::dashboard::Protocol;
use crate::reconfigure::protocols::{changed, kept, opened, reduces, stopped};
use crate::reconfigure::{Active, Findings};

use super::Ctx;

/// Fill in what changing `key` to `value` does to the ways of downloading.
///
/// Nothing at all for a setting that decides neither, and nothing for a change
/// that leaves them where they are — a report about a protocol nobody touched
/// would be a warning nobody caused.
pub(super) async fn opening(ctx: &Ctx, found: &mut Findings, key: &str, value: &str) {
    let before = ctx.settings.protocols;
    let Some(after) = changed(before, key, value) else {
        return;
    };
    if after == before {
        return;
    }
    found.opens = opened(before, after);
    found.keeps = kept(before, after, ctx.settings.data_root.as_deref());
    if let Ok(manifest) = ctx.stack.checked_manifest(ctx.today()) {
        found.stops = stopped(&manifest, before, after);
    }
    if reduces(before, after) {
        found.active = interrupted(ctx, before, after).await;
    }
}

/// Whether this change is one waiting could carry through: a reduction, and so a
/// change with work under it that finishing would clear.
///
/// Read by the write path before it sits down to wait, so `--wait` on a change
/// that takes nothing away waits for nothing rather than for every download on the
/// machine.
pub(in crate::app) fn waits_for_downloads(ctx: &Ctx, key: &str, value: &str) -> bool {
    changed(ctx.settings.protocols, key, value)
        .is_some_and(|after| reduces(ctx.settings.protocols, after))
}

/// What is still coming down over a protocol this change takes away.
///
/// Narrowed to the protocols actually being dropped: a Usenet download is not
/// interrupted by switching torrents off, and naming it would be reporting work
/// that is in no danger. The clients are asked through the same read a teardown
/// makes, so "what is in flight" has one answer on this machine.
async fn interrupted(
    ctx: &Ctx,
    before: crate::config::Protocols,
    after: crate::config::Protocols,
) -> Vec<Active> {
    let dropped = |protocol: Protocol| match protocol {
        Protocol::Usenet => before.usenet && !after.usenet,
        Protocol::Torrent => before.torrent && !after.torrent,
    };
    super::super::engine::in_flight(ctx, &[])
        .await
        .into_iter()
        .filter(|download| dropped(download.protocol))
        .map(|download| Active {
            protocol: match download.protocol {
                Protocol::Usenet => "usenet".to_owned(),
                Protocol::Torrent => "torrent".to_owned(),
            },
            name: download.name,
            progress: download.progress,
        })
        .collect()
}
