//! Reading where the services actually file, before the data location moves.
//!
//! The \*arrs are the only things that know: each holds its own root folders, and
//! what they hold is not what lemonfiber would write today — an adopted stack, a
//! folder added by hand, or one left behind by an earlier location all live here
//! and all survive a move differently. So they are asked rather than assumed.
//!
//! Where they cannot be asked, this says so rather than answering with none. A
//! service that will not talk has not told anybody it has no library, and reading
//! silence as an empty one is exactly how a move comes to be applied over a library
//! it would break.

use std::path::Path;

use crate::reconfigure::relocating::{moving as decided, Existing, MOUNT};
use crate::reconfigure::LibraryPath;

use super::Ctx;

/// What the services hold, and why it could not be read where it could not.
pub(super) struct Library {
    /// What each path the services hold comes to after the move.
    pub(super) paths: Vec<LibraryPath>,
    /// Why nothing could be read, where a library is here and nothing would say
    /// where it is filed.
    pub(super) unread: Option<String>,
}

/// What moving the data location to `to` does to the library already here.
pub(super) async fn moving(ctx: &Ctx, to: &Path) -> Library {
    let Some(from) = ctx.settings.data_root.as_deref() else {
        // Nothing has been chosen yet, so there is no library at a previous location
        // for this to invalidate — this is setup's answer, not a move. A location
        // already set to what was asked for is settled before this is reached: the
        // proposal reads that as nothing to do, so there is no move to weigh.
        return nothing();
    };
    let Some(existing) = held(ctx, to).await else {
        return Library {
            paths: Vec::new(),
            unread: unreachable(ctx, from).await,
        };
    };
    Library {
        paths: decided(&existing, to),
        unread: None,
    }
}

/// A library nothing has to say about.
const fn nothing() -> Library {
    Library {
        paths: Vec::new(),
        unread: None,
    }
}

/// The root folders every \*arr that could be opened holds, with whether the host
/// directory each would land on after the move is there.
///
/// `None` where not one \*arr could be asked — which is a different answer from
/// none of them holding anything, and the only one that must never be read as an
/// empty library. A single \*arr answering is enough to make this a read: the
/// others are still starting, and a later run completes them.
async fn held(ctx: &Ctx, to: &Path) -> Option<Vec<Existing>> {
    let manifest = ctx.stack.checked_manifest(ctx.today()).ok()?;
    let project =
        super::super::targets::project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref());
    let arrs = super::super::seed::servarr_arrs(&manifest.services, project.as_deref());

    let mut found = Vec::new();
    let mut answered = false;
    for arr in &arrs {
        let Some(client) = arr.target.open(&ctx.http, ctx.filesystem.as_ref()).await else {
            continue;
        };
        let Ok(folders) = crate::ports::service::Client::root_folders(&client).await else {
            continue;
        };
        answered = true;
        for folder in folders {
            let present = resolves(ctx, to, &folder.path).await;
            found.push(Existing {
                service: arr.target.name.clone(),
                path: folder.path,
                present,
            });
        }
    }
    answered.then_some(found)
}

/// Whether the host directory a container path would land on after the move is
/// there. A path outside the mount lands nowhere the move decides, and its
/// presence is not what settles it, so it is not looked for.
async fn resolves(ctx: &Ctx, to: &Path, path: &str) -> bool {
    let Some(rest) = path
        .trim_end_matches('/')
        .strip_prefix(MOUNT)
        .map(|rest| rest.trim_start_matches('/'))
    else {
        return false;
    };
    ctx.filesystem.canonicalize(&to.join(rest)).await.is_ok()
}

/// Why a move must not be made when nothing would say where the library is filed.
///
/// Only where there is a library to lose. A previous location with no media
/// directory under it holds nothing a move could break, so a stack that has never
/// run is moved freely rather than blocked by services that were never up.
async fn unreachable(ctx: &Ctx, from: &Path) -> Option<String> {
    let media = from.join("media");
    if ctx.filesystem.canonicalize(&media).await.is_err() {
        return None;
    }
    Some(format!(
        "there is a library in {} and no service would say where it files, so moving the data \
         location cannot be shown to be safe. Nothing was written.",
        media.display()
    ))
}
