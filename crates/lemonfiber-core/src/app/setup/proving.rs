//! The answers that are proven before they are kept.
//!
//! Three of the wizard's questions are not merely read back. A data location is
//! tested for hardlinks, because whether imports cost nothing is a property of the
//! volume rather than of what the operator typed. A credential and a Usenet
//! provider are tried against the live service, because a key that does not work
//! is worth finding out now rather than at the first download.
//!
//! Kept apart from the walk for that reason: the walk is "ask each question in
//! order", and this is "and do not take the answer on trust". The two go wrong
//! differently, and they are read for different reasons.

use std::path::{Path, PathBuf};

use crate::ports::filesystem::FileSystem;
use crate::storage::{self, Linked};
use crate::validate::{Credential, Validation, Validator};
use crate::wizard::{Answer, Indexer, Provider};

use super::{CredentialChoice, Prompt, StorageWarning};

/// Ask for a data location and test what it can do, until one is settled on.
///
/// A location that hardlinks is taken as chosen; one that cannot — or one that
/// could not be tested — is put back to the operator with what was found, to use
/// anyway or replace. The loop ends only when they accept a location, so it can
/// never wedge: the way out is always to say yes to the one in hand.
pub(super) async fn resolve_location(prompt: &dyn Prompt, filesystem: &dyn FileSystem) -> PathBuf {
    loop {
        let chosen = prompt.data_location();
        match assess(filesystem, &chosen).await {
            Assessment::Links { inferred_from } => {
                prompt.hardlinks(&chosen, inferred_from.as_deref());
                return chosen;
            }
            Assessment::Warned(warning) => {
                if prompt.storage_warning(&chosen, &warning) {
                    return chosen;
                }
            }
        }
    }
}

/// Ask for a credential and prove it against its live service, until one is
/// settled on — proven, taken unverified, or left for now.
///
/// Nothing about the credential is kept before the live test has run (a test is
/// attempted the moment one is entered), and what the test observed is shown so
/// the operator sees it succeed rather than a silent pass. A test that does not
/// prove it is told apart by cause and put back to them: try again, keep it
/// unverified — recorded as such so a later diagnosis can point at it — or leave
/// it unset. Entering nothing is a supported end, not a failure to answer.
pub(super) async fn resolve_credentials(prompt: &dyn Prompt, validator: &dyn Validator) -> Answer {
    loop {
        let Some((url, key)) = prompt.credential() else {
            return Answer::Credentials(None);
        };
        // Tested once, the moment it is entered: the service is asked, and what it
        // answered is all the operator is shown or the answer records.
        let outcome = validator
            .validate(&Credential::Indexer {
                url: url.clone(),
                key: key.clone(),
            })
            .await;
        if let Validation::Valid { observed } = &outcome {
            prompt.credential_valid(observed);
            return Answer::Credentials(Some(Indexer {
                url,
                key,
                validated: true,
            }));
        }
        // Not proven — told apart by cause and put back to the operator to act on.
        match prompt.credential_failed(&outcome) {
            CredentialChoice::Retry => {}
            CredentialChoice::Proceed => {
                return Answer::Credentials(Some(Indexer {
                    url,
                    key,
                    validated: false,
                }))
            }
            CredentialChoice::Skip => return Answer::Credentials(None),
        }
    }
}

/// Ask for a Usenet provider and prove its login over NNTP, until one is settled
/// on — proven, taken unverified, or left for now. The same shape as the indexer,
/// against a different transport: nothing is kept before the login is attempted,
/// and a login that does not take is put back to the operator by cause.
pub(super) async fn resolve_provider(prompt: &dyn Prompt, validator: &dyn Validator) -> Answer {
    loop {
        let Some(entry) = prompt.usenet_provider() else {
            return Answer::Provider(None);
        };
        let outcome = validator
            .validate(&Credential::Usenet {
                host: entry.host.clone(),
                port: entry.port,
                secure: entry.tls,
                user: entry.user.clone(),
                pass: entry.pass.clone(),
            })
            .await;
        let kept = |validated| {
            Answer::Provider(Some(Provider {
                host: entry.host.clone(),
                port: entry.port,
                user: entry.user.clone(),
                pass: entry.pass.clone(),
                tls: entry.tls,
                validated,
            }))
        };
        if let Validation::Valid { observed } = &outcome {
            prompt.credential_valid(observed);
            return kept(true);
        }
        match prompt.credential_failed(&outcome) {
            CredentialChoice::Retry => {}
            CredentialChoice::Proceed => return kept(false),
            CredentialChoice::Skip => return Answer::Provider(None),
        }
    }
}

/// What testing a prospective data location for hardlinks came to.
enum Assessment {
    /// The location hardlinks. `inferred_from` is the ancestor the test actually
    /// ran against where the location itself did not exist yet to be tested — so a
    /// result read off a parent is never presented as if the chosen path were
    /// proven. `None` where the chosen path was tested directly.
    Links { inferred_from: Option<PathBuf> },
    /// The location is usable only with a caveat the operator must weigh.
    Warned(StorageWarning),
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
async fn assess(filesystem: &dyn FileSystem, chosen: &Path) -> Assessment {
    let Some((base, exact)) = nearest_existing(filesystem, chosen).await else {
        return Assessment::Warned(StorageWarning::Untested {
            reason: "it could not be reached, and neither could any parent of it".to_owned(),
        });
    };
    // A parent tested in the location's place is the inferred case; the location
    // itself, where it already exists, is a direct result.
    let inferred_from = (!exact).then(|| base.clone());

    match storage::test_link(filesystem, &base).await {
        Linked::Yes { .. } => Assessment::Links { inferred_from },
        Linked::No => {
            let facts = filesystem.describe(&base).await;
            Assessment::Warned(StorageWarning::CopyOnly {
                limitation: facts.kind.limitation().map(str::to_owned),
            })
        }
        Linked::Unwritable { message } => {
            Assessment::Warned(StorageWarning::Untested { reason: message })
        }
        Linked::Unconfirmed => Assessment::Warned(StorageWarning::Untested {
            reason: "a hardlink was made but could not be confirmed to point at one file"
                .to_owned(),
        }),
    }
}

/// The deepest ancestor of `path` already on disk, resolved through any symlinks,
/// and whether it is `path` itself — or nothing where not even the root of it can
/// be reached.
///
/// The flag is how a caller tells a location it tested directly from one whose
/// answer it had to read off a parent, since the chosen leaf does not exist yet.
async fn nearest_existing(filesystem: &dyn FileSystem, path: &Path) -> Option<(PathBuf, bool)> {
    let mut exact = true;
    for ancestor in path.ancestors() {
        if let Ok(real) = filesystem.canonicalize(ancestor).await {
            return Some((real, exact));
        }
        // Past the first step the resolved place is a parent, not the path asked
        // about, so a link proven there is inferred rather than direct.
        exact = false;
    }
    None
}
