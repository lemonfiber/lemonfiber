//! Proving the data root can hardlink, rather than trusting that it can.
//!
//! The whole media pipeline rests on one property: downloads and the library
//! share a filesystem, so importing a file links it rather than copying it.
//! When that breaks nothing announces it — imports still succeed, the library
//! still fills — and the only symptoms are a disk consuming twice what it should
//! and torrents that cannot seed from the library copy, both found late.
//!
//! So capability is measured, never inferred: create a file under the data root,
//! link it, and check the two names point at one underlying file. A filesystem's
//! type only names *why* a link failed once the link has been tried — exFAT
//! cannot link at all, a network share usually cannot — and that name is added
//! to the finding rather than standing in for the test.
//!
//! The mode the stack should run in follows from the result. It is derived here,
//! never chosen: the operator picks a location, and this determines what the
//! location can do.
//!
//! See `.docs/architecture/module-layout.md`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::{Category, Check, Finding, Verdict};
use crate::error::{Code, Problem, Remedy, Severity, State};
use crate::platform::Environment;
use crate::ports::filesystem::{FileSystem, Ownership, StorageFacts};
use crate::stack::mounts::Crowded;
use crate::storage::{self, Linked};

mod findings;
mod mounts;
mod space;

pub use findings::{COPY_ONLY, DEGRADED, ROOT_ABSENT, ROOT_UNWRITABLE, SERVICE_DENIED};
pub use mounts::SPLIT_MOUNTS;
pub use space::{LOW_SPACE_FLOOR, SPACE_LOW};

use findings::{
    copying, linked, service_skipped, service_unverified, service_verdict, unconfirmed,
};
use space::space;

/// The capability the storage check remembers between runs, so a later run can
/// tell that a location which used to hardlink has stopped.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct Recorded {
    /// Whether the data root could hardlink when it was last checked.
    hardlinks: bool,
}

/// Whether the data root can hardlink, and what mode that puts the stack in.
pub struct StorageCheck {
    filesystem: Arc<dyn FileSystem>,
    root: Option<PathBuf>,
    state: Option<PathBuf>,
    environment: Environment,
    service_user: Option<(u32, u32)>,
    committed: Option<u64>,
    crowded: Vec<Crowded>,
}

impl StorageCheck {
    /// A storage check over the given filesystem, data root, the file where it
    /// remembers what it last saw, the platform, and the user the services run
    /// as.
    ///
    /// The root is optional because a machine that has not been set up has not
    /// chosen one yet, and an operator in that state is told to run setup rather
    /// than shown an error about a path they never picked. The state file is
    /// optional too: without it the check still runs, it just cannot notice a
    /// capability that was there before and is now gone. The platform and service
    /// user decide the permission finding: host ownership only gates the services
    /// where Docker does not map it away, and only once a service user is known.
    ///
    /// `committed` is how many bytes the download clients still have to write — what
    /// the free-space finding projects exhaustion from. A diagnosis always supplies
    /// a figure, zero where the clients are quiet or unreachable; `None` is the
    /// "no projection at all" case a caller with no clients to read passes, and
    /// guards the raw free space exactly as a zero would.
    ///
    /// `crowded` is what reading the stack's own compose files found: the services
    /// that would see more than one mount beneath the data location. It arrives read
    /// rather than read here, because the files are the stack's business and this
    /// check's seam is the filesystem — but it is reported here, because it answers
    /// the same question the probe does and a separate heading would let an operator
    /// read one half without the other.
    #[must_use]
    pub fn new(
        filesystem: Arc<dyn FileSystem>,
        root: Option<PathBuf>,
        state: Option<PathBuf>,
        environment: Environment,
        service_user: Option<(u32, u32)>,
        committed: Option<u64>,
        crowded: Vec<Crowded>,
    ) -> Self {
        Self {
            filesystem,
            root,
            state,
            environment,
            service_user,
            committed,
            crowded,
        }
    }

    /// Run the probe against a resolved data root and read what it proved.
    ///
    /// The volume is described first, so its free space is reported alongside
    /// even a root that cannot be written to — a full disk and an unwritable one
    /// are different problems, and the operator is owed both answers at once.
    ///
    /// The capability seen last time is loaded before the finding is built, so a
    /// location that used to link and no longer does is reported as a regression
    /// rather than as an ordinary copy-mode location, and the current result is
    /// recorded for the next run to compare against.
    async fn probe(&self, real: &Path) -> Vec<Finding> {
        let facts = self.filesystem.describe(real).await;
        let space = space(&facts, self.committed);
        let permissions = self.permissions(real).await;

        // The empirical create-link-inspect lives above the filesystem port, in the
        // one place both this check and setup ask it, so the two can never disagree
        // about what "can hardlink" means. What is left here is turning that one
        // answer into the findings a diagnosis reports.
        let (mut findings, current) = match storage::test_link(self.filesystem.as_ref(), real).await
        {
            Linked::Unwritable { message } => (unwritable(&message), None),
            // A location that used to link and now cannot is a regression, told from
            // one that never could by what the last run recorded.
            Linked::No => (
                copying(&facts, self.remembered().await == Some(true)),
                Some(false),
            ),
            Linked::Yes { links } => (linked(&facts, links), Some(true)),
            Linked::Unconfirmed => (unconfirmed(), None),
        };

        // Only a working link is written down (inside `remember`), so a lost
        // capability keeps reporting degraded rather than settling into an ordinary
        // copy-mode warning on its second run.
        self.remember(current).await;
        findings.push(space);
        findings.push(permissions);
        findings
    }

    /// Whether the user the services run as can write the data root — a problem
    /// distinct from the operator being unable to, and reported apart from it.
    ///
    /// Only meaningful where ownership is real. Docker Desktop maps it away, so
    /// on every platform but native Linux the host's permissions do not gate the
    /// containers and the finding is skipped rather than guessed at.
    async fn permissions(&self, real: &Path) -> Finding {
        if self.environment != Environment::LinuxNative {
            return service_skipped(
                "Docker maps file ownership on this platform, so the host's permissions do not \
                 gate the services",
            );
        }
        let Some((uid, gid)) = self.service_user else {
            return service_skipped("the user the services run as is not configured yet");
        };
        match self.filesystem.ownership(real).await {
            None => service_unverified(),
            Some(owner) => service_verdict(owner, uid, gid),
        }
    }

    /// The hardlink capability recorded on the last run, where there is a state
    /// file and it holds a reading.
    async fn remembered(&self) -> Option<bool> {
        let path = self.state.as_ref()?;
        let text = self.filesystem.read(path).await?;
        serde_json::from_str::<Recorded>(&text)
            .ok()
            .map(|recorded| recorded.hardlinks)
    }

    /// Record a working link, so a later run can notice the capability was lost.
    ///
    /// Only a success is written. A failure is never recorded over a known-good
    /// baseline: a location that has regressed keeps reporting degraded until it
    /// links again, rather than quietly settling into an ordinary copy-mode
    /// warning on its second run. A result that could not be determined is not
    /// written either.
    async fn remember(&self, current: Option<bool>) {
        let (Some(path), Some(true)) = (self.state.as_ref(), current) else {
            return;
        };
        let text = serde_json::to_string(&Recorded { hardlinks: true }).unwrap_or_default();
        self.filesystem.write(path, &text).await;
    }
}

#[async_trait]
impl Check for StorageCheck {
    fn category(&self) -> Category {
        Category::Storage
    }

    /// Longer than a check bounded by a container command. This one waits on the
    /// operator's own storage, which may be a network share or a drive that has
    /// spun down, and calling merely slow hardware unreadable sends them to
    /// diagnose a disk that is working.
    fn budget(&self) -> std::time::Duration {
        super::FILESYSTEM_BUDGET
    }

    async fn run(&self) -> Vec<Finding> {
        ran(self).await
    }
}

/// Whether the data location is there, writable, and on one filesystem.
async fn ran(check: &StorageCheck) -> Vec<Finding> {
    let mut found = match &check.root {
        None => vec![skipped(
            "no data location is configured yet — run setup to choose one",
        )],
        // Resolved first so the probe runs against the filesystem the data
        // actually lives on: a symlinked root would otherwise be tested on the
        // filesystem holding the link, which is not the one that matters.
        Some(root) => match check.filesystem.canonicalize(root).await {
            Err(fault) => absent(root, &fault.message),
            Ok(real) => check.probe(&real).await,
        },
    };
    // The same question asked of the container's view, which the probe above cannot
    // reach from here: on this machine the data location is one filesystem and links
    // work perfectly, and a stack that mounts the downloads and the library separately
    // has put a boundary between them that exists only inside the container. Reported
    // whatever the probe found, since a location that could not be reached at all is
    // still a stack whose layout an operator can be told about.
    found.extend(mounts::findings(&check.crowded));
    found
}

/// The findings when the data root could not be reached at all.
fn absent(root: &Path, detail: &str) -> Vec<Finding> {
    let problem = Problem::new(
        ROOT_ABSENT,
        Severity::Error,
        format!("The data location {} could not be reached", root.display()),
        "Nothing can be stored where there is no reachable directory, and a stack that \
         wrote into a missing mount point would build a phantom library on the system disk.",
        Remedy::new("Check the location exists and any drive holding it is connected"),
    )
    .in_state(State::Guided)
    .with_detail(detail.to_owned());

    pair(
        Verdict::Fail(problem),
        Verdict::Skipped {
            reason: "the data location could not be reached, so no mode was derived".to_owned(),
        },
    )
}

/// The findings when the data root is present but cannot be written to.
fn unwritable(detail: &str) -> Vec<Finding> {
    let problem = Problem::new(
        ROOT_UNWRITABLE,
        Severity::Error,
        "The data location cannot be written to",
        "The services run as a user that has to own what they import, so a data root they \
         cannot write to fails every import far from where the cause shows.",
        Remedy::new("Give the account that runs the services write access to the data location"),
    )
    .in_state(State::Guided)
    .with_detail(detail.to_owned());

    pair(
        Verdict::Fail(problem),
        Verdict::Skipped {
            reason: "the data location could not be written to, so no mode was derived".to_owned(),
        },
    )
}

/// The two findings a storage run reports: the hardlink capability, and the mode
/// derived from it.
fn pair(hardlinks: Verdict, mode: Verdict) -> Vec<Finding> {
    vec![
        finding("storage.hardlinks", "Hardlinks", hardlinks),
        finding("storage.mode", "Storage mode", mode),
    ]
}

/// A finding in the storage category.
fn finding(check: &str, title: &str, verdict: Verdict) -> Finding {
    Finding::in_category(Category::Storage, check, title, verdict)
}

/// A single finding for a check that does not apply.
fn skipped(reason: &str) -> Finding {
    finding(
        "storage",
        "Storage",
        Verdict::Skipped {
            reason: reason.to_owned(),
        },
    )
}

#[cfg(test)]
mod tests;
