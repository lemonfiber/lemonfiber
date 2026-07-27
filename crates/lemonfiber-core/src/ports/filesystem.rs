//! Touching the data root to prove what it can do.
//!
//! The one property the whole media pipeline rests on is that downloads and the
//! library share a filesystem, so an import can hardlink rather than copy. A
//! filesystem's *name* only hints at whether that works — exFAT never links, a
//! network share usually cannot — but the exceptions are common enough that the
//! name is not evidence. A created-and-inspected link is.
//!
//! So the port is deliberately small operations rather than one "test hardlinks"
//! call: the sequence that proves capability — create, link, compare identities,
//! clean up — is decision-making, and it belongs above the port where a fake can
//! drive every branch of it without a real disk. What is left here is what only
//! a real filesystem can answer.
//!
//! See `.docs/architecture/ports-and-adapters.md`.

use std::path::{Path, PathBuf};

use async_trait::async_trait;

/// A filesystem's own identity for a path: which underlying file it names, and
/// how many names currently point at it.
///
/// Two directory entries are the same file when their `file` numbers agree; a
/// freshly linked file reports two names where one reports one. The number is an
/// inode on Unix and a file index on Windows — the port does not care which, only
/// that the same file compares equal to itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Identity {
    /// The filesystem's number for the file this entry points at.
    pub file: u64,
    /// How many names currently point at it.
    pub links: u64,
}

/// Whether a path is there, and on which volume.
///
/// The volume matters as much as the presence: when a drive is unplugged its
/// mount point often survives as an empty directory on the filesystem beneath
/// it, so a path that is still there but on a *different* volume than before is
/// a loss, not a presence. The number identifies the volume; comparing it to an
/// earlier reading is how a swapped-out mount is told from a steady one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Presence {
    /// The path is there, on the volume identified this way.
    On(u64),
    /// The path is not there at all.
    Gone,
}

/// Who owns a path and how its permission bits are set, as the platform reports
/// them.
///
/// Meaningful where ownership is real — native Linux — and reported so the check
/// above can decide whether the user the services run as could write here. The
/// mode is the low permission bits, the same nine a person reads as `rwxr-xr-x`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ownership {
    /// The owning user id.
    pub uid: u32,
    /// The owning group id.
    pub gid: u32,
    /// The permission bits.
    pub mode: u32,
}

/// A filesystem operation could not be completed, in the platform's own words.
///
/// One shape for every refusal because the caller decides what a failure *at a
/// given step* means — a create that fails is a permission problem, a link that
/// fails is the copy-mode signal — and that decision is about which call failed,
/// not about how the operating system worded it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fault {
    /// The operating system's own description of what went wrong.
    pub message: String,
}

impl Fault {
    /// A fault carrying the platform's description of it.
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

/// What the platform reports about the filesystem behind a path.
///
/// The type is a hint used only to *name* a limitation the probe has already
/// established — never to decide capability, which is measured rather than
/// inferred. The capacity figures are the platform's own, reported as they are
/// because a disk filling mid-import leaves partial files and a stalled queue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageFacts {
    /// The filesystem type, as far as it can be recognised.
    pub kind: FsKind,
    /// Whether it sits on removable media.
    pub removable: bool,
    /// Bytes free on the volume the path sits on.
    pub available: u64,
    /// The volume's total size in bytes, or zero where it could not be read.
    pub total: u64,
}

/// The filesystem types whose hardlink behaviour lemonfiber can speak to by name.
///
/// Recognised from the type name the platform reports. The name explains a
/// failure the probe found; it never stands in for the probe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FsKind {
    /// A local filesystem that links normally, carrying its name for display.
    Linking(String),
    /// exFAT — cannot hardlink at all, and cannot be made to.
    ExFat,
    /// A FAT variant — no hardlinks.
    Fat,
    /// An SMB or CIFS network share.
    Smb,
    /// An NFS network share.
    Nfs,
    /// A Windows filesystem seen across the WSL2 boundary.
    Wsl,
    /// A type not specifically recognised, carrying its name for display.
    Unknown(String),
}

impl FsKind {
    /// The recognised type behind a filesystem's reported name.
    ///
    /// Matched case-insensitively against the names each platform uses, so a
    /// macOS `smbfs` and a Linux `cifs` are both understood as one network share.
    #[must_use]
    pub fn classify(name: &str) -> Self {
        let lower = name.to_ascii_lowercase();
        match lower.as_str() {
            "exfat" => Self::ExFat,
            "msdos" | "vfat" | "fat" | "fat12" | "fat16" | "fat32" => Self::Fat,
            "smbfs" | "cifs" | "smb" | "smb2" | "smb3" => Self::Smb,
            "nfs" | "nfs4" | "nfsv4" => Self::Nfs,
            "9p" | "v9fs" | "drvfs" => Self::Wsl,
            "ext2" | "ext3" | "ext4" | "xfs" | "btrfs" | "zfs" | "apfs" | "hfs" | "hfsplus"
            | "ntfs" | "overlay" | "overlayfs" | "fuseblk" | "tmpfs" => {
                Self::Linking(name.to_owned())
            }
            _ => Self::Unknown(name.to_owned()),
        }
    }

    /// Whether this type is reached over the network.
    #[must_use]
    pub const fn is_network(&self) -> bool {
        matches!(self, Self::Smb | Self::Nfs)
    }

    /// How to name this type as the reason a link could not be made, where the
    /// type is one whose limitation lemonfiber can state specifically.
    ///
    /// `None` where the name explains nothing — a filesystem that normally links
    /// but did not is a fault to report plainly, not one to blame on its type.
    #[must_use]
    pub const fn limitation(&self) -> Option<&'static str> {
        match self {
            Self::ExFat => Some("exFAT cannot create hardlinks"),
            Self::Fat => Some("FAT filesystems cannot create hardlinks"),
            Self::Smb => Some("hardlinks are not usable across an SMB or CIFS share"),
            Self::Nfs => Some("hardlinks are not usable across an NFS share"),
            Self::Wsl => Some("hardlinks do not cross the WSL2 boundary to the Windows filesystem"),
            Self::Linking(_) | Self::Unknown(_) => None,
        }
    }

    /// The type as a person reads it, for a finding's evidence line.
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::Linking(name) | Self::Unknown(name) => name.clone(),
            Self::ExFat => "exFAT".to_owned(),
            Self::Fat => "FAT".to_owned(),
            Self::Smb => "SMB".to_owned(),
            Self::Nfs => "NFS".to_owned(),
            Self::Wsl => "WSL2".to_owned(),
        }
    }
}

/// One mounted volume as the platform reports it, before a path is attributed to
/// one of them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mount {
    /// Where the volume is mounted.
    pub point: PathBuf,
    /// The filesystem type name it reports.
    pub kind: String,
    /// Whether it is removable media.
    pub removable: bool,
    /// Bytes free on it.
    pub available: u64,
    /// Its total size in bytes.
    pub total: u64,
}

/// Choose the filesystem a path sits on from a set of mounts, by the longest
/// mount point that is a prefix of it.
///
/// The longest match is the right one: `/` is a prefix of everything, so a data
/// root under `/mnt/media` must be attributed to `/mnt/media` and not to the
/// root filesystem it also technically descends from. Where nothing matches —
/// no mount was reported — the type is unknown and the capacity zero, which the
/// caller treats as "nothing to name, nothing to measure" rather than a failure.
#[must_use]
pub fn pick(mounts: &[Mount], path: &Path) -> StorageFacts {
    let chosen = mounts
        .iter()
        .filter(|mount| path.starts_with(&mount.point))
        .max_by_key(|mount| mount.point.components().count());

    match chosen {
        Some(mount) => StorageFacts {
            kind: FsKind::classify(&mount.kind),
            removable: mount.removable,
            available: mount.available,
            total: mount.total,
        },
        None => StorageFacts {
            kind: FsKind::Unknown(String::new()),
            removable: false,
            available: 0,
            total: 0,
        },
    }
}

/// The filesystem, reached for the few things only a real one can answer.
///
/// Every method is one operation, so the sequence that proves a hardlink lives
/// above the port and stays testable. What a method could not do it reports as a
/// [`Fault`]; the caller decides what that means at the step it happened.
#[async_trait]
pub trait FileSystem: Send + Sync {
    /// Resolve a path to its real location, following symlinks, so a probe runs
    /// against the filesystem the data actually lives on rather than a link's.
    ///
    /// # Errors
    ///
    /// Returns a [`Fault`] when the path does not resolve — most often because
    /// nothing is there, which is itself the answer the caller wants.
    async fn canonicalize(&self, path: &Path) -> Result<PathBuf, Fault>;

    /// Create an empty file, failing where its parent is not a writable
    /// directory.
    ///
    /// # Errors
    ///
    /// Returns a [`Fault`] where the file could not be created.
    async fn touch(&self, path: &Path) -> Result<(), Fault>;

    /// Hardlink an existing file under a second name.
    ///
    /// # Errors
    ///
    /// Returns a [`Fault`] where the link could not be made — the observation
    /// that proves the filesystem cannot hardlink.
    async fn link(&self, from: &Path, to: &Path) -> Result<(), Fault>;

    /// The filesystem's identity for a path.
    ///
    /// # Errors
    ///
    /// Returns a [`Fault`] where the path's identity could not be read.
    async fn identify(&self, path: &Path) -> Result<Identity, Fault>;

    /// Remove a file, treating its absence as already done.
    ///
    /// Cleanup, so it cannot fail in a way the caller acts on: a probe file left
    /// behind is untidy, never wrong.
    async fn remove(&self, path: &Path);

    /// Read a small file lemonfiber wrote itself, or `None` where it is not there
    /// yet or cannot be read.
    ///
    /// Absence is the ordinary first-run case, not an error, so it and an
    /// unreadable file collapse to the same "nothing to compare against" answer.
    async fn read(&self, path: &Path) -> Option<String>;

    /// Record a small file lemonfiber keeps for itself, creating the directory
    /// for it where needed.
    ///
    /// Best-effort: a baseline that could not be written means the next run
    /// cannot detect a change from it, which is a lost opportunity rather than a
    /// fault to report — so this reports nothing.
    async fn write(&self, path: &Path, contents: &str);

    /// Who owns a path and how it may be accessed, or `None` where the platform
    /// does not report it — which is every platform but Unix.
    async fn ownership(&self, path: &Path) -> Option<Ownership>;

    /// What the platform reports about the filesystem behind a path.
    async fn describe(&self, path: &Path) -> StorageFacts;
}

/// Watching whether a path is still there, and still the same volume.
///
/// A trait of its own rather than another method on [`FileSystem`], because the
/// watch that needs it needs nothing else — and a check that needs the rest has
/// no use for this. Keeping them apart means a fake for either drives only the
/// question it answers.
#[async_trait]
pub trait Volume: Send + Sync {
    /// Whether a path is there, and on which volume, so a watch can tell a steady
    /// mount from one swapped out under it.
    async fn presence(&self, path: &Path) -> Presence;
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{pick, FsKind, Mount};

    /// A mount at a point, with a type and capacity a test does not otherwise
    /// care about filled in.
    fn mount(point: &str, kind: &str, removable: bool) -> Mount {
        Mount {
            point: PathBuf::from(point),
            kind: kind.to_owned(),
            removable,
            available: 500,
            total: 1_000,
        }
    }

    #[test]
    fn a_network_share_is_recognised_under_either_platforms_name() {
        assert_eq!(FsKind::classify("smbfs"), FsKind::Smb);
        assert_eq!(FsKind::classify("CIFS"), FsKind::Smb);
        assert_eq!(FsKind::classify("nfs4"), FsKind::Nfs);
        assert!(FsKind::classify("smbfs").is_network());
        assert!(FsKind::classify("nfs").is_network());
    }

    #[test]
    fn the_types_that_cannot_link_are_named_specifically() {
        // Each named limitation is stated so the reason reaches the operator: an
        // exFAT volume is told it is exFAT, a share is told it is a share.
        let named = [
            ("exfat", "exFAT"),
            ("vfat", "FAT"),
            ("smbfs", "SMB"),
            ("cifs", "SMB"),
            ("nfs", "NFS"),
            ("9p", "WSL2"),
            ("drvfs", "WSL2"),
        ];
        for (name, mention) in named {
            let stated = FsKind::classify(name).limitation();
            assert!(
                stated.is_some_and(|reason| reason.contains(mention)),
                "{name} should be named as {mention}"
            );
        }
    }

    #[test]
    fn a_filesystem_that_links_carries_no_blame_for_its_type() {
        let ext4 = FsKind::classify("ext4");
        assert_eq!(ext4, FsKind::Linking("ext4".to_owned()));
        assert!(ext4.limitation().is_none());
        assert!(!ext4.is_network());
        assert_eq!(ext4.label(), "ext4");
    }

    #[test]
    fn an_unrecognised_type_is_kept_by_name_rather_than_guessed_at() {
        let odd = FsKind::classify("somenewfs");
        assert_eq!(odd, FsKind::Unknown("somenewfs".to_owned()));
        assert!(odd.limitation().is_none());
        assert_eq!(odd.label(), "somenewfs");
    }

    #[test]
    fn each_named_limitation_reads_as_a_type_a_person_recognises() {
        for name in ["exfat", "fat32", "smbfs", "nfs", "9p"] {
            let kind = FsKind::classify(name);
            assert!(!kind.label().is_empty(), "{name} should have a label");
        }
    }

    #[test]
    fn a_path_is_attributed_to_the_longest_mount_that_contains_it() {
        let mounts = vec![
            mount("/", "apfs", false),
            mount("/Volumes/media", "exfat", true),
        ];
        let facts = pick(&mounts, Path::new("/Volumes/media/tv"));
        assert_eq!(facts.kind, FsKind::ExFat);
        assert!(facts.removable);
        assert_eq!(
            facts.total, 1_000,
            "the chosen mount's capacity comes through"
        );
    }

    #[test]
    fn a_path_under_no_reported_mount_has_an_unknown_filesystem() {
        let facts = pick(&[], Path::new("/anywhere"));
        assert_eq!(facts.kind, FsKind::Unknown(String::new()));
        assert!(!facts.removable);
        assert_eq!(facts.total, 0, "nothing to measure where nothing matched");
    }
}
