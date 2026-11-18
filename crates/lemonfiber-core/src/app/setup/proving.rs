//! The answers that are proven before they are kept.
//!
//! Three of the wizard's questions are not merely read back. A data location is
//! tested for hardlinks, because whether imports cost nothing is a property of the
//! volume rather than of what the operator typed. A credential and a Usenet
//! provider are tried against the live service, because a key that does not work
//! is worth finding out now rather than at the first download.
//!
//! Kept apart from the walk for that reason: the walk is "record the answer and move
//! on", and this is "and do not take the answer on trust". The two go wrong
//! differently, and they are read for different reasons.

use std::path::{Path, PathBuf};

use crate::model::StorageFinding;
use crate::ports::filesystem::FileSystem;
use crate::storage::{self, Linked};
use crate::validate::{pasted, Credential, Validation, Validator};
use crate::wizard::{Answer, Indexer, Provider};

/// What proving an answer came to: the credential's live test, or the data location's.
#[derive(Debug, Default)]
pub(super) struct Proof {
    /// What the service said of the credential given.
    pub(super) credential: Option<Validation>,
    /// What testing the data location given found.
    pub(super) storage: Option<StorageFinding>,
}

/// The answer as setup may record it, and what proving it came to.
///
/// A credential is trimmed, so the value tested is the value stored, and tested against
/// its live service before it is kept — so `validated` records what a test established
/// rather than what a caller asserted. A data location is tested for hardlinks.
///
/// Neither test refuses. The answer is kept and what the test found comes back beside
/// it, which leaves whoever asked the ways out: send a different answer, send none, or
/// go on with this one.
pub(super) async fn proven(
    validator: &dyn Validator,
    filesystem: &dyn FileSystem,
    answer: Answer,
) -> (Answer, Proof) {
    match answer {
        Answer::Credentials(Some(indexer)) => {
            let (url, key) = (pasted(&indexer.url), pasted(&indexer.key));
            let came_to = validator
                .validate(&Credential::Indexer {
                    url: url.clone(),
                    key: key.clone(),
                })
                .await;
            let validated = matches!(came_to, Validation::Valid { .. });
            (
                Answer::Credentials(Some(Indexer {
                    url,
                    key,
                    validated,
                })),
                Proof {
                    credential: Some(came_to),
                    storage: None,
                },
            )
        }
        Answer::Provider(Some(provider)) => {
            let provider = Provider {
                host: pasted(&provider.host),
                user: pasted(&provider.user),
                pass: pasted(&provider.pass),
                ..provider
            };
            let came_to = validator
                .validate(&Credential::Usenet {
                    host: provider.host.clone(),
                    port: provider.port,
                    secure: provider.tls,
                    user: provider.user.clone(),
                    pass: provider.pass.clone(),
                })
                .await;
            let validated = matches!(came_to, Validation::Valid { .. });
            (
                Answer::Provider(Some(Provider {
                    validated,
                    ..provider
                })),
                Proof {
                    credential: Some(came_to),
                    storage: None,
                },
            )
        }
        Answer::DataLocation(chosen) => {
            let found = assessed(filesystem, &chosen).await;
            (
                Answer::DataLocation(chosen),
                Proof {
                    credential: None,
                    storage: Some(found),
                },
            )
        }
        other => (other, Proof::default()),
    }
}

/// Test a location for hardlinks — empirically, never inferred from its name.
///
/// The location itself is not created here: nothing setup does before the
/// operator confirms the plan touches disk beyond the resumable progress file, so
/// where the chosen path does not exist yet its filesystem is tested through the
/// deepest parent that does. That parent's answer is only a proxy — a separate
/// drive mounted there later could differ — so a result read off a parent is
/// carried back as such rather than dressed up as the chosen path's own. A place
/// with no reachable parent cannot be tested at all, and says so.
pub(super) async fn assessed(filesystem: &dyn FileSystem, chosen: &Path) -> StorageFinding {
    let Some((base, exact)) = nearest_existing(filesystem, chosen).await else {
        return StorageFinding::Untested {
            reason: "it could not be reached, and neither could any parent of it".to_owned(),
        };
    };
    let inferred_from = (!exact).then(|| base.clone());

    match storage::test_link(filesystem, &base).await {
        Linked::Yes { .. } => StorageFinding::Links { inferred_from },
        Linked::No => {
            let facts = filesystem.describe(&base).await;
            StorageFinding::CopyOnly {
                limitation: facts.kind.limitation().map(str::to_owned),
            }
        }
        Linked::Unwritable { message } => StorageFinding::Untested { reason: message },
        Linked::Unconfirmed => StorageFinding::Untested {
            reason: "a hardlink was made but could not be confirmed to point at one file"
                .to_owned(),
        },
    }
}

/// The deepest ancestor of `path` already on disk, resolved through any symlinks,
/// and whether it is `path` itself — or nothing where not even the root of it can
/// be reached.
async fn nearest_existing(filesystem: &dyn FileSystem, path: &Path) -> Option<(PathBuf, bool)> {
    let mut exact = true;
    for ancestor in path.ancestors() {
        if let Ok(real) = filesystem.canonicalize(ancestor).await {
            return Some((real, exact));
        }
        exact = false;
    }
    None
}

#[cfg(test)]
mod tests;
