//! Where each service files what it downloads.
//!
//! A root folder is the one piece of seeding two services can genuinely contest — the
//! same path claimed by two *arrs is an operator decision, not something to resolve on
//! their behalf.

use super::{
    canonical_root, observe_or_skip, same_path, wire_one, BTreeMap, Client, Journal, Naming,
    RootFolder, State, Wiring,
};

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
/// A folder another \*arr also wants — named in `contested` (from
/// [`contested_roots`]) — is refused rather than written, because two \*arrs on
/// one root folder would each manage the other's files. A folder outside `root`,
/// the data tree lemonfiber mounts, is refused too: the service would file where
/// its downloads are neither hardlinked to nor visible to the rest of the stack.
/// Both refusals are made only once the service is reachable, so a service still
/// starting is skipped and retried rather than handed a verdict a re-run cannot
/// lift.
pub async fn wire_root_folders(
    client: &dyn Client,
    service: &str,
    wanted: &[RootFolder],
    contested: &BTreeMap<String, Vec<String>>,
    root: &str,
    journal: &mut Journal,
    at: &str,
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
        let state = if let Some(reason) = contest_reason(service, folder, contested) {
            State::Refused { reason }
        } else if let Some(reason) = outside_root_reason(folder, root) {
            State::Refused { reason }
        } else if already {
            State::AlreadyWired
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
            )
            .await
        };
        wirings.push(Wiring::settled(describe(service, folder), state));
    }
    wirings
}

/// Root-folder paths more than one \*arr wants, each mapped to the \*arrs that
/// want it, named and sorted. Two \*arrs pointed at one root folder would each
/// manage the other's files, so a shared folder is refused rather than wired; a
/// path only one \*arr wants is left out, since there is nothing to refuse.
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

/// Why a wanted folder is refused: the other \*arrs that also claim its path,
/// named, or `None` where the path is this \*arr's alone. Called only for a
/// folder this \*arr wants, so where the path is contested this \*arr is one of
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
        "{} is also the root folder for {}; two *arrs on one root folder would each manage the other's files",
        folder.path,
        others.join(" and ")
    ))
}

/// Why a wanted folder is refused for falling outside the data root: its path, or
/// `None` where it sits within `root` — as every folder lemonfiber builds does,
/// under the tree it mounts at `root`. A root folder outside that tree would have
/// the service file where its downloads are neither hardlinked to nor visible to
/// the rest of the stack, so it is refused rather than created.
pub(super) fn outside_root_reason(folder: &RootFolder, root: &str) -> Option<String> {
    let within = format!("{}/", canonical_root(root));
    if canonical_root(&folder.path).starts_with(&within) {
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
