//! What a configuration backup holds, and everything decidable about one without
//! touching a disk.
//!
//! A backup here is *configuration*, never media: the library may be terabytes
//! and irreplaceable, but it is not what breaks — what breaks is the small,
//! intricate state in the service databases and the `.env` beside them. Making
//! that recoverable is what lets an operator experiment, update, and touch a
//! working system instead of leaving it running years-old versions out of fear.
//!
//! This module is the archive's vocabulary and its policy, kept pure so the whole
//! of it runs in a test with no services and no filesystem: what a capture
//! includes ([`plan`]), whether a captured set carries credentials, whether an
//! archive read back can be restored here at all ([`Compatibility::assess`]),
//! whether it was taken against a different data root ([`relocation`]), and which
//! of several archives to prune ([`Retention::prune`]). The writing and reading
//! of the archive itself live in the app layer; nothing decided here can be wrong
//! in a way a test cannot reach.

/// How much of the stack a backup covers.
///
/// Whole-stack is the common case, but restoring one service is often what is
/// actually wanted — one \*arr's configuration mangled while the rest is fine —
/// so the scope is recorded in the archive and honoured on the way back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "scope", rename_all = "snake_case")]
pub enum Scope {
    /// Every service's configuration, plus lemonfiber's own and the stack.
    WholeStack,
    /// One named service's configuration alone.
    Service {
        /// The service whose configuration this covers.
        name: String,
    },
    /// An existing setup's own configuration, at the host paths it keeps it in.
    ///
    /// The one scope whose sources are not lemonfiber's layout. A capture taken
    /// before a takeover has to cover the tree that is already there — lemonfiber's
    /// own holds nothing worth protecting until the takeover has happened — so the
    /// host path each tree was read from is recorded here, in the manifest, rather
    /// than inferred from a layout that does not describe it.
    ///
    /// Recording those paths is also what makes putting one back an ordinary
    /// extraction the operator performs deliberately, rather than something
    /// lemonfiber does on their behalf into a tree it does not manage.
    Existing {
        /// The Compose project the capture was taken from.
        project: String,
        /// The host trees captured, in the order the survey reported them.
        trees: Vec<Tree>,
    },
}

/// One host tree captured from a setup lemonfiber does not manage.
///
/// Both halves are needed to find it again: the archive path says where it sits
/// inside the archive, and the host path says where it was read from. Nothing
/// derives the second from the first, because a tree outside lemonfiber's layout
/// has no layout to derive it from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Tree {
    /// Where it was read from, on the machine whose setup was taken over.
    pub host_path: String,
    /// Where it sits inside the archive.
    pub archive_path: String,
}

impl Scope {
    /// The scope of a capture covering an existing setup's own trees.
    ///
    /// The archive paths are assigned here, in one place, so a capture and anything
    /// reading the archive back cannot disagree about them. They are positional
    /// rather than worked out from the host path: two setups mounting `/srv/media`
    /// and `/mnt/srv/media` would otherwise land on one name inside a single
    /// archive, and a capture that silently dropped a tree is precisely the failure
    /// this scope exists to prevent.
    #[must_use]
    pub fn existing(project: &str, host_paths: &[String]) -> Self {
        Self::Existing {
            project: project.to_owned(),
            trees: host_paths
                .iter()
                .enumerate()
                .map(|(index, host_path)| Tree {
                    host_path: host_path.clone(),
                    archive_path: format!("{}/{index}", area::EXISTING),
                })
                .collect(),
        }
    }
}

/// One thing a capture copies into the archive.
///
/// The source is where it is read from on this machine; the archive path is where
/// it lands inside the archive, stable across machines so a restore knows where to
/// put it back. The label is what a contents listing shows the operator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// Where it is read from.
    pub source: PathBuf,
    /// Where it sits inside the archive, and is restored back to a data root.
    pub archive_path: String,
    /// What a listing calls it, in the operator's terms.
    pub label: String,
}

/// The record written inside an archive, and read back to decide a restore.
///
/// Everything a restore needs to know before it overwrites anything: what made
/// the archive, when, what data root it was taken against, what it covers, whether
/// it is sensitive, and the contents to list. Round-trips through JSON so the same
/// value the capture wrote is the value the restore reads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(rename = "BackupManifest")]
pub struct Manifest {
    /// The archive format, checked before anything inside is trusted.
    pub schema: u32,
    /// The lemonfiber version that wrote it, checked against the one restoring.
    pub product_version: String,
    /// When it was taken. Opaque here; the surface stamps it from the clock.
    pub created_at: String,
    /// The data root it was taken against, to notice a restore to a different one.
    pub data_root: String,
    /// What it covers.
    pub scope: Scope,
    /// Whether it carries credentials, and so must be handled as sensitive.
    pub sensitive: bool,
    /// What is inside, for a listing shown before anything is overwritten.
    pub members: Vec<Member>,
}

/// One entry in an archive's contents listing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Member {
    /// Where it sits inside the archive.
    pub archive_path: String,
    /// What it is, in the operator's terms.
    pub label: String,
}

/// One backup already on disk, as retention sees it: a name and when it was taken.
///
/// Ordered by when it was taken, oldest first, so the ones to prune are simply the
/// ones the keep-count does not reach.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Existing {
    /// What the archive is called on disk.
    pub name: String,
    /// When it was taken, compared lexically — the surface names archives so this
    /// holds.
    pub created_at: String,
}

use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};

use crate::config::paths::Paths;

/// The archive format this build writes and knows how to read back.
///
/// Bumped only when the layout inside the archive changes incompatibly. An
/// archive stamped with any other schema is refused rather than half-read, since
/// a backup that restores into a subtly wrong state is worse than none.
pub const SCHEMA: u32 = 1;

/// The time a capture of a typical configuration is meant to finish inside.
///
/// A `SHOULD` in the specification, and it is treated as one here: nothing refuses a
/// capture for being large, and nothing reports a failure for one that takes longer.
/// What the number is good for is being said beforehand. A capture that will take ten
/// minutes is not a fault — it is a library nobody warned the operator about — and the
/// difference between "this is taking a while" and "this has hung" is the whole of
/// what somebody watching it needs.
pub(crate) const WITHIN: std::time::Duration = std::time::Duration::from_secs(60);

/// The slowest disk this is willing to reason about, in bytes a second.
///
/// Not a measurement of any machine in particular, and deliberately not one: it is a
/// floor picked to sit under anything this product plausibly runs on, including a USB
/// 2.0 enclosure and the spinning disk in a repurposed desktop. Reasoning from a floor
/// is what makes the answer safe in the direction that matters — a capture this calls
/// comfortable finishes comfortably everywhere, and one it calls large may still be
/// quick on an `NVMe`, which is the error worth making.
const FLOOR_BYTES_PER_SECOND: u64 = 10 * 1024 * 1024;

/// The bytes a capture can move and still be expected to finish inside [`WITHIN`].
///
/// Derived rather than written down, so the two numbers behind it are the only ones
/// anybody has to keep true. A third, stated separately, is a third chance to disagree
/// with the other two.
pub const BUDGET: u64 = FLOOR_BYTES_PER_SECOND * WITHIN.as_secs();

/// What a capture came to, against the time a capture is meant to take.
///
/// Reported and never enforced. The room check already walks the trees to decide
/// whether the archive fits, so the bytes are in hand before anything is written and
/// cost nothing extra to say — and what they are measured against is the work, not a
/// clock. A wall-clock gate on a machine whose disk throughput varies by more than the
/// margin either passes for reasons unrelated to this product or fails for them, and
/// neither reading is worth having.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Pace {
    /// The bytes the captured trees came to, as the room check measured them.
    pub moved: u64,
    /// The bytes a capture may move and still be expected to finish in time.
    ///
    /// Carried with the reading rather than left for a reader to look up, so a surface
    /// showing this does not need a second copy of the number to compare against.
    pub budget: u64,
    /// Whether this capture is inside it.
    pub brisk: bool,
}

impl Pace {
    /// How a capture of `moved` bytes stands against the budget.
    #[must_use]
    pub const fn of(moved: u64) -> Self {
        Self {
            moved,
            budget: BUDGET,
            brisk: moved <= BUDGET,
        }
    }
}

/// What a capture will copy, decided from the layout and the scope alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// The scope this plan covers.
    pub scope: Scope,
    /// Each thing to copy, in a stable order.
    pub items: Vec<Item>,
    /// Whether the captured set carries credentials, and so is sensitive.
    pub sensitive: bool,
}

/// Where inside the archive each captured area lands.
///
/// Constants rather than inline strings so the writer and the reader cannot drift
/// onto different names for the same area.
mod area {
    /// lemonfiber's own configuration — `.env`, the change journal, setup progress.
    pub const CONFIG: &str = "config";
    /// The per-service configuration directories the containers mount.
    pub const SERVICES: &str = "services";
    /// The materialised compose files.
    pub const STACK: &str = "stack";
    /// The trees of a setup lemonfiber does not manage, captured before a takeover.
    ///
    /// Absent from [`super::destinations`] on purpose: there is no place on this
    /// machine lemonfiber may write these back to, so the area a restore would need
    /// to aim at simply does not exist.
    pub(crate) const EXISTING: &str = "existing";
}

/// Decide what a capture of `scope` copies, from the install layout.
///
/// Whole-stack takes lemonfiber's configuration (which defines the expected
/// state), every service's configuration and database, and the materialised
/// stack. A single service takes only its own configuration directory. Neither
/// takes the media library, downloads, images or logs — those live outside this
/// layout entirely, so excluding them is structural rather than a filter that
/// could be forgotten.
///
/// The set is marked sensitive when it carries credentials: lemonfiber's `.env`
/// holds the VPN key and provider passwords, and each service's configuration
/// holds its API key, so any capture of either is exactly as sensitive as the
/// secrets inside it.
#[must_use]
pub fn plan(paths: &Paths, scope: &Scope) -> Plan {
    let items = match scope {
        Scope::WholeStack => vec![
            Item {
                source: paths.config_dir().to_path_buf(),
                archive_path: area::CONFIG.to_owned(),
                label: "lemonfiber configuration".to_owned(),
            },
            Item {
                source: paths.service_config(),
                archive_path: area::SERVICES.to_owned(),
                label: "service configuration".to_owned(),
            },
            Item {
                source: paths.stack(),
                archive_path: area::STACK.to_owned(),
                label: "materialised stack".to_owned(),
            },
        ],
        Scope::Service { name } => vec![Item {
            source: paths.service_config().join(name),
            archive_path: format!("{}/{name}", area::SERVICES),
            label: format!("{name} configuration"),
        }],
        // The only scope that reads nothing from `paths`: an existing setup keeps
        // its configuration where it keeps it, and the survey is what found out
        // where that is.
        Scope::Existing { trees, .. } => trees
            .iter()
            .map(|tree| Item {
                source: PathBuf::from(&tree.host_path),
                archive_path: tree.archive_path.clone(),
                label: tree.host_path.clone(),
            })
            .collect(),
    };

    // Every scope captures credential-bearing configuration — the `.env`'s VPN key
    // and provider passwords, a service's own API key — so a capture is always as
    // sensitive as those secrets, and is marked so rather than inferred loosely.
    Plan {
        scope: scope.clone(),
        items,
        sensitive: true,
    }
}

/// Where each archived area is written back to on this machine's layout.
///
/// The mirror of what [`plan`] reads from — the same directory for each area — so
/// a capture and a restore cannot drift onto different places for the same thing.
/// The restore hands these to the archive reader, which unpacks each member under
/// the directory its area names.
#[must_use]
pub fn destinations(paths: &Paths) -> Vec<(String, PathBuf)> {
    vec![
        (area::CONFIG.to_owned(), paths.config_dir().to_path_buf()),
        (area::SERVICES.to_owned(), paths.service_config()),
        (area::STACK.to_owned(), paths.stack()),
    ]
}

impl Manifest {
    /// Build the manifest that describes a capture, given the version and time the
    /// surface stamped and the [`Plan`] that decided the contents.
    ///
    /// The version and time are handed in rather than read here, so the core stays
    /// free of the clock and the build's own identity — the same reason a change
    /// journal carries a timestamp it did not read.
    #[must_use]
    pub fn describe(
        plan: &Plan,
        product_version: impl Into<String>,
        created_at: impl Into<String>,
        data_root: impl Into<String>,
    ) -> Self {
        let members = plan
            .items
            .iter()
            .map(|item| Member {
                archive_path: item.archive_path.clone(),
                label: item.label.clone(),
            })
            .collect();
        Self {
            schema: SCHEMA,
            product_version: product_version.into(),
            created_at: created_at.into(),
            data_root: data_root.into(),
            scope: plan.scope.clone(),
            sensitive: plan.sensitive,
            members,
        }
    }

    /// The paths inside the archive that would place a file outside the tree it is
    /// unpacked into — an empty list means every member lands where it should.
    ///
    /// A manifest is read back from an archive that may be corrupt or hostile, so
    /// its own words are not trusted: a member path or a single-service name
    /// carrying `..`, a root, or a nested segment would, once the restore executor
    /// carries out the plan, write over something it was never meant to touch. The
    /// refusal is decided here, before any file is opened, because whether a path
    /// escapes is a total property of the manifest that needs no disk to settle.
    #[must_use]
    pub fn escapes(&self) -> Vec<String> {
        let mut escaping: Vec<String> = self
            .members
            .iter()
            .filter(|member| !contained(&member.archive_path))
            .map(|member| member.archive_path.clone())
            .collect();
        if let Scope::Service { name } = &self.scope {
            if !single_segment(name) {
                escaping.push(name.clone());
            }
        }
        escaping
    }
}

/// Whether an archive-relative path stays within the tree it is unpacked into: a
/// non-empty path built only of ordinary names, with no `..`, no root and no drive
/// prefix. A bare `.` is harmless and allowed.
fn contained(path: &str) -> bool {
    let path = Path::new(path);
    !path.as_os_str().is_empty()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_) | Component::CurDir))
}

/// Whether a service name is a single ordinary path segment — no separators, no
/// traversal — so joining it onto the service-config directory cannot leave it.
fn single_segment(name: &str) -> bool {
    let mut components = Path::new(name).components();
    matches!(
        (components.next(), components.next()),
        (Some(Component::Normal(_)), None)
    )
}

mod compatibility;
mod retention;
pub mod run;

pub use compatibility::{relocation, Compatibility, Relocation};
pub use retention::Retention;

#[cfg(test)]
mod tests;
