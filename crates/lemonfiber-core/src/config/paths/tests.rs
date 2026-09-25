use std::path::{Path, PathBuf};

use super::Paths;

fn paths() -> Paths {
    Paths::rooted(
        Path::new("/home/op/.config"),
        Path::new("/home/op/.local/share"),
    )
}

#[test]
fn everything_lives_under_a_directory_named_for_the_product() {
    let paths = paths();
    assert_eq!(paths.config_dir(), Path::new("/home/op/.config/lemonfiber"));
    assert_eq!(
        paths.data_dir(),
        Path::new("/home/op/.local/share/lemonfiber")
    );
}

#[test]
fn configuration_and_regenerable_data_are_kept_apart() {
    let paths = paths();
    let config: Vec<PathBuf> = vec![
        paths.env_file(),
        paths.journal(),
        paths.journal_key(),
        paths.setup_progress(),
        paths.baseline(),
        paths.acknowledged(),
        paths.materialised(),
        paths.quality(),
        paths.notifications(),
        paths.autostart(),
        paths.boot(),
        paths.bandwidth(),
        paths.accepted(),
        paths.refusals(),
        paths.admission(),
        paths.updates(),
        paths.plugins(),
    ];
    let data: Vec<PathBuf> = vec![
        paths.stack(),
        paths.service_config(),
        paths.backups(),
        paths.bundles(),
        paths.storage_state(),
        paths.hosted(),
    ];

    // The failure message uses an inline capture rather than a call such as
    // `path.display()`: an argument expression only evaluates when the
    // assertion fails, so it is a line no passing test can cover.
    for path in &config {
        assert!(
            path.starts_with(paths.config_dir()),
            "{path:?} is not configuration"
        );
    }
    for path in &data {
        assert!(
            path.starts_with(paths.data_dir()),
            "{path:?} is not regenerable"
        );
    }
}

#[test]
fn the_baseline_sits_beside_the_env_file() {
    // Seeding derives where it writes the baseline from the environment file it is
    // handed — `env_file.with_file_name("baseline.json")` — while a backup captures
    // the configuration directory whole, and its restore-survival test checks
    // against `baseline()`. The two derivations must land on the same file, or an
    // adopted baseline would be written somewhere a restore does not carry. This
    // ties them: the layout's own `baseline()` is exactly what the seed's formula
    // produces from `env_file()`.
    let paths = paths();
    assert_eq!(
        paths.env_file().with_file_name("baseline.json"),
        paths.baseline()
    );
}

#[test]
fn the_materialised_record_sits_beside_the_env_file() {
    // A lifecycle command derives where it keeps the record from the environment
    // file it is handed — `env_file.with_file_name("materialised.json")` — the same
    // way seeding derives the baseline's. This ties that formula to the layout's
    // own `materialised()`, so the two cannot drift onto different files.
    let paths = paths();
    assert_eq!(
        paths.env_file().with_file_name("materialised.json"),
        paths.materialised()
    );
}

#[test]
fn the_answered_choices_sit_beside_the_env_file() {
    // The doctor derives where it keeps them from the environment file it is
    // handed — `env_file.with_file_name("accepted.json")` — while a backup
    // captures the configuration directory whole. The two derivations must land
    // on the same file, or a restore would put every settled question again.
    let paths = paths();
    assert_eq!(
        paths.env_file().with_file_name("accepted.json"),
        paths.accepted()
    );
}

#[test]
fn the_quality_choice_sits_beside_the_env_file() {
    // The quality command derives where it keeps the choice from the environment
    // file it is handed — `env_file.with_file_name("quality.json")` — the same way
    // seeding derives the baseline's. This ties that formula to the layout's own
    // `quality()`, so a backup that captures the configuration directory carries it.
    let paths = paths();
    assert_eq!(
        paths.env_file().with_file_name("quality.json"),
        paths.quality()
    );
}

#[test]
fn what_the_household_declared_about_its_line_sits_beside_the_env_file() {
    // The bandwidth command derives where it keeps the declaration the same way
    // the quality command does, and this ties that formula to the layout's own
    // `bandwidth()` — so a backup that captures the configuration directory
    // carries it, and a restore does not throw away weeks of watching the line.
    let paths = paths();
    assert_eq!(
        paths.env_file().with_file_name("bandwidth.json"),
        paths.bandwidth()
    );
}

#[test]
fn what_the_update_check_remembers_sits_beside_the_env_file() {
    // The check derives where it keeps its memory from the environment file it is
    // handed — `env_file.with_file_name("updates.json")` — the same way the
    // answered choices are derived. This ties that formula to the layout's own
    // `updates()`, so a restore carries a machine's record of having given up.
    let paths = paths();
    assert_eq!(
        paths.env_file().with_file_name("updates.json"),
        paths.updates()
    );
}

#[test]
fn every_location_is_distinct() {
    let paths = paths();
    let all = [
        paths.env_file(),
        paths.journal(),
        paths.journal_key(),
        paths.setup_progress(),
        paths.baseline(),
        paths.materialised(),
        paths.quality(),
        paths.autostart(),
        paths.boot(),
        paths.bandwidth(),
        paths.admission(),
        paths.updates(),
        paths.stack(),
        paths.service_config(),
        paths.backups(),
        paths.storage_state(),
        paths.hosted(),
    ];
    for (index, path) in all.iter().enumerate() {
        for other in all.iter().skip(index + 1) {
            assert_ne!(path, other, "two things share a location");
        }
    }
}

#[test]
fn the_layout_does_not_depend_on_where_the_bases_are() {
    let elsewhere = Paths::rooted(Path::new("/tmp/a"), Path::new("/tmp/b"));
    assert_eq!(elsewhere.env_file(), Path::new("/tmp/a/lemonfiber/.env"));
    assert_eq!(elsewhere.stack(), Path::new("/tmp/b/lemonfiber/stack"));
}
/// Beside the settings, for the same reason the baseline is: what one operator
/// has been told is not what another has, even sharing a stack directory.
#[test]
fn what_has_been_acknowledged_sits_with_the_configuration() {
    let paths = paths();

    let held = paths.acknowledged();

    let where_it_is = held.display().to_string();
    assert_eq!(held.parent(), paths.baseline().parent());
    assert!(held.ends_with("acknowledged.json"), "{where_it_is}");
}
