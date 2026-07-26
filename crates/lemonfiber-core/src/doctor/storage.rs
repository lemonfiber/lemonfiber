//! Proving the filesystem can hardlink, rather than trusting that it can.
//!
//! The stack's one rule is that downloads and media share a mount, so an import
//! links instead of copies: instant, free, and leaving the file seedable. When
//! that breaks nothing announces it — imports still succeed, the library still
//! fills, and the only symptoms are a disk consuming twice what it should and
//! torrents that cannot seed. The exceptions are common: exFAT cannot hardlink
//! at all, SMB on macOS will not expose links usably, and the Windows side of
//! the WSL2 boundary breaks them.
//!
//! So the capability is *tested*, never inferred: create a file on the operator's
//! real volume, hardlink it, and compare inode and link count. A filesystem type
//! is a hint; a successful link is a fact. This is why there is no filesystem
//! port — a fake would happily report links working on a volume where they do
//! not, which is precisely the silent degradation this exists to catch. The IO
//! runs against the real disk; only the reading of its result is pure, so every
//! consequence a machine cannot reproduce is still exercised in a test.
//!
//! What breaking hardlinks *means* is stated in consequences an operator can act
//! on — copy instead of link, twice the disk, no seeding — not as the property
//! "hardlinks unsupported", which means nothing to most people.
//!
//! See `.docs/architecture/storage-probe.md`.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use async_trait::async_trait;

use super::{Category, Check, Finding, Verdict};
use crate::error::{Code, Problem, Remedy, Severity, State};
use crate::platform::{Environment, HostOs, HOST_OS};

/// Raised when the chosen data root is not present.
pub const DATA_ROOT_MISSING: Code = Code::new("STO-1");

/// Raised when the data root is present but cannot be written to.
pub const DATA_ROOT_UNWRITABLE: Code = Code::new("STO-2");

/// Raised when the data root is present and writable but cannot hardlink.
pub const HARDLINKS_UNAVAILABLE: Code = Code::new("STO-3");

/// The filesystem types that cannot hardlink and never will, whatever is tried.
///
/// FAT and its descendants have no concept of a second name for one file, so the
/// remedy is a different location rather than a different option — a distinction
/// worth stating, because "unsupported" reads as "not turned on yet".
const CANNOT_HARDLINK_EVER: [&str; 3] = ["exfat", "vfat", "msdos"];

/// The filesystem types reached over a network, where hardlinks are a property
/// of the export rather than the mount.
const NETWORK_FILESYSTEMS: [&str; 6] = ["nfs", "nfs4", "cifs", "smbfs", "smb3", "afpfs"];

/// The filesystem types by which a Windows volume shows through the WSL2 boundary.
///
/// A data root reached this way is on the Windows side; hardlinks break for
/// anything crossing back, and the fix is to keep the data inside the Linux
/// filesystem rather than to change a mount option.
const WSL2_BOUNDARY_FILESYSTEMS: [&str; 2] = ["9p", "drvfs"];

/// Tests whether the operator's data root can hardlink, and says what it means.
pub struct StorageCheck {
    /// The volume to probe, absent until setup has chosen one.
    root: Option<PathBuf>,
    /// Which environment this is, so a limitation can be named for the operator's
    /// actual platform where the filesystem type alone does not settle it.
    environment: Environment,
}

impl StorageCheck {
    /// A check over the chosen data root, in the given environment.
    #[must_use]
    pub const fn new(root: Option<PathBuf>, environment: Environment) -> Self {
        Self { root, environment }
    }

    /// Look at the real volume: resolve it, learn what backs it, and try to link.
    ///
    /// The only impure step. It resolves symlinks first, so the test runs against
    /// the real filesystem rather than one a link points away from, and hands
    /// back plain values the pure reading below turns into a finding.
    fn observe(root: &Path) -> Observation {
        match std::fs::canonicalize(root) {
            Err(err) => Observation::Absent {
                kind: err.kind(),
                detail: err.to_string(),
            },
            Ok(real) => Observation::Present {
                medium: classify_medium(&real, &mount_table(), HOST_OS),
                outcome: attempt_link(&probe_paths(&real)),
            },
        }
    }
}

#[async_trait]
impl Check for StorageCheck {
    fn category(&self) -> Category {
        Category::Storage
    }

    async fn run(&self) -> Vec<Finding> {
        let Some(root) = self.root.as_deref() else {
            return vec![skipped()];
        };
        vec![interpret(&Self::observe(root), self.environment)]
    }
}

/// What the operating system reports for a file, enough to prove a hardlink.
///
/// Two names for one file share an inode and each raise the link count; a
/// filesystem that accepts the call but reports neither has not made a hardlink,
/// whatever it returned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FileId {
    /// The inode, which two hardlinks to one file share.
    inode: u64,
    /// How many names resolve to this file's data.
    links: u64,
}

/// What trying to make a hardlink under the data root came to.
#[derive(Debug, Clone, PartialEq, Eq)]
enum LinkOutcome {
    /// A file and a link to it were made and inspected.
    Linked {
        /// The identity of the original file.
        original: FileId,
        /// The identity of the link.
        link: FileId,
    },
    /// The original file could not be created — the volume is not writable.
    CouldNotWrite {
        /// The operating system's own words.
        detail: String,
    },
    /// The file was written but could not be hardlinked — the capability is
    /// absent, whatever the reason the operating system gave.
    CouldNotLink {
        /// The operating system's own words.
        detail: String,
    },
    /// The link was made but its identity could not be read back, so the result
    /// cannot be trusted either way.
    CouldNotInspect {
        /// The operating system's own words.
        detail: String,
    },
}

/// What kind of storage backs the data root, as far as the system will say.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct Medium {
    /// The filesystem type, lowercased, where one could be read.
    fstype: Option<String>,
    /// Whether the store is reached over a network.
    network: bool,
    /// Whether the store is a Windows volume seen across the WSL2 boundary.
    boundary: bool,
}

/// Everything observing the volume established, ready to be read into a finding.
enum Observation {
    /// The data root resolved, and this is what backs it and what linking did.
    Present {
        /// What backs the volume.
        medium: Medium,
        /// What trying to hardlink came to.
        outcome: LinkOutcome,
    },
    /// The data root could not be resolved at all.
    Absent {
        /// Why it could not be resolved.
        kind: io::ErrorKind,
        /// The operating system's own words.
        detail: String,
    },
}

/// The next unique suffix for a probe file, so two runs never collide.
static PROBE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// The pair of paths a single probe uses, unique to this process and call.
///
/// A dotfile prefix keeps it out of a casual listing, and the process id plus a
/// counter keep two probes — or two lemonfiber processes — from choosing the
/// same name and fighting over it.
fn probe_paths(dir: &Path) -> (PathBuf, PathBuf) {
    let sequence = PROBE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let base = format!(
        ".lemonfiber-hardlink-probe.{}.{sequence}",
        std::process::id()
    );
    let link = format!("{base}.link");
    (dir.join(base), dir.join(link))
}

/// Create a file, hardlink it, read both identities, and clean up.
///
/// Each step names the failure it is: a write that fails is a writability
/// problem, a link that fails where the write did not is a capability one, and
/// the two have entirely different remedies. Cleanup runs whatever happened, so a
/// probe leaves nothing behind on the operator's volume.
fn attempt_link((original, link): &(PathBuf, PathBuf)) -> LinkOutcome {
    let outcome = if let Err(err) = std::fs::File::create(original) {
        LinkOutcome::CouldNotWrite {
            detail: err.to_string(),
        }
    } else if let Err(err) = std::fs::hard_link(original, link) {
        LinkOutcome::CouldNotLink {
            detail: err.to_string(),
        }
    } else {
        inspect(original, link)
    };
    let _ = std::fs::remove_file(original);
    let _ = std::fs::remove_file(link);
    outcome
}

/// Read back the identities of the two names, or say they could not be read.
///
/// Split out so the unreadable case — a link made but not stattable — is a line
/// a test can reach by inspecting paths that are not there, rather than one that
/// waits on a disk fault that never comes.
fn inspect(original: &Path, link: &Path) -> LinkOutcome {
    match (identify(original), identify(link)) {
        (Ok(original), Ok(link)) => LinkOutcome::Linked { original, link },
        // An or-pattern binds whichever name would not stat, so the detail is
        // always a real error rather than an unreachable default nothing covers.
        (Err(err), _) | (_, Err(err)) => LinkOutcome::CouldNotInspect {
            detail: err.to_string(),
        },
    }
}

/// The inode and link count the operating system reports for a path.
#[cfg(unix)]
fn identify(path: &Path) -> io::Result<FileId> {
    use std::os::unix::fs::MetadataExt as _;
    let metadata = std::fs::metadata(path)?;
    Ok(FileId {
        inode: metadata.ino(),
        links: metadata.nlink(),
    })
}

/// The inode and link count, where the platform does not expose them.
///
/// A native Windows build cannot read either through the standard library; the
/// data root belongs inside the WSL2 filesystem there anyway, which the reading
/// below says in as many words.
#[cfg(not(unix))]
fn identify(_path: &Path) -> io::Result<FileId> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "file identity is not available on this platform",
    ))
}

/// The kernel's table of what is mounted where, or nothing where it cannot be
/// read.
///
/// Read only to *name* a failure the probe already established — never to decide
/// capability, which the probe alone is trusted for. Absent outside Linux, where
/// the pure classifier returns an unknown medium and the environment names the
/// platform instead.
fn mount_table() -> String {
    std::fs::read_to_string("/proc/self/mountinfo").unwrap_or_default()
}

/// What backs `path`, read from the mount table for the platform it came from.
///
/// Pure over its inputs — the path, the table text, and the host — so every
/// platform's reading is reachable from any one machine. Only Linux publishes a
/// table in this shape; elsewhere the medium is unknown and the environment
/// carries the naming.
fn classify_medium(path: &Path, table: &str, host: HostOs) -> Medium {
    if host != HostOs::Linux {
        return Medium::default();
    }
    let mut best: Option<(usize, &str)> = None;
    for line in table.lines() {
        let Some((before, after)) = line.split_once(" - ") else {
            continue;
        };
        let Some(mount_point) = before.split_whitespace().nth(4) else {
            continue;
        };
        let Some(fstype) = after.split_whitespace().next() else {
            continue;
        };
        if path.starts_with(mount_point) && best.is_none_or(|(len, _)| mount_point.len() > len) {
            best = Some((mount_point.len(), fstype));
        }
    }
    best.map_or_else(Medium::default, |(_, fstype)| Medium::of(fstype))
}

impl Medium {
    /// The medium a filesystem type names.
    fn of(fstype: &str) -> Self {
        let fstype = fstype.to_ascii_lowercase();
        Self {
            network: NETWORK_FILESYSTEMS.contains(&fstype.as_str()),
            boundary: WSL2_BOUNDARY_FILESYSTEMS.contains(&fstype.as_str()),
            fstype: Some(fstype),
        }
    }

    /// Whether this type can never hardlink, whatever is tried.
    fn cannot_hardlink_ever(&self) -> bool {
        self.fstype
            .as_deref()
            .is_some_and(|fstype| CANNOT_HARDLINK_EVER.contains(&fstype))
    }
}

/// Read the observation into the one finding it amounts to.
///
/// The whole judgement lives here and nothing above it decides anything, so the
/// cases a real disk will not reproduce on demand — a network export, an exFAT
/// stick, the far side of WSL2 — are all reachable by handing this the values
/// they would produce.
fn interpret(observation: &Observation, environment: Environment) -> Finding {
    let verdict = match observation {
        Observation::Absent { kind, detail } if *kind == io::ErrorKind::NotFound => {
            Verdict::Fail(unavailable(detail))
        }
        Observation::Absent { detail, .. } => unreachable_root(detail),
        Observation::Present { outcome, medium } => present(outcome, medium, environment),
    };
    finding(verdict)
}

/// The verdict for a data root that resolved, from what backs it and what
/// linking did.
fn present(outcome: &LinkOutcome, medium: &Medium, environment: Environment) -> Verdict {
    match outcome {
        LinkOutcome::Linked { original, link } if is_hardlink(*original, *link) => Verdict::Pass {
            note: Some(
                "hardlinks work — imports will link, instantly and without a second copy"
                    .to_owned(),
            ),
        },
        LinkOutcome::Linked { .. } => Verdict::Unverified {
            reason: "the volume accepted a link but did not report it as one".to_owned(),
            remedy: Remedy::new(
                "Run the check again; if it persists, treat this volume as copy-only",
            ),
        },
        LinkOutcome::CouldNotWrite { detail } => Verdict::Fail(unwritable(detail)),
        LinkOutcome::CouldNotLink { .. } => Verdict::Warn(copy_mode(medium, environment)),
        LinkOutcome::CouldNotInspect { detail } => Verdict::Unverified {
            reason: format!("the link could not be inspected: {detail}"),
            remedy: Remedy::new("Run the check again once the volume is responding"),
        },
    }
}

/// Whether the two names are genuinely one file: same inode, and a raised count.
fn is_hardlink(original: FileId, link: FileId) -> bool {
    original.inode == link.inode && link.links >= 2
}

/// The finding a probe becomes, always under the one stable identifier.
fn finding(verdict: Verdict) -> Finding {
    Finding {
        check: "storage.hardlinks".to_owned(),
        category: Category::Storage,
        title: "Hardlink capability".to_owned(),
        verdict,
    }
}

/// The finding for a data root nowhere to be found.
fn unavailable(detail: &str) -> Problem {
    Problem::new(
        DATA_ROOT_MISSING,
        Severity::Error,
        "The data root is not there",
        "The location downloads and media share could not be found. Services started against it would write into an empty mount point, so nothing should run until it is present.",
        Remedy::new("Check the drive is connected and the path is right, then run this again"),
    )
    .in_state(State::Guided)
    .with_detail(detail.to_owned())
}

/// The verdict for a data root that could not be reached for some other reason.
fn unreachable_root(detail: &str) -> Verdict {
    Verdict::Unverified {
        reason: format!("the data root could not be resolved: {detail}"),
        remedy: Remedy::new("Check the path is reachable and readable, then run this again"),
    }
}

/// The finding for a data root present but not writable.
fn unwritable(detail: &str) -> Problem {
    Problem::new(
        DATA_ROOT_UNWRITABLE,
        Severity::Error,
        "The data root cannot be written to",
        "The location is there, but a file could not be created in it, so services could not import into it either. This is a permissions problem rather than a missing drive.",
        Remedy::new("Give yourself write access to the data root, then run this again"),
    )
    .in_state(State::Guided)
    .with_detail(detail.to_owned())
}

/// The finding for a volume that works but cannot hardlink, stated as what the
/// operator will live with rather than as a filesystem property.
///
/// The consequence is the same wherever the type sits; what changes is the
/// remedy, and whether it is worth trying one at all — which is why the specific
/// cause is named where it can be.
fn copy_mode(medium: &Medium, environment: Environment) -> Problem {
    let consequence = "Imports will copy instead of link: each takes minutes rather than being instant, uses twice the disk while it runs, and torrents cannot seed from the library copy.";
    let (summary, remedy) = limitation(medium, environment);
    Problem::new(
        HARDLINKS_UNAVAILABLE,
        Severity::Warning,
        summary,
        consequence,
        remedy,
    )
    .in_state(State::Guided)
}

/// The specifically-named cause and its remedy, from the filesystem type first
/// and the platform where the type alone does not settle it.
fn limitation(medium: &Medium, environment: Environment) -> (String, Remedy) {
    if medium.cannot_hardlink_ever() {
        return (
            "This volume is exFAT or FAT, which cannot hardlink at all".to_owned(),
            Remedy::new(
                "Choose a location on a native filesystem — this cannot be worked around on FAT",
            ),
        );
    }
    if medium.boundary || matches!(environment, Environment::Windows) {
        return (
            "This data root is on the Windows side of the WSL2 boundary, where hardlinks break"
                .to_owned(),
            Remedy::new("Move the data root inside the WSL2 filesystem, not a Windows drive"),
        );
    }
    if medium.network && matches!(environment, Environment::MacOs) {
        return (
            "This is an SMB mount on macOS, which will not expose hardlinks usably".to_owned(),
            Remedy::new("Mount the share over NFS instead, where hardlinks work"),
        );
    }
    if medium.network {
        return (
            "This is a network mount, where hardlinks depend on the export rather than the mount"
                .to_owned(),
            Remedy::new(
                "Use a local volume, or an export that supports hardlinks, for the data root",
            ),
        );
    }
    (
        "This volume cannot hardlink".to_owned(),
        Remedy::new("Choose a location that supports hardlinks, or continue in copy mode"),
    )
}

/// The finding for a check with no data root to probe yet.
fn skipped() -> Finding {
    finding(Verdict::Skipped {
        reason: "no data root has been chosen yet".to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{
        attempt_link, classify_medium, copy_mode, inspect, interpret, limitation, present,
        probe_paths, Environment, FileId, HostOs, LinkOutcome, Medium, Observation, StorageCheck,
        Verdict, DATA_ROOT_MISSING, DATA_ROOT_UNWRITABLE,
    };
    use crate::doctor::{Category, Check};

    /// A fresh temporary directory that removes itself when the guard drops.
    ///
    /// A guard rather than a bare path so a probe's own files and the directory
    /// are gone whatever a test does, on the pattern the config store uses.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(tag: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "lemonfiber-storage-{tag}-{}-{}",
                std::process::id(),
                counter()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            let created = std::fs::create_dir_all(&dir);
            assert!(created.is_ok(), "the scratch directory should be creatable");
            Self(dir)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            // Restore write access first: a test that dropped it to force a
            // failure would otherwise leave a directory nothing can remove.
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt as _;
                let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(0o700));
            }
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// A monotonic tag so two scratch directories in one test never collide.
    fn counter() -> u64 {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        NEXT.fetch_add(1, Ordering::Relaxed)
    }

    fn id(inode: u64, links: u64) -> FileId {
        FileId { inode, links }
    }

    // --- The pure reading: every consequence, reachable without the disk. ---

    #[test]
    fn a_working_link_passes_and_says_imports_will_link() {
        let observation = Observation::Present {
            medium: Medium::default(),
            outcome: LinkOutcome::Linked {
                original: id(42, 2),
                link: id(42, 2),
            },
        };
        assert!(matches!(
            interpret(&observation, Environment::LinuxNative).verdict,
            Verdict::Pass { note: Some(note) } if note.contains("link")
        ));
    }

    #[test]
    fn a_link_that_is_not_really_a_link_is_unverified_not_passed() {
        // Same call returned, but the inode and count betray a copy: the honest
        // answer is that capability was not established, never a pass.
        let outcome = LinkOutcome::Linked {
            original: id(1, 1),
            link: id(2, 1),
        };
        assert!(matches!(
            present(&outcome, &Medium::default(), Environment::LinuxNative),
            Verdict::Unverified { .. }
        ));
    }

    #[test]
    fn a_missing_data_root_fails_because_services_would_write_into_nothing() {
        let observation = Observation::Absent {
            kind: std::io::ErrorKind::NotFound,
            detail: "no such file or directory".to_owned(),
        };
        assert!(matches!(
            interpret(&observation, Environment::LinuxNative).verdict,
            Verdict::Fail(problem) if problem.code == DATA_ROOT_MISSING
        ));
    }

    #[test]
    fn a_data_root_that_will_not_resolve_for_another_reason_is_unverified() {
        let observation = Observation::Absent {
            kind: std::io::ErrorKind::PermissionDenied,
            detail: "permission denied".to_owned(),
        };
        assert!(matches!(
            interpret(&observation, Environment::LinuxNative).verdict,
            Verdict::Unverified { .. }
        ));
    }

    #[test]
    fn an_unwritable_data_root_fails_as_a_permissions_problem() {
        let outcome = LinkOutcome::CouldNotWrite {
            detail: "permission denied".to_owned(),
        };
        assert!(matches!(
            present(&outcome, &Medium::default(), Environment::LinuxNative),
            Verdict::Fail(problem) if problem.code == DATA_ROOT_UNWRITABLE
        ));
    }

    #[test]
    fn an_uninspectable_link_is_unverified() {
        let outcome = LinkOutcome::CouldNotInspect {
            detail: "stale file handle".to_owned(),
        };
        assert!(matches!(
            present(&outcome, &Medium::default(), Environment::LinuxNative),
            Verdict::Unverified { reason, .. } if reason.contains("stale file handle")
        ));
    }

    #[test]
    fn a_failed_link_is_a_warning() {
        // The copy-mode arm of `present` itself, kept to a `matches!` so no arm
        // goes uncovered; the wording is asserted on `copy_mode` directly below.
        let outcome = LinkOutcome::CouldNotLink {
            detail: "operation not permitted".to_owned(),
        };
        assert!(matches!(
            present(&outcome, &Medium::default(), Environment::LinuxNative),
            Verdict::Warn(_)
        ));
    }

    #[test]
    fn copy_mode_states_consequences_not_properties() {
        // The meaning is the operator's cost — copy, disk, seeding — never the
        // bare property "hardlink unsupported", which tells them nothing.
        let problem = copy_mode(&Medium::default(), Environment::LinuxNative);
        assert!(problem.meaning.contains("copy"));
        assert!(problem.meaning.contains("seed"));
        assert!(!problem
            .meaning
            .to_ascii_lowercase()
            .contains("hardlink unsupported"));
    }

    #[test]
    fn exfat_is_named_as_impossible_rather_than_merely_off() {
        let (summary, remedy) = limitation(&Medium::of("exfat"), Environment::MacOs);
        assert!(summary.contains("exFAT"));
        assert!(remedy.action.contains("cannot be worked around"));
    }

    #[test]
    fn an_smb_mount_on_macos_is_named_specifically() {
        let (summary, _) = limitation(&Medium::of("smbfs"), Environment::MacOs);
        assert!(summary.contains("SMB") && summary.contains("macOS"));
    }

    #[test]
    fn a_network_mount_elsewhere_is_named_as_a_network_mount() {
        let (summary, _) = limitation(&Medium::of("nfs"), Environment::LinuxNative);
        assert!(summary.contains("network"));
    }

    #[test]
    fn the_wsl2_boundary_is_named_from_the_filesystem_or_the_platform() {
        let (from_fs, _) = limitation(&Medium::of("9p"), Environment::LinuxNative);
        assert!(from_fs.contains("WSL2"));
        let (from_platform, _) = limitation(&Medium::default(), Environment::Windows);
        assert!(from_platform.contains("WSL2"));
    }

    #[test]
    fn an_unremarkable_volume_that_cannot_link_gets_a_plain_naming() {
        let (summary, _) = limitation(&Medium::default(), Environment::LinuxNative);
        assert!(summary.contains("cannot hardlink"));
    }

    // --- Classifying the medium: every platform, from one machine. ---

    #[test]
    fn outside_linux_the_medium_is_unknown_and_the_platform_decides() {
        for host in [HostOs::MacOs, HostOs::Windows, HostOs::Other] {
            assert_eq!(
                classify_medium(Path::new("/srv/media"), "", host),
                Medium::default()
            );
        }
    }

    #[test]
    fn the_medium_is_the_longest_mount_that_contains_the_path() {
        // A nested mount must win over the root it sits under, or a network
        // export mounted deep in a local tree would read as local.
        let table = "\
23 1 8:1 / / rw - ext4 /dev/sda1 rw
44 23 0:50 / /srv/media rw - nfs4 nas:/media rw
";
        let medium = classify_medium(Path::new("/srv/media/tv"), table, HostOs::Linux);
        assert_eq!(medium.fstype.as_deref(), Some("nfs4"));
        assert!(medium.network);
    }

    #[test]
    fn a_path_on_no_named_mount_is_an_unknown_medium() {
        let table = "44 23 0:50 / /srv/media rw - nfs4 nas:/media rw\n";
        assert_eq!(
            classify_medium(Path::new("/home/op/data"), table, HostOs::Linux),
            Medium::default()
        );
    }

    #[test]
    fn lines_that_are_not_mount_entries_are_ignored() {
        // Three malformed shapes precede the real entry, each of which must be
        // skipped rather than derail the scan: no separator at all, too few
        // fields to hold a mount point, and a separator with no type after it.
        let table = "garbage without a separator\n1 2 3 - ext4\n\
9 8 0:1 / /empty rw - \n\
23 1 8:1 / / rw - ext4 /dev/sda1 rw\n";
        assert_eq!(
            classify_medium(Path::new("/"), table, HostOs::Linux)
                .fstype
                .as_deref(),
            Some("ext4")
        );
    }

    #[test]
    fn a_filesystem_type_is_read_case_insensitively() {
        assert!(Medium::of("EXFAT").cannot_hardlink_ever());
    }

    // --- The impure probe: driven against a real disk, all lines reached. ---

    #[tokio::test]
    async fn a_real_writable_volume_reports_hardlinks_working() {
        let scratch = Scratch::new("works");
        let check = StorageCheck::new(Some(scratch.path().to_path_buf()), Environment::LinuxNative);
        let findings = check.run().await;
        assert!(matches!(
            findings.first().map(|finding| &finding.verdict),
            Some(Verdict::Pass { .. })
        ));
        assert_eq!(
            findings.first().map(|finding| finding.category),
            Some(Category::Storage)
        );
    }

    #[tokio::test]
    async fn no_data_root_is_skipped_rather_than_probed() {
        let findings = StorageCheck::new(None, Environment::LinuxNative)
            .run()
            .await;
        assert!(matches!(
            findings.first().map(|finding| &finding.verdict),
            Some(Verdict::Skipped { .. })
        ));
    }

    #[tokio::test]
    async fn a_nonexistent_data_root_is_reported_as_missing() {
        let missing = std::env::temp_dir().join(format!(
            "lemonfiber-storage-absent-{}-{}",
            std::process::id(),
            counter()
        ));
        let findings = StorageCheck::new(Some(missing), Environment::LinuxNative)
            .run()
            .await;
        assert!(matches!(
            findings.first().map(|finding| &finding.verdict),
            Some(Verdict::Fail(problem)) if problem.code == DATA_ROOT_MISSING
        ));
    }

    #[test]
    fn a_created_file_and_its_link_share_an_inode_and_raise_the_count() {
        let scratch = Scratch::new("inode");
        let paths = probe_paths(scratch.path());
        assert!(matches!(
            attempt_link(&paths),
            LinkOutcome::Linked { original, link }
                if original.inode == link.inode && link.links >= 2
        ));
        // The probe cleans up after itself, so nothing of it is left behind.
        assert!(!paths.0.exists() && !paths.1.exists());
    }

    #[test]
    fn a_write_into_a_missing_directory_is_a_write_failure() {
        let gone = std::env::temp_dir().join(format!(
            "lemonfiber-storage-gone-{}-{}",
            std::process::id(),
            counter()
        ));
        let paths = probe_paths(&gone);
        assert!(matches!(
            attempt_link(&paths),
            LinkOutcome::CouldNotWrite { .. }
        ));
    }

    #[test]
    fn a_link_onto_an_existing_name_is_a_link_failure() {
        // The write succeeds and the link does not — the shape of a volume that
        // holds files but cannot hardlink, reached here without an exotic disk.
        let scratch = Scratch::new("occupied");
        let original = scratch.path().join("original");
        let link = scratch.path().join("occupied.link");
        let _ = std::fs::write(&link, "already here");
        assert!(matches!(
            attempt_link(&(original, link)),
            LinkOutcome::CouldNotLink { .. }
        ));
    }

    #[test]
    fn a_link_whose_identity_cannot_be_read_is_uninspectable() {
        let scratch = Scratch::new("phantom");
        let absent = scratch.path().join("never-made");
        assert!(matches!(
            inspect(&absent, &absent),
            LinkOutcome::CouldNotInspect { .. }
        ));
    }

    #[cfg(unix)]
    #[test]
    fn identity_reads_the_inode_and_count_of_a_real_file() {
        let scratch = Scratch::new("identity");
        let file = scratch.path().join("real");
        let _ = std::fs::write(&file, "x");
        let read = super::identify(&file);
        assert!(read.is_ok_and(|found| found.links >= 1));
        assert!(super::identify(&scratch.path().join("absent")).is_err());
    }
}
