//! A plugin the bundle installs, read from the tree this build carries.
//!
//! Its manifest and every file of its tree are written into a checkout of their own and
//! installed from there as any directory is, recorded as its repository at the revision
//! the bundle pins: nothing is fetched, and nothing about it passes the gates any other
//! install is held to.

use std::path::{Path, PathBuf};

use lemonfiber_bundled::{Pinned, Stage, PINNED};

use super::{fetching, Consent, Errand, Installs, Register};
use crate::app::Ctx;
use crate::error::{Diagnose as _, Problem};

/// The plugin of this id the bundle installs, where it pins one.
pub(super) fn pinned(id: &str) -> Option<&'static Pinned> {
    PINNED
        .iter()
        .find(|one| one.first_party.plugin == id && one.stage == Some(Stage::Installed))
}

/// Carry an errand out over `pinned`'s tree, written where a fetch would have checked it
/// out and removed again afterwards.
///
/// # Errors
///
/// Where there is nowhere to write it or it cannot be written, and every refusal the
/// errand makes over a directory.
pub(super) async fn embedded(
    ctx: &Ctx,
    held: Register,
    pinned: &Pinned,
    errand: Errand<'_>,
    consent: &Consent,
) -> Result<Installs, Box<Problem>> {
    let Some(into) = fetching::checkout(ctx, pinned.revision) else {
        return Err(Box::new(crate::config::store::Failure::Nowhere.problem()));
    };
    let outermost = outermost_missing(&into);
    if let Err(why) = written(&into, pinned) {
        taken_away(&into, &outermost);
        return Err(Box::new(fetching::unfetched(
            pinned.origin,
            &why.to_string(),
        )));
    }
    let from = fetching::Fetched {
        url: pinned.origin,
        commit: pinned.revision,
        vouched: None,
    };
    let result = super::carried(ctx, held, &into, Some(&from), errand, consent).await;
    taken_away(&into, &outermost);
    result
}

/// The outermost directory on the way to `into` that is not there yet.
fn outermost_missing(into: &Path) -> PathBuf {
    into.ancestors()
        .take_while(|dir| !dir.exists())
        .last()
        .unwrap_or(into)
        .to_path_buf()
}

/// `into` taken away, and each directory above it up to `outermost` the errand made,
/// where nothing else was written into it since.
fn taken_away(into: &Path, outermost: &Path) {
    let _ = std::fs::remove_dir_all(into);
    for dir in into
        .ancestors()
        .skip(1)
        .take_while(|dir| dir.starts_with(outermost))
    {
        if std::fs::remove_dir(dir).is_err() {
            break;
        }
    }
}

/// `pinned`'s manifest and tree, written into `into` made fresh.
fn written(into: &std::path::Path, pinned: &Pinned) -> std::io::Result<()> {
    fetching::made_fresh(into)?;
    std::fs::write(into.join(lemonfiber_bundle::MANIFEST), pinned.copy)?;
    for (path, bytes) in pinned.files {
        let at = into.join(path);
        std::fs::create_dir_all(at.parent().unwrap_or(into))?;
        std::fs::write(at, bytes)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
