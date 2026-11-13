//! Each live stream the core opened for a member, kept owner-only until it is closed.
//!
//! The media server keeps a live stream open until it is told to close it, so the core
//! keeps what closes each one: who it was opened for, the channel, the id it is closed
//! by, the paths it is served at with the live streams they name, the moment its passes
//! are written to lapse at and when they lapse now. No pass is kept. One whose pass lapsed unrenewed is closed by
//! [`lapsed`], every minute while the web API is served and whenever the household is
//! read, and a removed member's are closed at once.

use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::app::Ctx;
use crate::config::paths::TUNED as KEPT;
use crate::ports::service::Tuning;

/// One live stream the core opened.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Open {
    /// The member it was opened for, by the id the media server files them under.
    pub(crate) member: String,
    /// The channel it is of.
    pub(crate) channel: String,
    /// The id the media server closes it by.
    pub(crate) closes_by: String,
    /// The path and query at the upstream of the tuner's own stream, where a live-stream
    /// path covers it.
    pub(crate) direct: Option<String>,
    /// The path and query at the upstream of its transcoded playlist, where a live-stream
    /// path covers it.
    pub(crate) transcoded: Option<String>,
    /// Every live stream those paths name.
    pub(crate) streams: Vec<String>,
    /// The moment its passes are written to lapse at, in seconds since the Unix epoch: a
    /// renewal answers the same passes, held good for longer.
    pub(crate) issued: u64,
    /// When its passes lapse now, in seconds since the Unix epoch.
    pub(crate) lapses: u64,
}

/// Held over every change to what is kept, so a sweep and a request in this process
/// never write over each other.
static WRITING: Mutex<()> = Mutex::new(());

/// Where what is kept is written, or nothing where there is nowhere to keep it.
pub(crate) fn place(ctx: &Ctx) -> Option<PathBuf> {
    crate::app::targets::beside_env(ctx, KEPT)
}

/// What is kept, or nothing kept where there is no file or it will not read.
fn read(ctx: &Ctx) -> Vec<Open> {
    place(ctx)
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// Write what is kept.
fn write(ctx: &Ctx, open: &[Open]) {
    let Some(path) = place(ctx) else {
        return;
    };
    let text = serde_json::to_string_pretty(open).unwrap_or_default();
    let _ = crate::config::store::write(&path, &text);
}

/// Change what is kept, holding every other change in this process off until it is
/// written.
fn changed(ctx: &Ctx, change: impl FnOnce(&mut Vec<Open>)) {
    let _held = WRITING
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut open = read(ctx);
    change(&mut open);
    write(ctx, &open);
}

/// Keep `opened`, in place of anything kept under the same id.
pub(crate) fn kept(ctx: &Ctx, opened: Open) {
    changed(ctx, |open| {
        open.retain(|one| one.closes_by != opened.closes_by);
        open.push(opened);
    });
}

/// The live stream `member` holds that names `stream`, where the core opened one.
pub(crate) fn theirs(ctx: &Ctx, member: &str, stream: &str) -> Option<Open> {
    read(ctx)
        .into_iter()
        .find(|one| one.member == member && one.streams.iter().any(|named| named == stream))
}

/// Record that the live stream closed by `closes_by` holds a pass until `lapses`.
pub(crate) fn renewed(ctx: &Ctx, closes_by: &str, lapses: u64) {
    changed(ctx, |open| {
        for one in open.iter_mut().filter(|one| one.closes_by == closes_by) {
            one.lapses = lapses;
        }
    });
}

/// Close every live stream kept that `which` picks, and forget each one closed.
///
/// One the media server would not close is kept, so the next sweep asks again rather
/// than forgetting a stream that is still open.
pub(crate) async fn closed(ctx: &Ctx, server: &dyn Tuning, which: impl Fn(&Open) -> bool) {
    let picked: Vec<Open> = read(ctx).into_iter().filter(|one| which(one)).collect();
    let mut gone = Vec::new();
    for one in picked {
        if server.untune(&one.closes_by).await.is_ok() {
            gone.push(one.closes_by);
        }
    }
    if !gone.is_empty() {
        changed(ctx, |open| {
            open.retain(|one| !gone.contains(&one.closes_by))
        });
    }
}

/// Close every live stream whose pass lapsed unrenewed.
///
/// Quietly nothing where nothing lapsed or there is no media server to ask: the next
/// sweep that can reach one does it.
pub(crate) async fn lapsed(ctx: &Ctx) {
    if ctx.dry_run {
        return;
    }
    let now = ctx.seconds();
    if !read(ctx).iter().any(|one| one.lapses <= now) {
        return;
    }
    let Ok(manifest) = ctx.stack.checked_manifest(ctx.today()) else {
        return;
    };
    if let Some(server) = crate::app::targets::serving(ctx, &manifest).await {
        closed(ctx, server.as_ref(), |one| one.lapses <= now).await;
    }
}

#[cfg(test)]
mod tests;
