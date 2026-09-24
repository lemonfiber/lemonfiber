//! What moving the data location does to the library paths already in place.
//!
//! The sharpest change in the product. Every \*arr holds absolute paths to its root
//! folders, and those paths are inside its container: they name the mount, not the
//! host directory under it. Moving the data location moves what the mount resolves
//! to, so a path like `/data/media/tv` goes on being spelled the same and starts
//! meaning somewhere else — which is why the breakage is silent, and why it has to
//! be worked out before the write rather than discovered after it.
//!
//! Two ways a path fails to survive the move. It can sit inside the mount and
//! resolve to a host directory that is not there, which leaves the \*arr importing
//! into a void. Or it can sit *outside* the mount altogether — a folder the operator
//! or an adopted stack pointed somewhere of its own — in which case moving the data
//! location cannot re-point it at all, and no write here would.
//!
//! Nothing here reads a filesystem or a service. What was found is handed in; what
//! it comes to is decided here.

use std::path::Path;

use super::LibraryPath;

/// Where the operator's data location is mounted inside every service.
///
/// The tree a root folder has to sit within for moving the data location to carry
/// it: a path under this is re-pointed by the move, and a path outside it is not
/// reachable by the move at all.
pub(crate) const MOUNT: &str = "/data";

/// One library path a service holds, as it was read from that service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Existing {
    /// The service holding it.
    pub service: String,
    /// The path as that service holds it — a path inside its container.
    pub path: String,
    /// Whether the host directory it would resolve to after the move is there.
    ///
    /// Answered by whoever went and looked; a path outside the mount has no host
    /// directory the move could resolve, and this says nothing about those.
    pub present: bool,
}

/// What moving the data location to `to` does to each library path in hand.
///
/// One row per path, in the order they were read, each saying whether the library
/// there survives the move and why. A path under the mount survives exactly when
/// the host directory it lands on exists; one outside the mount never does.
#[must_use]
pub fn moving(existing: &[Existing], to: &Path) -> Vec<LibraryPath> {
    existing
        .iter()
        .map(|found| match beneath(&found.path) {
            None => LibraryPath {
                service: found.service.clone(),
                path: found.path.clone(),
                host: None,
                carried: false,
                because: format!(
                    "{} is outside {MOUNT}, the tree the data location is mounted at, so moving \
                     the data location cannot re-point it — {} would go on filing where it \
                     files now",
                    found.path, found.service
                ),
            },
            Some(rest) => {
                // An empty remainder is the mount itself, and joining nothing onto a
                // path adds a trailing separator that names the same directory by a
                // different string — which two reports would then disagree about.
                let host = if rest.is_empty() {
                    to.to_path_buf()
                } else {
                    to.join(rest)
                };
                LibraryPath {
                    service: found.service.clone(),
                    path: found.path.clone(),
                    host: Some(host.display().to_string()),
                    carried: found.present,
                    because: if found.present {
                        format!(
                            "{} keeps filing into {}, which after the move is {}",
                            found.service,
                            found.path,
                            host.display()
                        )
                    } else {
                        format!(
                            "{} is not there, so {} would go on filing into {} and find nothing \
                             behind it",
                            host.display(),
                            found.service,
                            found.path
                        )
                    },
                }
            }
        })
        .collect()
}

/// Why the move must not go ahead, where the library would not survive it.
///
/// `None` where every path carries, which is the move that may simply be made.
/// A path that would not carry is not a judgement call an operator can take on:
/// there is no version of this where the \*arrs point at absent paths and the
/// operator is better off, so the sentence names what breaks rather than offering
/// to break it.
#[must_use]
pub fn unresolvable(paths: &[LibraryPath]) -> Option<String> {
    let lost: Vec<&str> = paths
        .iter()
        .filter(|path| !path.carried)
        .map(|path| path.path.as_str())
        .collect();
    if lost.is_empty() {
        return None;
    }
    Some(format!(
        "moving the data location would leave {} pointing at nothing. Nothing was written.",
        lost.join(", ")
    ))
}

/// The part of a container path that sits under the mount, or `None` where it does
/// not sit under it at all.
///
/// The mount itself counts as being under it, and comes back as an empty remainder:
/// a root folder registered as `/data` is re-pointed by the move exactly as one
/// beneath it is.
fn beneath(path: &str) -> Option<&str> {
    let trimmed = path.trim_end_matches('/');
    if trimmed == MOUNT {
        return Some("");
    }
    trimmed
        .strip_prefix(MOUNT)
        .and_then(|rest| rest.strip_prefix('/'))
}

#[cfg(test)]
mod tests;
