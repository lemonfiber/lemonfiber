//! Where the stack's data may be kept.
//!
//! Every container given the data root can change anything beneath it, so the data root
//! may not be the machine itself or a part of it the machine runs on. Refused:
//!
//! - the filesystem root;
//! - one of the system's own trees, at it or beneath it: [`SYSTEM`], and every tree whose
//!   name starts with [`LIBRARIES`]. `/run` holds the engine's socket, which is root to
//!   whoever holds it; beneath [`MOUNTED`], where drives are mounted, is the exception;
//! - one of the [`BASES`] drives and homes are kept beneath, as a whole: it holds every
//!   drive or every user beneath it. A directory beneath one is not refused for that;
//! - anything at all where the operator's home is not known, or is the filesystem root,
//!   since nothing could then rule out that the path holds it;
//! - the operator's home, anything above it, any hidden directory at any depth in it, and
//!   the platform's own library there;
//! - another user's home: anything beneath one of [`HOMES`] that is not in the operator's.
//!
//! Anything else is the operator's to choose, a pool or a share at the top of the
//! filesystem included. A path written relative to the stack's directory has to stay
//! inside it, and is then judged by the same rules.
//!
//! Judged on the path every link on the way resolves to, since a link is where the
//! containers would really be writing; a link that leads somewhere not there yet is
//! refused, since where it leads cannot be judged, and without regard to case where the platform's
//! filesystem has none, since there `/ETC` is `/etc`. Only the paths themselves are read,
//! so the judgement is the same inside a container that has the stack's directory mounted
//! at the host's path, where the data root itself may not be there at all.

use std::path::{Component, Path, PathBuf};

use crate::platform::Environment;
use crate::ports::filesystem::FileSystem;

use super::Ctx;

/// The system's own trees: a data root at or beneath any of them is refused.
const SYSTEM: &[&str] = &[
    "/etc",
    "/usr",
    "/bin",
    "/sbin",
    "/boot",
    "/dev",
    "/proc",
    "/sys",
    "/var",
    "/root",
    "/run",
    "/tmp",
    "/snap",
    "/nix",
    "/lost+found",
    "/opt/containerd",
    "/private",
    "/System",
    "/Applications",
    "/Library",
];

/// What the first name of every library tree beneath the root starts with: `/lib`,
/// `/lib32`, `/lib64`, `/libx32` and the rest.
const LIBRARIES: &str = "lib";

/// The one place inside a system tree that drives are mounted beneath, which is not the
/// system's own beneath it.
const MOUNTED: &str = "/run/media";

/// The directories drives and homes are kept beneath: refused as a whole, and not for
/// that reason beneath.
const BASES: &[&str] = &[
    "/home", "/Users", "/mnt", "/media", "/srv", "/Volumes", "/opt", MOUNTED,
];

/// The directories users' homes are kept beneath.
const HOMES: &[&str] = &["/home", "/Users"];

/// Directories directly in the operator's home that are the platform's own rather than
/// theirs to fill.
const HOME_OWN: &[&str] = &["Library"];

/// Why anything is refused where the operator's home is not known.
const UNKNOWN_HOME: &str =
    "your home directory is not known, so nothing rules out that this holds it";

/// Why `value` will not do as the data root, where it will not.
pub(super) async fn refusal(ctx: &Ctx, value: &str) -> Option<String> {
    let path = Path::new(value);
    let plain = path
        .components()
        .all(|part| !matches!(part, Component::ParentDir | Component::Prefix(_)));
    let judge = Judge::of(ctx);
    let why = if !plain {
        "it is written with `..`, which hides where it really is"
    } else if path.is_absolute() {
        judge.handed_over(ctx, path).await?
    } else {
        match judge.in_the_stack(ctx, path).await {
            Some(inside) => judge.handed_over(ctx, &inside).await?,
            None => "a path written relative to the stack's directory has to stay inside it",
        }
    };
    Some(format!(
        "{value} cannot be the data root: every container given the data root can change \
         anything beneath it, and {why}"
    ))
}

/// How names are compared on this machine.
#[derive(Debug, Clone, Copy)]
struct Judge {
    /// Whether the filesystem ignores case.
    caseless: bool,
}

impl Judge {
    /// The comparison `ctx`'s platform makes.
    fn of(ctx: &Ctx) -> Self {
        Self {
            caseless: ctx.environment == Environment::MacOs,
        }
    }

    /// What part of the machine the absolute `path` would hand the containers, where it
    /// would hand them one.
    async fn handed_over(self, ctx: &Ctx, path: &Path) -> Option<&'static str> {
        let files = ctx.seams.filesystem.as_ref();
        let Some(real) = resolved(files, path).await else {
            return Some("a link on the way leads somewhere that is not there yet");
        };
        let real = names(&real);
        let Some(first) = real.first() else {
            return Some("this is the whole machine");
        };
        if self.system(&real, first) {
            return Some("this is a part of the machine it runs on");
        }
        if BASES.iter().any(|base| self.at(&real, base)) {
            return Some("this holds every drive or every user beneath it");
        }
        let Some(home) = ctx.settings.home.as_deref() else {
            return Some(UNKNOWN_HOME);
        };
        let Some(home) = resolved(files, home).await else {
            return Some(UNKNOWN_HOME);
        };
        let home = names(&home);
        if home.is_empty() {
            return Some(UNKNOWN_HOME);
        }
        if self.inside(&home, &real).is_some() {
            return Some("this holds your home");
        }
        if let Some(inside) = self.inside(&real, &home) {
            let hidden = inside.iter().any(|name| name.starts_with('.'));
            let own = inside
                .first()
                .is_some_and(|name| HOME_OWN.iter().any(|one| self.same(name, one)));
            return (hidden || own).then_some("this is a directory your home keeps for itself");
        }
        HOMES
            .iter()
            .any(|homes| self.beneath(&real, homes))
            .then_some("this is in another user's home")
    }

    /// Whether `real`, whose first name is `first`, is at or beneath one of the system's
    /// own trees, and not beneath where drives are mounted.
    fn system(self, real: &[String], first: &str) -> bool {
        let tree = SYSTEM
            .iter()
            .any(|tree| self.inside(real, &names(Path::new(tree))).is_some())
            || self.libraries(first);
        tree && !self.beneath(real, MOUNTED)
    }

    /// The absolute path the relative `path` names inside the stack's directory, where it
    /// resolves strictly inside it.
    async fn in_the_stack(self, ctx: &Ctx, path: &Path) -> Option<PathBuf> {
        let project =
            crate::app::targets::project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref())?;
        let files = ctx.seams.filesystem.as_ref();
        let root = names(&resolved(files, &project).await?);
        let joined = project.join(path);
        let real = names(&resolved(files, &joined).await?);
        self.inside(&real, &root)
            .is_some_and(|inside| !inside.is_empty())
            .then_some(joined)
    }

    /// Whether `name`, the first beneath the root, is one of the library trees.
    fn libraries(self, name: &str) -> bool {
        if self.caseless {
            name.to_lowercase().starts_with(LIBRARIES)
        } else {
            name.starts_with(LIBRARIES)
        }
    }

    /// Whether `real` is the directory `base` names.
    fn at(self, real: &[String], base: &str) -> bool {
        self.inside(real, &names(Path::new(base)))
            .is_some_and(|inside| inside.is_empty())
    }

    /// Whether `real` lies strictly beneath the directory `base` names.
    fn beneath(self, real: &[String], base: &str) -> bool {
        self.inside(real, &names(Path::new(base)))
            .is_some_and(|inside| !inside.is_empty())
    }

    /// The names `path` has beneath `base`, where it lies at or beneath it.
    fn inside(self, path: &[String], base: &[String]) -> Option<Vec<String>> {
        let matched = path.len() >= base.len()
            && path
                .iter()
                .zip(base)
                .all(|(one, other)| self.same(one, other));
        matched.then(|| path.get(base.len()..).unwrap_or_default().to_vec())
    }

    /// Whether two names are one, ignoring case where the filesystem does.
    fn same(self, one: &str, other: &str) -> bool {
        if self.caseless {
            one.to_lowercase() == other.to_lowercase()
        } else {
            one == other
        }
    }
}

/// The names in `path` beneath the root.
fn names(path: &Path) -> Vec<String> {
    path.components()
        .filter_map(|part| match part {
            Component::Normal(name) => Some(name.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect()
}

/// `path` with every link on the way resolved: the deepest part of it that is there,
/// resolved, and the names beneath that which are not there yet. Nothing where a part
/// that is not there is a link, which leads somewhere not there yet and so cannot be
/// judged by where it leads, or where no part of it is there at all, which a path
/// written from the root never is.
async fn resolved(files: &dyn FileSystem, path: &Path) -> Option<PathBuf> {
    for ancestor in path.ancestors() {
        if let Ok(real) = files.canonicalize(ancestor).await {
            let rest = path.strip_prefix(ancestor).unwrap_or(Path::new(""));
            return Some(real.join(rest));
        }
        let linked = tokio::fs::symlink_metadata(ancestor)
            .await
            .is_ok_and(|meta| meta.file_type().is_symlink());
        if linked {
            return None;
        }
    }
    None
}

#[cfg(test)]
mod tests;
