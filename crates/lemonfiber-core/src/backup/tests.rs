use std::path::{Path, PathBuf};

use super::{
    area, plan, relocation, Compatibility, Existing, Item, Manifest, Member, Pace, Plan, Retention,
    Scope, SCHEMA,
};
use crate::config::paths::Paths;
use crate::version::Version;

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
    // A plan that captured nothing excludes the library trivially, and would be
    // the one plan nobody should ship.
    assert!(!plan.items.is_empty(), "the capture takes nothing at all");
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

/// A capture taken before a takeover reads the setup's own host paths.
///
/// The whole point of the scope: nothing in the plan comes from `paths`, which
/// describes lemonfiber's layout and would describe the wrong machine's worth of
/// configuration entirely.
#[test]
fn a_capture_of_an_existing_setup_reads_the_host_paths_it_was_given() {
    let scope = Scope::existing(
        "media",
        &["/srv/their-media".to_owned(), "/mnt/tv".to_owned()],
    );
    let made = plan(&paths(), &scope);

    assert_eq!(
        made.items
            .iter()
            .map(|item| item.source.clone())
            .collect::<Vec<_>>(),
        vec![PathBuf::from("/srv/their-media"), PathBuf::from("/mnt/tv")]
    );
    assert!(
        made.sensitive,
        "somebody else's configuration holds keys too"
    );
    assert!(
        made.items
            .iter()
            .all(|item| item.archive_path.starts_with(area::EXISTING)),
        "{:?}",
        made.items
    );
}

/// Two trees ending in the same name still land on separate places in the archive.
///
/// Archive paths are positional rather than worked out from the host path, because
/// a capture that silently dropped one of two trees is the failure this scope
/// exists to prevent.
#[test]
fn two_trees_with_the_same_last_name_do_not_land_on_one_archive_path() {
    let scope = Scope::existing(
        "media",
        &["/srv/config".to_owned(), "/mnt/other/config".to_owned()],
    );
    let made = plan(&paths(), &scope);
    let mut seen: Vec<&str> = made
        .items
        .iter()
        .map(|item| item.archive_path.as_str())
        .collect();
    seen.sort_unstable();
    seen.dedup();
    assert_eq!(seen.len(), 2, "{:?}", made.items);
}

/// There is nowhere on this machine an existing setup's trees may be written back
/// to, so the area a restore would aim at is absent from the destinations.
#[test]
fn the_existing_area_is_not_somewhere_a_restore_can_write() {
    let targets = super::destinations(&paths());
    assert!(
        !targets.iter().any(|(area, _)| area == area::EXISTING),
        "{targets:?}"
    );
}

/// An archive of a setup lemonfiber does not manage is refused, with the trees it
/// holds named so the operator can put them back themselves.
#[test]
fn an_archive_of_a_setup_we_do_not_manage_is_refused_with_its_paths_named() {
    let scope = Scope::existing("media", &["/srv/their-media".to_owned()]);
    let made = plan(&paths(), &scope);
    let manifest = Manifest::describe(&made, "0.3.0", "t", "/srv/media");

    assert_eq!(
        Compatibility::assess(&manifest, "0.3.0", SCHEMA),
        Compatibility::NotOurs {
            project: "media".to_owned(),
            paths: vec!["/srv/their-media".to_owned()],
        }
    );
}

/// The refusal outranks the versions: an archive of somebody else's trees is not
/// made restorable by having been written by this very build.
#[test]
fn a_foreign_archive_is_refused_even_where_every_version_agrees() {
    let scope = Scope::existing("media", &["/srv/their-media".to_owned()]);
    let manifest = Manifest::describe(&plan(&paths(), &scope), "0.3.0", "t", "/srv/media");
    let whole = Manifest::describe(
        &plan(&paths(), &Scope::WholeStack),
        "0.3.0",
        "t",
        "/srv/media",
    );

    assert_eq!(
        Compatibility::assess(&whole, "0.3.0", SCHEMA),
        Compatibility::Compatible,
        "the same versions restore an archive of our own"
    );
    assert_eq!(
        Compatibility::assess(&manifest, "0.3.0", SCHEMA),
        Compatibility::NotOurs {
            project: "media".to_owned(),
            paths: vec!["/srv/their-media".to_owned()],
        },
        "but not one of somebody else's"
    );
}

/// A format this build cannot read is refused before its scope is trusted.
#[test]
fn a_foreign_archive_in_an_unreadable_format_is_refused_for_the_format() {
    let scope = Scope::existing("media", &["/srv/their-media".to_owned()]);
    let mut manifest = Manifest::describe(&plan(&paths(), &scope), "0.3.0", "t", "/srv/media");
    manifest.schema = SCHEMA + 1;

    assert_eq!(
        Compatibility::assess(&manifest, "0.3.0", SCHEMA),
        Compatibility::Incompatible {
            detail: format!(
                "the archive is format {} and this lemonfiber reads format {SCHEMA}",
                SCHEMA + 1
            ),
        },
        "a scope read out of a format we do not understand is not one to act on"
    );
}

/// The budget is derived from two numbers rather than written down as a third,
/// and this is what holds that true: a floor throughput and a minute, multiplied.
#[test]
fn the_budget_is_what_a_minute_at_the_floor_comes_to() {
    assert_eq!(
        super::BUDGET,
        super::FLOOR_BYTES_PER_SECOND * super::WITHIN.as_secs()
    );
}

/// Inclusive at the edge: a capture that exactly fills the budget is one the
/// budget says it has time for.
#[test]
fn a_capture_is_brisk_up_to_the_budget_and_not_past_it() {
    assert_eq!(
        (
            Pace::of(0).brisk,
            Pace::of(super::BUDGET).brisk,
            Pace::of(super::BUDGET + 1).brisk,
        ),
        (true, true, false)
    );
}

/// The reading carries the number it was judged against, so nothing showing it
/// needs a second copy of the budget to compare against.
#[test]
fn a_reading_carries_what_it_was_judged_against() {
    let pace = Pace::of(7);
    assert_eq!((pace.moved, pace.budget), (7, super::BUDGET));
}
