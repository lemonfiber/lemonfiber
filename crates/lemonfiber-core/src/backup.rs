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

pub use compatibility::{relocation, Compatibility, Relocation};
pub use retention::Retention;

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::compatibility::Version;
    use super::{
        area, plan, relocation, Compatibility, Existing, Item, Manifest, Member, Plan, Retention,
        Scope, SCHEMA,
    };
    use crate::config::paths::Paths;

    fn paths() -> Paths {
        Paths::rooted(
            Path::new("/home/op/.config"),
            Path::new("/home/op/.local/share"),
        )
    }

    fn whole_stack_plan() -> Plan {
        plan(&paths(), &Scope::WholeStack)
    }

    #[test]
    fn a_whole_stack_capture_takes_config_services_and_the_stack() {
        let plan = whole_stack_plan();
        let archived: Vec<&str> = plan
            .items
            .iter()
            .map(|item| item.archive_path.as_str())
            .collect();
        assert_eq!(archived, vec![area::CONFIG, area::SERVICES, area::STACK]);
        assert!(plan.sensitive, "a whole-stack capture holds credentials");
    }

    #[test]
    fn a_whole_stack_capture_takes_the_expected_state_baseline() {
        // An adopted edit is kept in the baseline, and the baseline must survive a
        // restore for the adoption to. It sits inside the configuration directory the
        // capture takes whole, so it is carried along by construction — this guards
        // that the baseline stays under the captured area rather than drifting out.
        let paths = paths();
        let plan = plan(&paths, &Scope::WholeStack);
        let config = plan
            .items
            .iter()
            .find(|item| item.archive_path == area::CONFIG);
        assert!(
            config.is_some_and(|item| paths.baseline().starts_with(&item.source)),
            "the baseline is inside the captured configuration directory",
        );
    }

    #[test]
    fn a_whole_stack_capture_reads_from_the_install_layout() {
        let paths = paths();
        let plan = plan(&paths, &Scope::WholeStack);
        let sources: Vec<PathBuf> = plan.items.iter().map(|item| item.source.clone()).collect();
        assert_eq!(
            sources,
            vec![
                paths.config_dir().to_path_buf(),
                paths.service_config(),
                paths.stack(),
            ]
        );
    }

    #[test]
    fn a_whole_stack_capture_excludes_the_media_library_by_construction() {
        // The operator's media lives at their data root, outside this layout, so no
        // item a whole-stack capture takes sits under the data directory's non
        // -lemonfiber areas — the exclusion is structural, not a filter.
        let plan = whole_stack_plan();
        for item in &plan.items {
            assert!(
                item.source.starts_with("/home/op/.config/lemonfiber")
                    || item.source.starts_with("/home/op/.local/share/lemonfiber"),
                "{:?} is outside the install layout",
                item.source
            );
        }
    }

    #[test]
    fn a_single_service_capture_takes_only_that_service() {
        let paths = paths();
        let plan = plan(
            &paths,
            &Scope::Service {
                name: "sonarr".to_owned(),
            },
        );
        assert_eq!(
            plan.items,
            vec![Item {
                source: paths.service_config().join("sonarr"),
                archive_path: format!("{}/sonarr", area::SERVICES),
                label: "sonarr configuration".to_owned(),
            }]
        );
        assert!(plan.sensitive, "a service's config holds its API key");
    }

    #[test]
    fn restore_writes_each_area_back_to_where_the_capture_read_it() {
        let paths = paths();
        let places: Vec<(String, PathBuf)> = super::destinations(&paths);
        assert_eq!(
            places,
            vec![
                (area::CONFIG.to_owned(), paths.config_dir().to_path_buf()),
                (area::SERVICES.to_owned(), paths.service_config()),
                (area::STACK.to_owned(), paths.stack()),
            ]
        );
    }

    #[test]
    fn a_manifest_describes_the_plan_it_was_built_from() {
        let plan = whole_stack_plan();
        let manifest = Manifest::describe(&plan, "0.3.0", "2026-07-30T00:00:00Z", "/srv/media");
        assert_eq!(manifest.schema, SCHEMA);
        assert_eq!(manifest.product_version, "0.3.0");
        assert_eq!(manifest.created_at, "2026-07-30T00:00:00Z");
        assert_eq!(manifest.data_root, "/srv/media");
        assert_eq!(manifest.scope, Scope::WholeStack);
        assert!(manifest.sensitive);
        let labelled: Vec<&str> = manifest
            .members
            .iter()
            .map(|member| member.label.as_str())
            .collect();
        assert_eq!(
            labelled,
            vec![
                "lemonfiber configuration",
                "service configuration",
                "materialised stack"
            ]
        );
    }

    #[test]
    fn a_manifest_round_trips_through_json() {
        let plan = plan(
            &paths(),
            &Scope::Service {
                name: "radarr".to_owned(),
            },
        );
        let manifest = Manifest::describe(&plan, "0.3.0", "t", "/srv/media");
        let line = serde_json::to_string(&manifest).unwrap_or_default();
        let read = serde_json::from_str::<Manifest>(&line).ok();
        assert_eq!(read.as_ref(), Some(&manifest), "{line}");
    }

    #[test]
    fn a_version_parses_three_numbers_and_ignores_a_pre_release_suffix() {
        assert_eq!(
            Version::parse("1.2.3"),
            Some(Version {
                major: 1,
                minor: 2,
                patch: 3
            })
        );
        assert_eq!(
            Version::parse("0.3.0-rc.1"),
            Some(Version {
                major: 0,
                minor: 3,
                patch: 0
            })
        );
    }

    #[test]
    fn a_version_that_is_not_three_numbers_does_not_parse() {
        for bad in ["1.2", "1.2.3.4", "1.x.0", "", "nightly"] {
            assert_eq!(Version::parse(bad), None, "{bad:?} should not parse");
        }
    }

    fn manifest_of(version: &str, schema: u32) -> Manifest {
        let mut manifest = Manifest::describe(&whole_stack_plan(), version, "t", "/srv/media");
        manifest.schema = schema;
        manifest
    }

    #[test]
    fn an_archive_of_the_same_version_and_schema_is_compatible() {
        let manifest = manifest_of("0.3.0", SCHEMA);
        assert_eq!(
            Compatibility::assess(&manifest, "0.3.0", SCHEMA),
            Compatibility::Compatible
        );
    }

    #[test]
    fn an_older_archive_within_the_same_major_is_compatible() {
        let manifest = manifest_of("0.2.5", SCHEMA);
        assert_eq!(
            Compatibility::assess(&manifest, "0.3.0", SCHEMA),
            Compatibility::Compatible
        );
    }

    #[test]
    fn a_newer_archive_is_refused_with_the_gap_named() {
        let manifest = manifest_of("0.4.0", SCHEMA);
        assert_eq!(
            Compatibility::assess(&manifest, "0.3.0", SCHEMA),
            Compatibility::TooNew {
                archive: "0.4.0".to_owned(),
                current: "0.3.0".to_owned(),
            }
        );
    }

    #[test]
    fn an_archive_a_whole_major_behind_is_a_warned_downgrade() {
        let manifest = manifest_of("1.4.0", SCHEMA);
        assert_eq!(
            Compatibility::assess(&manifest, "2.0.0", SCHEMA),
            Compatibility::Downgrade {
                archive: "1.4.0".to_owned(),
                current: "2.0.0".to_owned(),
            }
        );
    }

    #[test]
    fn an_archive_in_an_unknown_format_is_incompatible() {
        let manifest = manifest_of("0.3.0", SCHEMA + 1);
        assert!(matches!(
            Compatibility::assess(&manifest, "0.3.0", SCHEMA),
            Compatibility::Incompatible { .. }
        ));
    }

    #[test]
    fn an_archive_whose_version_cannot_be_read_is_incompatible() {
        let manifest = manifest_of("nightly", SCHEMA);
        assert!(matches!(
            Compatibility::assess(&manifest, "0.3.0", SCHEMA),
            Compatibility::Incompatible { .. }
        ));
    }

    #[test]
    fn a_restore_with_a_version_the_build_cannot_state_is_incompatible() {
        // The running build's own version is malformed — defensive, but its branch
        // is real and must resolve to a refusal rather than a panic.
        let manifest = manifest_of("0.3.0", SCHEMA);
        assert!(matches!(
            Compatibility::assess(&manifest, "unknown", SCHEMA),
            Compatibility::Incompatible { .. }
        ));
    }

    #[test]
    fn a_restore_to_the_same_data_root_needs_no_relocation() {
        let manifest = manifest_of("0.3.0", SCHEMA);
        assert_eq!(relocation(&manifest, Path::new("/srv/media")), None);
    }

    #[test]
    fn a_restore_to_a_different_data_root_is_offered_a_re_point() {
        let manifest = manifest_of("0.3.0", SCHEMA);
        let moved = relocation(&manifest, Path::new("/mnt/library"));
        assert_eq!(
            moved.map(|move_| (move_.was, move_.now)),
            Some(("/srv/media".to_owned(), "/mnt/library".to_owned()))
        );
    }

    #[test]
    fn a_trailing_slash_on_the_data_root_is_not_a_relocation() {
        // The same place spelled with a trailing separator is the same root, and
        // must not raise a re-point the operator then has to wave away.
        let manifest = manifest_of("0.3.0", SCHEMA);
        assert_eq!(relocation(&manifest, Path::new("/srv/media/")), None);
    }

    #[test]
    fn a_clean_whole_stack_manifest_escapes_nothing() {
        assert!(manifest_of("0.3.0", SCHEMA).escapes().is_empty());
    }

    #[test]
    fn a_clean_single_service_manifest_escapes_nothing() {
        let plan = plan(
            &paths(),
            &Scope::Service {
                name: "sonarr".to_owned(),
            },
        );
        let manifest = Manifest::describe(&plan, "0.3.0", "t", "/srv/media");
        assert!(manifest.escapes().is_empty());
    }

    #[test]
    fn a_member_that_traverses_out_of_the_archive_is_caught() {
        // A hostile or corrupt archive naming `../../etc/passwd` must be refused
        // before the restore executor ever opens a file.
        let mut manifest = manifest_of("0.3.0", SCHEMA);
        manifest.members.push(Member {
            archive_path: "../../etc/passwd".to_owned(),
            label: "hostile".to_owned(),
        });
        assert!(manifest.escapes().contains(&"../../etc/passwd".to_owned()));
    }

    #[test]
    fn a_single_service_name_that_is_more_than_one_segment_is_caught() {
        // The archive path `services/a/b` stays inside the archive, but the name
        // `a/b` would leave the single service's own directory when joined on
        // restore, so the scope itself is refused.
        let plan = plan(
            &paths(),
            &Scope::Service {
                name: "a/b".to_owned(),
            },
        );
        let manifest = Manifest::describe(&plan, "0.3.0", "t", "/srv/media");
        assert_eq!(manifest.escapes(), vec!["a/b".to_owned()]);
    }

    fn taken(name: &str, at: &str) -> Existing {
        Existing {
            name: name.to_owned(),
            created_at: at.to_owned(),
        }
    }

    #[test]
    fn retention_prunes_the_oldest_beyond_the_keep_count() {
        let existing = vec![
            taken("c", "2026-07-03"),
            taken("a", "2026-07-01"),
            taken("b", "2026-07-02"),
        ];
        let pruned = Retention::keeping(2).prune(existing);
        // Sorted oldest-first, keeping two leaves only the eldest to prune, named
        // whatever order it was listed in.
        assert_eq!(
            pruned.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(),
            vec!["a"]
        );
    }

    #[test]
    fn retention_keeps_everything_when_there_is_no_surplus() {
        let existing = vec![taken("a", "2026-07-01"), taken("b", "2026-07-02")];
        assert!(Retention::keeping(5).prune(existing).is_empty());
    }

    #[test]
    fn retention_never_prunes_the_last_remaining_backup() {
        // A keep of zero would delete the only recovery there is; it is raised to
        // one, so a lone backup is always kept.
        let only = vec![taken("a", "2026-07-01")];
        assert!(Retention::keeping(0).prune(only).is_empty());
    }

    #[test]
    fn a_scope_round_trips_through_json() {
        for scope in [
            Scope::WholeStack,
            Scope::Service {
                name: "prowlarr".to_owned(),
            },
        ] {
            let line = serde_json::to_string(&scope).unwrap_or_default();
            let read = serde_json::from_str::<Scope>(&line).ok();
            assert_eq!(read.as_ref(), Some(&scope), "{line}");
        }
    }
}
