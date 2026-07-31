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
use crate::storage::{self, Linked};

/// Raised when the data root cannot hardlink, so imports must copy.
pub const COPY_ONLY: Code = Code::new("STORAGE-1");

/// Raised when the data root exists but cannot be written to.
pub const ROOT_UNWRITABLE: Code = Code::new("STORAGE-2");

/// Raised when the data root is not there to test.
pub const ROOT_ABSENT: Code = Code::new("STORAGE-3");

/// Raised when the volume holding the data root is nearly full.
pub const SPACE_LOW: Code = Code::new("STORAGE-4");

/// Raised when the data root used to hardlink and no longer does.
pub const DEGRADED: Code = Code::new("STORAGE-5");

/// Raised when the operator owns the data root but the services cannot write it.
pub const SERVICE_DENIED: Code = Code::new("STORAGE-6");

/// The capability the storage check remembers between runs, so a later run can
/// tell that a location which used to hardlink has stopped.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct Recorded {
    /// Whether the data root could hardlink when it was last checked.
    hardlinks: bool,
}

/// The free space a volume must keep clear once the queue has landed.
///
/// The check projects exhaustion from committed downloads — the free space *minus*
/// what the download clients still have to write — and warns when that projected
/// figure falls under this floor. Where no download client is
/// reachable there is nothing to subtract, so the same floor guards the raw free
/// space instead. A single large import can be tens of gigabytes and unpacking
/// needs room beside the file, so the floor sits well above one file.
const LOW_SPACE_FLOOR: u64 = 10 * 1024 * 1024 * 1024;

/// Whether the data root can hardlink, and what mode that puts the stack in.
pub struct StorageCheck {
    filesystem: Arc<dyn FileSystem>,
    root: Option<PathBuf>,
    state: Option<PathBuf>,
    environment: Environment,
    service_user: Option<(u32, u32)>,
    committed: Option<u64>,
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
    #[must_use]
    pub fn new(
        filesystem: Arc<dyn FileSystem>,
        root: Option<PathBuf>,
        state: Option<PathBuf>,
        environment: Environment,
        service_user: Option<(u32, u32)>,
        committed: Option<u64>,
    ) -> Self {
        Self {
            filesystem,
            root,
            state,
            environment,
            service_user,
            committed,
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

    async fn run(&self) -> Vec<Finding> {
        let Some(root) = &self.root else {
            return vec![skipped(
                "no data location is configured yet — run setup to choose one",
            )];
        };

        // Resolved first so the probe runs against the filesystem the data
        // actually lives on: a symlinked root would otherwise be tested on the
        // filesystem holding the link, which is not the one that matters.
        match self.filesystem.canonicalize(root).await {
            Err(fault) => absent(root, &fault.message),
            Ok(real) => self.probe(&real).await,
        }
    }
}

/// The findings when the link was made and confirmed: a pass naming how many
/// names point at the one file, and the mode a working link puts the stack in.
fn linked(facts: &StorageFacts, links: u64) -> Vec<Finding> {
    let note = format!("{}, {links} names to one file", facts.kind.label());
    pair(Verdict::Pass { note: Some(note) }, working_mode(facts))
}

/// The findings when the link was made but could not be confirmed to point at one
/// file: the capability is not disproven, but it is not proven either, and an
/// unproven guarantee is never reported as met.
fn unconfirmed() -> Vec<Finding> {
    pair(
        Verdict::Unverified {
            reason: "the link was made but could not be confirmed to point at the same file"
                .to_owned(),
            remedy: Remedy::new("Run the storage check again"),
        },
        Verdict::Skipped {
            reason: "the link could not be confirmed, so no mode was derived".to_owned(),
        },
    )
}

/// The findings when the link could not be made.
///
/// A location that used to link and now cannot is a regression, not the same
/// thing as one that never could: something changed under a running stack, and
/// every import since has quietly been copying. It is reported as its own,
/// louder finding, while a location that was never able to link stays the
/// ordinary copy-mode warning.
fn copying(facts: &StorageFacts, regressed: bool) -> Vec<Finding> {
    if regressed {
        return pair(Verdict::Fail(degraded()), degraded_mode());
    }

    let summary = match facts.kind.limitation() {
        Some(cause) => format!("This location cannot hardlink — {cause}"),
        None => "This location cannot hardlink".to_owned(),
    };
    let problem = Problem::new(
        COPY_ONLY,
        Severity::Warning,
        summary,
        storage::COPY_CONSEQUENCE,
        Remedy::new("Choose a location that hardlinks, or continue in copy mode")
            .with_detail("The services are configured to copy so imports still work"),
    )
    .in_state(State::Guided);

    pair(Verdict::Warn(problem), copy_mode(facts))
}

/// The problem for a location whose hardlink capability was lost.
fn degraded() -> Problem {
    Problem::new(
        DEGRADED,
        Severity::Error,
        "Hardlinks have stopped working here",
        "This location used to hardlink and no longer does — usually a drive that came back \
         mounted with different options. Every import since has been copying, using twice the \
         disk and leaving nothing to seed from.",
        Remedy::new("Check how the data location is mounted, and remount it as it was")
            .with_detail("A network share remounted without the right options is the common cause"),
    )
    .in_state(State::Guided)
}

/// The mode a regressed location is now in: copying, where it used to link.
fn degraded_mode() -> Verdict {
    Verdict::Warn(
        Problem::new(
            DEGRADED,
            Severity::Warning,
            "degraded — was linking, now copying",
            "The stack is running in copy mode it was not set up for, because the location \
             changed under it.",
            Remedy::new(
                "Restore the location's hardlink support, then run the storage check again",
            ),
        )
        .in_state(State::Guided),
    )
}

/// The mode a working link puts the stack in: local, or external on removable
/// media, both of which link.
fn working_mode(facts: &StorageFacts) -> Verdict {
    let note = if facts.removable {
        "external — hardlinks on removable media"
    } else {
        "local — imports hardlink instantly"
    };
    Verdict::Pass {
        note: Some(note.to_owned()),
    }
}

/// The mode a failed link puts the stack in: copy, or a network share's copy.
fn copy_mode(facts: &StorageFacts) -> Verdict {
    let note = if facts.kind.is_network() {
        "nas — imports copy across a network share"
    } else {
        "copy — imports copy; this location cannot hardlink"
    };
    Verdict::Pass {
        note: Some(note.to_owned()),
    }
}

/// Whether a user and group can write a path, by the ownership and mode of it.
///
/// The classes do not fall back on each other: a file owned by the user is
/// judged on its owner bits alone, even where the group or other bits would be
/// more permissive, because that is how the kernel decides it.
fn writable(owner: Ownership, uid: u32, gid: u32) -> bool {
    const OWNER_WRITE: u32 = 0o200;
    const GROUP_WRITE: u32 = 0o020;
    const OTHER_WRITE: u32 = 0o002;
    // Root is bound by no permission bits, so a container running as it can
    // write regardless of who owns the directory.
    if uid == 0 {
        return true;
    }
    if owner.uid == uid {
        owner.mode & OWNER_WRITE != 0
    } else if owner.gid == gid {
        owner.mode & GROUP_WRITE != 0
    } else {
        owner.mode & OTHER_WRITE != 0
    }
}

/// The service-facing permission finding, once ownership is known.
fn service_verdict(owner: Ownership, uid: u32, gid: u32) -> Finding {
    let verdict = if writable(owner, uid, gid) {
        Verdict::Pass {
            note: Some(format!("writable by the services ({uid}:{gid})")),
        }
    } else {
        Verdict::Fail(
            Problem::new(
                SERVICE_DENIED,
                Severity::Error,
                "The services cannot write to the data location",
                format!(
                    "The containers run as {uid}:{gid}, but the data location is owned by {}:{} \
                     with mode {:o} and is not writable by them. Imports fail inside the services, \
                     far from where the cause is.",
                    owner.uid, owner.gid, owner.mode
                ),
                Remedy::new(
                    "Give the service user ownership of the data location, or write access",
                )
                .with_detail(format!("chown -R {uid}:{gid} the data location")),
            )
            .in_state(State::Guided),
        )
    };
    finding("storage.permissions", "Service access", verdict)
}

/// The permission finding where it does not apply — off native Linux, or before
/// the service user is known.
fn service_skipped(reason: &str) -> Finding {
    finding(
        "storage.permissions",
        "Service access",
        Verdict::Skipped {
            reason: reason.to_owned(),
        },
    )
}

/// The permission finding where ownership could not be read.
fn service_unverified() -> Finding {
    finding(
        "storage.permissions",
        "Service access",
        Verdict::Unverified {
            reason: "the data location's ownership could not be read".to_owned(),
            remedy: Remedy::new(
                "Confirm the data location exists, then run the storage check again",
            ),
        },
    )
}

/// The free-space finding for the volume the data root sits on.
///
/// A volume that reports no size at all could not be measured rather than being
/// empty, so it is unverified rather than reported as full. Otherwise the finding
/// is a projection: the free space left once the download clients' committed
/// content has landed, warned on when that projected figure falls under the floor
/// rather than only when the volume is already nearly full. A committed figure of
/// zero — the clients quiet or unreachable — and `None` — no projection supplied at
/// all — both subtract nothing, so the same floor guards the raw free space, the
/// behaviour before a client could be read.
fn space(facts: &StorageFacts, committed: Option<u64>) -> Finding {
    if facts.total == 0 {
        return finding(
            "storage.space",
            "Free space",
            Verdict::Unverified {
                reason: "the volume's free space could not be read".to_owned(),
                remedy: Remedy::new("Run the storage check again once the location is reachable"),
            },
        );
    }

    let underway = committed.unwrap_or(0);
    let projected = facts.available.saturating_sub(underway);
    let verdict = if projected >= LOW_SPACE_FLOOR {
        Verdict::Pass {
            note: Some(free_note(facts, underway)),
        }
    } else if underway > 0 {
        // Room now, but the queue will consume it: exhaustion the operator is
        // warned of before it happens rather than once the disk is already full.
        Verdict::Warn(
            Problem::new(
                SPACE_LOW,
                Severity::Warning,
                format!(
                    "Storage is projected to run out — {} of downloads still to land, {} free",
                    humanize(underway),
                    humanize(facts.available),
                ),
                "Downloads already queued will not fit alongside the room an import and its \
                 unpacking need, so the disk fills partway through and leaves half a file behind.",
                Remedy::new(
                    "Free space on the data location, thin the download queue, or move it to a \
                     larger volume",
                ),
            )
            .in_state(State::Guided),
        )
    } else {
        // No committed content to project from, but the volume is already low.
        Verdict::Warn(
            Problem::new(
                SPACE_LOW,
                Severity::Warning,
                format!("Free space is low — {} left", humanize(facts.available)),
                "A disk that fills partway through an import leaves half a file behind and \
                 stalls the queue, so what is left has to cover what is still to come.",
                Remedy::new("Free space on the data location, or move it to a larger volume"),
            )
            .in_state(State::Guided),
        )
    };
    finding("storage.space", "Free space", verdict)
}

/// The passing note: what is free of the whole, and — where downloads are underway
/// — what is still to land, so a pass that is comfortable only because the queue is
/// small still says so.
fn free_note(facts: &StorageFacts, underway: u64) -> String {
    let free_of_total = format!(
        "{} free of {}",
        humanize(facts.available),
        humanize(facts.total)
    );
    if underway > 0 {
        format!("{free_of_total}, {} still to land", humanize(underway))
    } else {
        free_of_total
    }
}

/// A byte count as a person reads it, to one decimal place.
///
/// Binary units, because that is what the tools an operator will cross-check
/// against report, and one decimal because a library measured to the byte is
/// noise around a figure whose point is "roughly how much room is left".
fn humanize(bytes: u64) -> String {
    const UNITS: [(&str, u64); 4] = [
        ("TiB", 1 << 40),
        ("GiB", 1 << 30),
        ("MiB", 1 << 20),
        ("KiB", 1 << 10),
    ];
    for (label, size) in UNITS {
        if bytes >= size {
            // Rounded to the nearest tenth rather than truncated, so 2.0 GiB does
            // not read as 1.9. The remainder is well under a terabyte, so scaling
            // it by ten cannot overflow; a remainder that rounds up to a whole
            // unit carries into it.
            let mut whole = bytes / size;
            let mut tenths = ((bytes % size) * 10 + size / 2) / size;
            if tenths == 10 {
                whole += 1;
                tenths = 0;
            }
            return format!("{whole}.{tenths} {label}");
        }
    }
    format!("{bytes} B")
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
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    use async_trait::async_trait;

    use super::{
        humanize, writable, Check, Environment, Finding, StorageCheck, Verdict, COPY_ONLY,
        DEGRADED, ROOT_ABSENT, ROOT_UNWRITABLE, SERVICE_DENIED, SPACE_LOW,
    };
    use crate::ports::filesystem::{Fault, FileSystem, FsKind, Identity, Ownership, StorageFacts};

    /// Room to spare, so a test that is not about space never trips the floor.
    const AMPLE: u64 = 500 * 1024 * 1024 * 1024;

    /// A one-terabyte volume, the size the ample figure is free space on.
    const CAPACITY: u64 = 1024 * 1024 * 1024 * 1024;

    /// Facts for a filesystem, with a capacity a test does not otherwise care
    /// about filled in generously.
    fn facts(kind: FsKind, removable: bool) -> StorageFacts {
        StorageFacts {
            kind,
            removable,
            available: AMPLE,
            total: CAPACITY,
        }
    }

    /// A filesystem whose every answer the test scripts. Identity is asked twice
    /// — of the original and of the link — and told apart by the name asked
    /// about, so a test can make the two disagree.
    struct Bench {
        resolves: Result<PathBuf, Fault>,
        writes: Result<(), Fault>,
        original: Result<Identity, Fault>,
        links: Result<(), Fault>,
        confirmed: Result<Identity, Fault>,
        facts: StorageFacts,
        remembered: Option<String>,
        owner: Option<Ownership>,
        /// What the check wrote to the state file, so a test can assert whether —
        /// and with what — the baseline was recorded.
        recorded: Arc<std::sync::Mutex<Vec<String>>>,
    }

    impl Bench {
        /// A healthy local filesystem: resolves, writes, links, and reports the
        /// two names as one file on a filesystem that links, with room to spare
        /// and no capability remembered from before.
        fn healthy() -> Self {
            Self {
                resolves: Ok(PathBuf::from("/data")),
                writes: Ok(()),
                original: Ok(Identity { file: 7, links: 1 }),
                links: Ok(()),
                confirmed: Ok(Identity { file: 7, links: 2 }),
                facts: facts(FsKind::Linking("apfs".to_owned()), false),
                remembered: None,
                owner: None,
                recorded: Arc::new(std::sync::Mutex::new(Vec::new())),
            }
        }
    }

    #[async_trait]
    impl FileSystem for Bench {
        async fn canonicalize(&self, _path: &Path) -> Result<PathBuf, Fault> {
            self.resolves.clone()
        }
        async fn touch(&self, _path: &Path) -> Result<(), Fault> {
            self.writes.clone()
        }
        async fn link(&self, _from: &Path, _to: &Path) -> Result<(), Fault> {
            self.links.clone()
        }
        async fn identify(&self, path: &Path) -> Result<Identity, Fault> {
            if path.to_string_lossy().ends_with(".link") {
                self.confirmed.clone()
            } else {
                self.original.clone()
            }
        }
        async fn remove(&self, _path: &Path) {}
        async fn read(&self, _path: &Path) -> Option<String> {
            self.remembered.clone()
        }
        async fn write(&self, _path: &Path, contents: &str) {
            if let Ok(mut log) = self.recorded.lock() {
                log.push(contents.to_owned());
            }
        }
        async fn ownership(&self, _path: &Path) -> Option<Ownership> {
            self.owner
        }
        async fn describe(&self, _path: &Path) -> StorageFacts {
            self.facts.clone()
        }
    }

    /// Off native Linux, so the permission finding skips and the tests that are
    /// not about it are undisturbed by it.
    async fn run(bench: Bench, root: Option<&str>) -> Vec<Finding> {
        StorageCheck::new(
            Arc::new(bench),
            root.map(PathBuf::from),
            None,
            Environment::MacOs,
            None,
            None,
        )
        .run()
        .await
    }

    /// A run carrying a committed-bytes figure, so the space finding projects
    /// exhaustion from it rather than guarding the raw free space.
    async fn run_committed(bench: Bench, root: &str, committed: Option<u64>) -> Vec<Finding> {
        StorageCheck::new(
            Arc::new(bench),
            Some(PathBuf::from(root)),
            None,
            Environment::MacOs,
            None,
            committed,
        )
        .run()
        .await
    }

    /// The same run, but with a place to remember what was seen, so a regression
    /// can be noticed between one call and the next.
    async fn run_remembering(bench: Bench, root: &str) -> Vec<Finding> {
        StorageCheck::new(
            Arc::new(bench),
            Some(PathBuf::from(root)),
            Some(PathBuf::from("/state/storage-state.json")),
            Environment::MacOs,
            None,
            None,
        )
        .run()
        .await
    }

    /// A run on native Linux, where host ownership gates the services, with the
    /// service user configured.
    async fn run_on_native_linux(bench: Bench, service_user: (u32, u32)) -> Vec<Finding> {
        StorageCheck::new(
            Arc::new(bench),
            Some(PathBuf::from("/data")),
            None,
            Environment::LinuxNative,
            Some(service_user),
            None,
        )
        .run()
        .await
    }

    fn verdict<'a>(findings: &'a [Finding], check: &str) -> Option<&'a Verdict> {
        findings
            .iter()
            .find(|finding| finding.check == check)
            .map(|finding| &finding.verdict)
    }

    #[tokio::test]
    async fn a_root_that_links_passes_and_derives_the_local_mode() {
        let findings = run(Bench::healthy(), Some("/data")).await;
        assert!(matches!(
            verdict(&findings, "storage.hardlinks"),
            Some(Verdict::Pass { note: Some(note) }) if note.contains("apfs")
        ));
        assert!(matches!(
            verdict(&findings, "storage.mode"),
            Some(Verdict::Pass { note: Some(note) }) if note.contains("local")
        ));
    }

    #[tokio::test]
    async fn a_working_link_on_removable_media_is_the_external_mode() {
        let bench = Bench {
            facts: facts(FsKind::Linking("apfs".to_owned()), true),
            ..Bench::healthy()
        };
        let findings = run(bench, Some("/data")).await;
        assert!(matches!(
            verdict(&findings, "storage.mode"),
            Some(Verdict::Pass { note: Some(note) }) if note.contains("external")
        ));
    }

    #[tokio::test]
    async fn exfat_is_named_specifically_as_the_reason_it_cannot_link() {
        let bench = Bench {
            links: Err(Fault::new("operation not permitted")),
            facts: facts(FsKind::ExFat, true),
            ..Bench::healthy()
        };
        let findings = run(bench, Some("/data")).await;
        assert!(matches!(
            verdict(&findings, "storage.hardlinks"),
            Some(Verdict::Warn(problem))
                if problem.code == COPY_ONLY && problem.summary.contains("exFAT")
        ));
        assert!(matches!(
            verdict(&findings, "storage.mode"),
            Some(Verdict::Pass { note: Some(note) }) if note.contains("copy")
        ));
    }

    #[tokio::test]
    async fn a_network_share_that_cannot_link_derives_the_nas_mode() {
        let bench = Bench {
            links: Err(Fault::new("not supported")),
            facts: facts(FsKind::Nfs, false),
            ..Bench::healthy()
        };
        let findings = run(bench, Some("/data")).await;
        assert!(matches!(
            verdict(&findings, "storage.mode"),
            Some(Verdict::Pass { note: Some(note) }) if note.contains("nas")
        ));
    }

    #[tokio::test]
    async fn a_filesystem_that_does_not_link_but_names_nothing_still_warns() {
        let bench = Bench {
            links: Err(Fault::new("nope")),
            facts: facts(FsKind::Unknown("weirdfs".to_owned()), false),
            ..Bench::healthy()
        };
        let findings = run(bench, Some("/data")).await;
        assert!(matches!(
            verdict(&findings, "storage.hardlinks"),
            Some(Verdict::Warn(problem)) if problem.code == COPY_ONLY
        ));
    }

    #[tokio::test]
    async fn a_root_that_cannot_be_written_to_fails_and_derives_no_mode() {
        let bench = Bench {
            writes: Err(Fault::new("permission denied")),
            ..Bench::healthy()
        };
        let findings = run(bench, Some("/data")).await;
        assert!(matches!(
            verdict(&findings, "storage.hardlinks"),
            Some(Verdict::Fail(problem)) if problem.code == ROOT_UNWRITABLE
        ));
        assert!(matches!(
            verdict(&findings, "storage.mode"),
            Some(Verdict::Skipped { .. })
        ));
    }

    #[tokio::test]
    async fn a_root_that_cannot_be_reached_is_reported_as_absent() {
        let bench = Bench {
            resolves: Err(Fault::new("no such file or directory")),
            ..Bench::healthy()
        };
        let findings = run(bench, Some("/data")).await;
        assert!(matches!(
            verdict(&findings, "storage.hardlinks"),
            Some(Verdict::Fail(problem)) if problem.code == ROOT_ABSENT
        ));
    }

    #[tokio::test]
    async fn a_link_that_cannot_be_confirmed_is_unverified_not_passed() {
        let bench = Bench {
            confirmed: Ok(Identity {
                file: 999,
                links: 1,
            }),
            ..Bench::healthy()
        };
        let findings = run(bench, Some("/data")).await;
        assert!(matches!(
            verdict(&findings, "storage.hardlinks"),
            Some(Verdict::Unverified { .. })
        ));
    }

    #[tokio::test]
    async fn an_unconfigured_machine_is_told_to_run_setup_rather_than_shown_an_error() {
        let findings = run(Bench::healthy(), None).await;
        assert!(matches!(
            verdict(&findings, "storage"),
            Some(Verdict::Skipped { reason }) if reason.contains("setup")
        ));
    }

    #[tokio::test]
    async fn a_volume_with_room_reports_its_free_space() {
        let findings = run(Bench::healthy(), Some("/data")).await;
        assert!(matches!(
            verdict(&findings, "storage.space"),
            Some(Verdict::Pass { note: Some(note) }) if note.contains("free of")
        ));
    }

    #[tokio::test]
    async fn a_nearly_full_volume_warns_about_space() {
        let mut low = facts(FsKind::Linking("ext4".to_owned()), false);
        low.available = 2 * 1024 * 1024 * 1024;
        let findings = run(
            Bench {
                facts: low,
                ..Bench::healthy()
            },
            Some("/data"),
        )
        .await;
        assert!(matches!(
            verdict(&findings, "storage.space"),
            Some(Verdict::Warn(problem)) if problem.code == SPACE_LOW
        ));
    }

    #[tokio::test]
    async fn committed_downloads_that_will_not_fit_project_exhaustion() {
        // Room now (the healthy bench reports ample free space), but the download
        // clients still have to write nearly all of it — so what is left once the
        // queue lands is below the floor, and the warning arrives before the disk
        // actually fills.
        let committed = 495 * 1024 * 1024 * 1024;
        let findings = run_committed(Bench::healthy(), "/data", Some(committed)).await;
        assert!(matches!(
            verdict(&findings, "storage.space"),
            Some(Verdict::Warn(problem))
                if problem.code == SPACE_LOW && problem.summary.contains("projected to run out")
        ));
    }

    #[tokio::test]
    async fn committed_downloads_that_fit_pass_and_still_name_what_is_coming() {
        // The queue fits with room to spare: a pass, but the note still states what
        // is on its way so a comfortable pass that is only comfortable because the
        // queue is small still says so.
        let committed = 100 * 1024 * 1024 * 1024;
        let findings = run_committed(Bench::healthy(), "/data", Some(committed)).await;
        assert!(matches!(
            verdict(&findings, "storage.space"),
            Some(Verdict::Pass { note: Some(note) }) if note.contains("still to land")
        ));
    }

    #[tokio::test]
    async fn free_space_is_reported_even_when_the_root_cannot_be_written_to() {
        let bench = Bench {
            writes: Err(Fault::new("permission denied")),
            ..Bench::healthy()
        };
        let findings = run(bench, Some("/data")).await;
        // A full disk and an unwritable one are different problems; the operator
        // gets both rather than the write failure hiding the space figure.
        assert!(matches!(
            verdict(&findings, "storage.space"),
            Some(Verdict::Pass { .. })
        ));
    }

    #[tokio::test]
    async fn a_volume_whose_size_cannot_be_read_is_unverified_not_reported_full() {
        let mut unreadable = facts(FsKind::Linking("ext4".to_owned()), false);
        unreadable.total = 0;
        unreadable.available = 0;
        let findings = run(
            Bench {
                facts: unreadable,
                ..Bench::healthy()
            },
            Some("/data"),
        )
        .await;
        assert!(matches!(
            verdict(&findings, "storage.space"),
            Some(Verdict::Unverified { .. })
        ));
    }

    #[test]
    fn a_byte_count_reads_in_the_unit_a_person_would_use() {
        assert_eq!(humanize(0), "0 B");
        assert_eq!(humanize(512), "512 B");
        assert_eq!(humanize(1536), "1.5 KiB");
        assert_eq!(humanize(3 * 1024 * 1024), "3.0 MiB");
        assert_eq!(
            humanize(10 * 1024 * 1024 * 1024 + 512 * 1024 * 1024),
            "10.5 GiB"
        );
        assert_eq!(humanize(1024_u64.pow(4) * 2), "2.0 TiB");
        // Rounded, not truncated: a hair under two gigabytes reads as 2.0, and
        // the tenth that rounds up carries into the whole rather than showing 1.10.
        assert_eq!(humanize(2 * 1024 * 1024 * 1024 - 1), "2.0 GiB");
        assert_eq!(humanize(1024 * 1024 * 1024 + 550 * 1024 * 1024), "1.5 GiB");
    }

    #[tokio::test]
    async fn a_location_that_used_to_link_and_stopped_is_reported_as_a_regression() {
        let bench = Bench {
            links: Err(Fault::new("not supported")),
            remembered: Some(r#"{"hardlinks":true}"#.to_owned()),
            ..Bench::healthy()
        };
        let recorded = bench.recorded.clone();
        let findings = run_remembering(bench, "/data").await;
        assert!(
            matches!(
                verdict(&findings, "storage.hardlinks"),
                Some(Verdict::Fail(problem)) if problem.code == DEGRADED
            ),
            "a lost capability is louder than one that was never there"
        );
        assert!(matches!(
            verdict(&findings, "storage.mode"),
            Some(Verdict::Warn(problem)) if problem.code == DEGRADED
        ));
        // The failure is not written over the good baseline, so the next run
        // still sees it and keeps shouting rather than downgrading to a warning.
        assert!(
            recorded.lock().is_ok_and(|log| log.is_empty()),
            "a regression must not overwrite the known-good baseline"
        );
    }

    #[tokio::test]
    async fn a_location_that_never_linked_is_copy_mode_not_a_regression() {
        let bench = Bench {
            links: Err(Fault::new("not supported")),
            remembered: Some(r#"{"hardlinks":false}"#.to_owned()),
            ..Bench::healthy()
        };
        let findings = run_remembering(bench, "/data").await;
        assert!(matches!(
            verdict(&findings, "storage.hardlinks"),
            Some(Verdict::Warn(problem)) if problem.code == COPY_ONLY
        ));
    }

    #[tokio::test]
    async fn an_unreadable_baseline_is_ignored_rather_than_trusted() {
        let bench = Bench {
            links: Err(Fault::new("not supported")),
            remembered: Some("not json at all".to_owned()),
            ..Bench::healthy()
        };
        let findings = run_remembering(bench, "/data").await;
        // A corrupt baseline cannot say the capability was ever there, so it is a
        // plain copy-mode location rather than a regression against nonsense.
        assert!(matches!(
            verdict(&findings, "storage.hardlinks"),
            Some(Verdict::Warn(problem)) if problem.code == COPY_ONLY
        ));
    }

    #[tokio::test]
    async fn a_healthy_run_records_what_it_saw_for_next_time() {
        let bench = Bench::healthy();
        let recorded = bench.recorded.clone();
        let findings = run_remembering(bench, "/data").await;
        assert!(matches!(
            verdict(&findings, "storage.hardlinks"),
            Some(Verdict::Pass { .. })
        ));
        // A working link is written down, so a later run can notice if it stops.
        assert!(
            recorded
                .lock()
                .is_ok_and(|log| log.iter().any(|written| written.contains("true"))),
            "the working link is recorded"
        );
    }

    #[tokio::test]
    async fn a_link_whose_identity_reads_as_zero_is_not_taken_as_proof() {
        // A filesystem that reports no usable file identity leaves nothing to
        // compare; two zeroes are equal but prove nothing, so the link is
        // unverified rather than passed.
        let bench = Bench {
            original: Ok(Identity { file: 0, links: 1 }),
            confirmed: Ok(Identity { file: 0, links: 2 }),
            ..Bench::healthy()
        };
        let findings = run(bench, Some("/data")).await;
        assert!(matches!(
            verdict(&findings, "storage.hardlinks"),
            Some(Verdict::Unverified { .. })
        ));
    }

    #[tokio::test]
    async fn a_run_that_could_not_confirm_the_link_records_nothing() {
        // An unconfirmed result must not overwrite a known-good baseline, so the
        // recording step is skipped rather than writing an uncertain answer.
        let bench = Bench {
            confirmed: Ok(Identity {
                file: 999,
                links: 1,
            }),
            ..Bench::healthy()
        };
        let findings = run_remembering(bench, "/data").await;
        assert!(matches!(
            verdict(&findings, "storage.hardlinks"),
            Some(Verdict::Unverified { .. })
        ));
    }

    #[tokio::test]
    async fn where_docker_maps_ownership_the_permission_check_does_not_apply() {
        let findings = run(Bench::healthy(), Some("/data")).await;
        assert!(matches!(
            verdict(&findings, "storage.permissions"),
            Some(Verdict::Skipped { .. })
        ));
    }

    #[tokio::test]
    async fn an_unconfigured_service_user_skips_the_permission_check_on_native_linux() {
        let findings = StorageCheck::new(
            Arc::new(Bench::healthy()),
            Some(PathBuf::from("/data")),
            None,
            Environment::LinuxNative,
            None,
            None,
        )
        .run()
        .await;
        assert!(matches!(
            verdict(&findings, "storage.permissions"),
            Some(Verdict::Skipped { reason }) if reason.contains("not configured")
        ));
    }

    #[tokio::test]
    async fn ownership_that_cannot_be_read_leaves_the_permission_unverified() {
        // The healthy bench reports no ownership; on native Linux with a service
        // user that is the "could not read it" case.
        let findings = run_on_native_linux(Bench::healthy(), (1000, 1000)).await;
        assert!(matches!(
            verdict(&findings, "storage.permissions"),
            Some(Verdict::Unverified { .. })
        ));
    }

    #[tokio::test]
    async fn a_data_root_the_services_can_write_passes() {
        let bench = Bench {
            owner: Some(Ownership {
                uid: 1000,
                gid: 1000,
                mode: 0o755,
            }),
            ..Bench::healthy()
        };
        let findings = run_on_native_linux(bench, (1000, 1000)).await;
        assert!(matches!(
            verdict(&findings, "storage.permissions"),
            Some(Verdict::Pass { .. })
        ));
    }

    #[tokio::test]
    async fn a_root_the_operator_owns_but_the_services_cannot_write_is_its_own_failure() {
        // Owned by root, world-unwritable: the operator running the check may
        // well own it, yet the containers running as 1000:1000 cannot write, and
        // that is a different problem from the operator being unable to.
        let bench = Bench {
            owner: Some(Ownership {
                uid: 0,
                gid: 0,
                mode: 0o755,
            }),
            ..Bench::healthy()
        };
        let findings = run_on_native_linux(bench, (1000, 1000)).await;
        assert!(matches!(
            verdict(&findings, "storage.permissions"),
            Some(Verdict::Fail(problem)) if problem.code == SERVICE_DENIED
        ));
        // The hardlink finding, which speaks for the operator, is untroubled —
        // the two permission problems are reported apart.
        assert!(matches!(
            verdict(&findings, "storage.hardlinks"),
            Some(Verdict::Pass { .. })
        ));
    }

    #[test]
    fn writing_is_judged_by_the_class_that_owns_the_file() {
        let owns = |mode| Ownership {
            uid: 5,
            gid: 5,
            mode,
        };
        // Owner class, even when the group or other bits would be kinder.
        assert!(writable(owns(0o200), 5, 9));
        assert!(!writable(owns(0o077), 5, 5));
        // Group class, when the user is not the owner but shares the group.
        assert!(writable(
            Ownership {
                uid: 0,
                gid: 5,
                mode: 0o020
            },
            5,
            5
        ));
        // Other class, when neither the user nor the group owns it.
        assert!(writable(
            Ownership {
                uid: 0,
                gid: 0,
                mode: 0o002
            },
            5,
            5
        ));
        assert!(!writable(
            Ownership {
                uid: 0,
                gid: 0,
                mode: 0o750
            },
            5,
            5
        ));
        // Root is bound by no bits: a container running as uid 0 writes a
        // directory it does not own and has no mode share in.
        assert!(writable(
            Ownership {
                uid: 5,
                gid: 5,
                mode: 0o700
            },
            0,
            0
        ));
    }

    #[tokio::test]
    async fn a_leftover_probe_link_does_not_read_as_an_inability_to_hardlink() {
        // A previous run interrupted between the link and its cleanup leaves the
        // link behind. Against a real filesystem that hardlinks fine, the check
        // must clear it and still pass, not conclude the volume cannot link.
        let dir = std::env::temp_dir().join(format!(
            "lemonfiber-storage-{}-leftover",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(dir.join(".lemonfiber-hardlink-probe.link"), "stale");

        let findings = StorageCheck::new(
            Arc::new(crate::adapters::Disk),
            Some(dir.clone()),
            None,
            Environment::MacOs,
            None,
            None,
        )
        .run()
        .await;
        // A stale probe link must not be mistaken for a filesystem that cannot
        // link: the check clears it first and still passes.
        assert!(matches!(
            verdict(&findings, "storage.hardlinks"),
            Some(Verdict::Pass { .. })
        ));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
