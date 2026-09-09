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
pub const MOUNT: &str = "/data";

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
mod tests {
    use std::path::Path;

    use super::{moving, unresolvable, Existing};

    /// A path a service holds, with whether the host directory behind it is there.
    fn holding(service: &str, path: &str, present: bool) -> Existing {
        Existing {
            service: service.to_owned(),
            path: path.to_owned(),
            present,
        }
    }

    #[test]
    fn a_folder_the_new_location_already_holds_is_carried_and_says_where_to() {
        let moved = moving(
            &[holding("sonarr", "/data/media/tv", true)],
            Path::new("/srv/new"),
        );
        let one = moved.first();
        assert!(one.is_some_and(|row| row.carried));
        assert_eq!(
            one.and_then(|row| row.host.clone()),
            Some("/srv/new/media/tv".to_owned())
        );
        assert!(unresolvable(&moved).is_none());
    }

    #[test]
    fn a_folder_the_new_location_does_not_hold_is_refused_and_names_the_host_path() {
        let moved = moving(
            &[holding("radarr", "/data/media/movies", false)],
            Path::new("/srv/new"),
        );
        let said = moved
            .first()
            .map(|row| row.because.clone())
            .unwrap_or_default();
        assert!(
            said.contains("/srv/new/media/movies is not there"),
            "{said}"
        );
        let refusal = unresolvable(&moved).unwrap_or_default();
        assert!(refusal.contains("/data/media/movies"), "{refusal}");
        assert!(refusal.contains("Nothing was written."), "{refusal}");
    }

    #[test]
    fn a_folder_outside_the_mount_cannot_be_repointed_by_a_move_at_all() {
        // An adopted stack's own root folder, or one the operator set by hand. No
        // write to the data location reaches it, so the move is refused rather than
        // silently leaving it where it was.
        let moved = moving(
            &[holding("sonarr", "/mnt/old/tv", true)],
            Path::new("/srv/new"),
        );
        let row = moved.first();
        assert!(row.is_some_and(|row| !row.carried && row.host.is_none()));
        let said = row.map(|row| row.because.clone()).unwrap_or_default();
        assert!(said.contains("outside /data"), "{said}");
        assert!(unresolvable(&moved).is_some());
    }

    #[test]
    fn the_mount_itself_is_repointed_like_anything_beneath_it() {
        let moved = moving(&[holding("lidarr", "/data/", true)], Path::new("/srv/new"));
        assert_eq!(
            moved.first().and_then(|row| row.host.clone()),
            Some("/srv/new".to_owned())
        );
    }

    #[test]
    fn a_name_that_merely_starts_with_the_mount_is_not_under_it() {
        // `/database` is not inside `/data`, and reading it as though it were would
        // carry a move that must be refused.
        let moved = moving(
            &[holding("sonarr", "/database/tv", true)],
            Path::new("/srv/new"),
        );
        assert!(moved.first().is_some_and(|row| row.host.is_none()));
    }

    #[test]
    fn a_stack_holding_no_library_path_has_nothing_to_refuse() {
        assert!(unresolvable(&moving(&[], Path::new("/srv/new"))).is_none());
    }
}
