//! Where each service files what it downloads.
//!
//! A root folder is the one piece of seeding two services can genuinely contest — the
//! same path claimed by two curators is an operator decision, not something to resolve on
//! their behalf.

use std::path::{Path, PathBuf};

use super::{
    canonical_root, observe_or_skip, same_path, wire_one, BTreeMap, Client, Journal, Naming,
    RootFolder, State, Wiring,
};
use crate::ports::filesystem::FileSystem;

/// What a wanted root folder is judged against before it may be written: the paths
/// another curator also claims, and the data tree lemonfiber mounts. And where that tree
/// is on the host, so the folder's directory is made before the service is asked to
/// file into it.
///
/// Two refusals answering one question — may this service file here? — so they arrive
/// as one thing rather than as two parameters a caller could pass one of. Neither is a
/// consequence of writing: both are read from what the operator's stack already is,
/// which is why a pass that only says what it would do reports them exactly as a real
/// run does.
pub struct Placing<'a> {
    /// Root-folder paths more than one curator wants, from [`contested_roots`]. A folder
    /// named here is refused rather than written: two curators on one root folder would
    /// each manage the other's files.
    pub contested: &'a BTreeMap<String, Vec<String>>,
    /// The host data root every root folder must sit within. A folder outside it is
    /// refused: the service would file where its downloads are neither hardlinked to
    /// nor visible to the rest of the stack.
    pub root: &'a str,
    /// Where `root` is on the host, for making a folder's directory before it is
    /// registered, or `None` where no data root is known.
    pub backing: Option<Backing<'a>>,
}

/// The host directory the data root is mounted from, and the filesystem it is on.
#[derive(Clone, Copy)]
pub struct Backing<'a> {
    /// The filesystem the data root is on.
    pub filesystem: &'a dyn FileSystem,
    /// The host directory mounted at [`Placing::root`].
    pub data_root: &'a Path,
}

/// Wire a service's root folders: register the ones it lacks, leave the ones it
/// already has, and record each write as a change.
///
/// The service is observed once. If it is not answering, every folder is skipped
/// so a later run completes them rather than any being called broken; if it
/// refuses, they fail. A folder the service already has is left alone — matched
/// by path, so a second run writes nothing. Each folder that must be written is
/// read back before it is called wired, because a write is not done until the
/// service reports it, and only then is it recorded.
///
/// A folder another curator also wants — named in [`Placing::contested`] — is refused
/// rather than written, because two curators on one root folder would each manage the
/// other's files. A folder outside [`Placing::root`], the data tree lemonfiber
/// mounts, is refused too: the service would file where
/// its downloads are neither hardlinked to nor visible to the rest of the stack.
/// Both refusals are made only once the service is reachable, so a service still
/// starting is skipped and retried rather than handed a verdict a re-run cannot
/// lift. A folder within the data tree has its directory made on the host where
/// [`Placing::backing`] says the tree is, before it is registered, because a service
/// refuses a folder that is not there; a directory that cannot be made fails the
/// folder with the platform's reason.
///
/// `rehearsing` changes one thing: a folder that would be registered is reported as
/// the folder it would be rather than made and written. Everything above that — the
/// read, the two refusals and the already-there match — is the same walk either way,
/// because each of them is a fact about the service rather than a consequence of
/// writing to it, and a rehearsal that reported them differently would be describing a
/// different run from the one it claims to be describing.
pub async fn wire_root_folders(
    client: &dyn Client,
    service: &str,
    wanted: &[RootFolder],
    placing: Placing<'_>,
    journal: &mut Journal,
    at: &str,
    rehearsing: bool,
) -> Vec<Wiring> {
    let existing = match observe_or_skip(client.root_folders().await, wanted, |folder| {
        describe(service, folder)
    }) {
        Ok(existing) => existing,
        Err(skipped) => return skipped,
    };

    let mut wirings = Vec::new();
    for folder in wanted {
        let already = existing
            .iter()
            .any(|have| same_path(&have.path, &folder.path));
        let state = if let Some(reason) = contest_reason(service, folder, placing.contested) {
            State::Refused { reason }
        } else if let Some(reason) = outside_root_reason(folder, placing.root) {
            State::Refused { reason }
        } else if already {
            State::AlreadyWired
        } else if let Some(detail) = unmade(&placing, folder, rehearsing).await {
            State::Failed { detail }
        } else {
            wire_one(
                client.register_root_folder(folder),
                client.root_folders(),
                |rows| {
                    rows.iter()
                        .find(|have| same_path(&have.path, &folder.path))
                        .map(|have| have.id.clone())
                },
                Naming {
                    service,
                    resource: "rootfolder",
                    noun: "folder",
                },
                journal,
                at,
                rehearsing.then(|| State::WouldWire {
                    yours: None,
                    ours: Some(folder.path.clone()),
                }),
            )
            .await
        };
        wirings.push(Wiring::settled(describe(service, folder), state));
    }
    wirings
}

/// Root-folder paths more than one curator wants, each mapped to the curators that
/// want it, named and sorted. Two curators pointed at one root folder would each
/// manage the other's files, so a shared folder is refused rather than wired; a
/// path only one curator wants is left out, since there is nothing to refuse.
#[must_use]
pub fn contested_roots<'a>(
    claims: impl IntoIterator<Item = (&'a str, &'a [RootFolder])>,
) -> BTreeMap<String, Vec<String>> {
    let mut by_path: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (service, folders) in claims {
        for folder in folders {
            let services = by_path.entry(canonical_root(&folder.path)).or_default();
            if !services.iter().any(|name| name == service) {
                services.push(service.to_owned());
            }
        }
    }
    for services in by_path.values_mut() {
        services.sort();
    }
    by_path.retain(|_, services| services.len() > 1);
    by_path
}

/// Why a wanted folder is refused: the other curators that also claim its path,
/// named, or `None` where the path is this curator's alone. Called only for a
/// folder this curator wants, so where the path is contested this curator is one of
/// its claimants and at least one other remains to name.
pub(super) fn contest_reason(
    service: &str,
    folder: &RootFolder,
    contested: &BTreeMap<String, Vec<String>>,
) -> Option<String> {
    let others: Vec<&str> = contested
        .get(&canonical_root(&folder.path))?
        .iter()
        .map(String::as_str)
        .filter(|name| *name != service)
        .collect();
    Some(format!(
        "{} is also the root folder for {}; two curators on one root folder would each manage the other's files",
        folder.path,
        others.join(" and ")
    ))
}

/// Why a folder's directory could not be made on the host before it is registered,
/// or `None` where it is there, where this pass only rehearses, or where no data
/// root is known to make it in.
async fn unmade(placing: &Placing<'_>, folder: &RootFolder, rehearsing: bool) -> Option<String> {
    let backing = placing.backing.filter(|_| !rehearsing)?;
    let host = on_host(folder, placing.root, backing.data_root)?;
    let made = backing.filesystem.make_beneath(&host, backing.data_root);
    made.await.err().map(|fault| {
        format!(
            "the directory {} could not be made: {}",
            host.display(),
            fault.message
        )
    })
}

/// The host directory backing a folder registered under `root`, or `None` where
/// the folder is not beneath `root`.
pub(crate) fn on_host(folder: &RootFolder, root: &str, data_root: &Path) -> Option<PathBuf> {
    beneath(folder, root).map(|rest| data_root.join(rest))
}

/// A folder's path below `root`, or `None` where it is not beneath `root`.
fn beneath(folder: &RootFolder, root: &str) -> Option<String> {
    let within = format!("{}/", canonical_root(root));
    canonical_root(&folder.path)
        .strip_prefix(&within)
        .map(str::to_owned)
}

/// Why a wanted folder is refused for falling outside the data root: its path, or
/// `None` where it sits within `root` — as every folder lemonfiber builds does,
/// under the tree it mounts at `root`. A root folder outside that tree would have
/// the service file where its downloads are neither hardlinked to nor visible to
/// the rest of the stack, so it is refused rather than created.
pub(super) fn outside_root_reason(folder: &RootFolder, root: &str) -> Option<String> {
    if beneath(folder, root).is_some() {
        return None;
    }
    Some(format!(
        "{} is outside the data root {root}; a root folder there is neither hardlinked to nor visible to the rest of the stack",
        folder.path
    ))
}

/// A connection's description for the report.
pub(super) fn describe(service: &str, folder: &RootFolder) -> String {
    format!("{} root folder in {service}", folder.media_type)
}
