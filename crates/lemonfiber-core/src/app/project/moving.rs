//! A service's state the stack's static Compose files kept inside its configuration
//! directory, moved once into its own mount tree, where the project mounts it from.
//!
//! Moved rather than copied, so nothing is held twice, and never through a link: the
//! configuration directory is one its container writes, so both ends are reached from the
//! project directory with no link at any step, and nothing is moved over what already
//! stands at the new place. A file the stack carries there is a starting file rather than
//! state, and stays; and a stack directory the operator named is run as it stands, so
//! nothing in it is moved.

use std::path::{Component, Path, PathBuf};

use include_dir::Dir;
use lemonfiber_manifest::Manifest;

use super::super::Ctx;
use crate::error::{Diagnose, Problem};
use crate::journal::{Change, Kind};
use crate::ports::filesystem::{Confined, Lying};
use crate::project::entry::{mounts, CONFIGURATION_TREE, MOUNT_TREE};
use crate::stack::Source;

/// Move every stack service's mounted state from its configuration directory into its
/// mount tree, where it is still at the one and not yet at the other, journalling each
/// move and each directory it makes on the way.
///
/// # Errors
///
/// Where the journal cannot be written. A move the filesystem refuses is left where it
/// was, and its service starts on an empty mount.
pub(crate) fn moved(ctx: &Ctx, manifest: &Manifest) -> Result<(), Box<Problem>> {
    let (false, Source::Embedded(carried), Some(directory), Some(paths)) = (
        ctx.dry_run,
        ctx.stack,
        super::directory(ctx),
        super::super::targets::layout(ctx),
    ) else {
        return Ok(());
    };
    let stamp = ctx.stamp();
    let confined = ctx.seams.confined.as_ref();
    for (from, to) in pending(confined, &directory, manifest, carried) {
        let mut changes: Vec<Change> = missing(&to)
            .into_iter()
            .map(|path| change(&stamp, Kind::Made { path: shown(&path) }, &path))
            .collect();
        changes.push(change(
            &stamp,
            Kind::Moved {
                from: shown(&from),
                to: shown(&to),
            },
            &from,
        ));
        crate::app::recover::journalled(&paths.journal(), &changes, ctx.seams.random.as_ref())
            .map_err(|failure| Box::new(failure.problem()))?;
        let _ = confined.moved(&from, &to, &directory);
    }
    Ok(())
}

/// Every mount of the stack's services whose state is still at its old place inside the
/// configuration directory, as where it is and where it goes.
fn pending(
    confined: &dyn Confined,
    directory: &Path,
    manifest: &Manifest,
    carried: &Dir<'_>,
) -> Vec<(PathBuf, PathBuf)> {
    crate::project::declared(manifest)
        .flat_map(|service| {
            mounts(&service.mount, service.takes_data).map(move |mount| {
                let starting = Path::new(CONFIGURATION_TREE)
                    .join(&service.id)
                    .join(&mount.from);
                let moved = Path::new(MOUNT_TREE).join(&service.id).join(&mount.from);
                (starting, moved)
            })
        })
        .filter(|(starting, _)| carried.get_file(starting).is_none())
        .map(|(starting, moved)| (directory.join(starting), directory.join(moved)))
        .filter(|(from, to)| {
            confined.lies(from, directory) == Lying::Plain
                && confined.lies(to, directory) == Lying::Absent
        })
        .collect()
}

/// The project directory a move from `from` to `to` was made beneath, where the two are a
/// pair moving makes: the same service and the same name, one in the configuration tree
/// and the other in the mount tree of one directory, every step an ordinary name.
pub(crate) fn beneath(from: &Path, to: &Path) -> Option<PathBuf> {
    let from: Vec<Component<'_>> = from.components().collect();
    let to: Vec<Component<'_>> = to.components().collect();
    if from.len() != to.len() {
        return None;
    }
    let at = from.iter().zip(&to).position(|(one, other)| one != other)?;
    let (root, was) = from.split_at_checked(at)?;
    let (_, now) = to.split_at_checked(at)?;
    let ordinary = |part: &Component<'_>| matches!(part, Component::Normal(_));
    let (mut was_below, mut now_below) = (was.iter(), now.iter());
    let paired = was_below.next() == Some(&Component::Normal(CONFIGURATION_TREE.as_ref()))
        && now_below.next() == Some(&Component::Normal(MOUNT_TREE.as_ref()))
        && was_below.len() >= 2
        && was_below.as_slice() == now_below.as_slice()
        && was.iter().all(ordinary)
        && root.iter().all(|part| {
            ordinary(part) || matches!(part, Component::RootDir | Component::Prefix(_))
        });
    paired.then(|| root.iter().collect())
}

/// Every directory above `path` that is not there yet, parent before child.
pub(super) fn missing(path: &Path) -> Vec<PathBuf> {
    let mut made: Vec<PathBuf> = path
        .ancestors()
        .skip(1)
        .take_while(|above| std::fs::symlink_metadata(above).is_err())
        .map(Path::to_path_buf)
        .collect();
    made.reverse();
    made
}

/// A path as the journal names it.
pub(super) fn shown(path: &Path) -> String {
    path.display().to_string()
}

/// One entry of the move about `target`, journalled under the project's own operation.
pub(super) fn change(stamp: &str, kind: Kind, target: &Path) -> Change {
    Change {
        at: stamp.to_owned(),
        operation: super::OPERATION.to_owned(),
        target: shown(target),
        kind,
    }
}
