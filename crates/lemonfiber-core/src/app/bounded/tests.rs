use super::{put, record_beside, withdraw, written, Withdrawn};
use crate::materialised::{checksum, Materialised};

const SHIPPED: &str = "watch {\n}\n";

/// A scratch directory unique to the named test, holding the proxy's file as it
/// ships and a record saying lemonfiber wrote it that way.
fn scratch(name: &str, recorded: bool) -> std::path::PathBuf {
    let dir =
        std::env::temp_dir().join(format!("lemonfiber-bounded-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(dir.join("Caddyfile"), SHIPPED);
    if recorded {
        let mut record = Materialised::new();
        record.record("config/caddy/Caddyfile", checksum(SHIPPED.as_bytes()));
        let _ = std::fs::write(
            dir.join("materialised.json"),
            serde_json::to_string(&record).unwrap_or_default(),
        );
    }
    dir
}

fn recorded(dir: &std::path::Path) -> Option<u32> {
    super::super::record::kept::<Materialised>(Some(&dir.join("materialised.json")))
        .checksum("config/caddy/Caddyfile")
}

fn read(dir: &std::path::Path) -> String {
    std::fs::read_to_string(dir.join("Caddyfile")).unwrap_or_default()
}

#[test]
fn a_region_is_written_and_the_record_moves_with_it() {
    let dir = scratch("put", true);
    let record = dir.join("materialised.json");

    let done = put(
        &dir.join("Caddyfile"),
        "config/caddy/Caddyfile",
        "plugin komga",
        "comics {\n}\n",
        Some(&record),
    );

    assert_eq!(done, Ok(()));
    let now = read(&dir);
    assert_eq!(
        crate::region::within(&now, "plugin komga").as_deref(),
        Some("comics {\n}\n")
    );
    assert_eq!(recorded(&dir), Some(checksum(now.as_bytes())));
}

/// A file the operator had already edited is still theirs after a region goes in,
/// so the record keeps saying what lemonfiber last wrote rather than adopting it.
#[test]
fn a_file_the_operator_had_edited_is_written_into_and_still_read_as_theirs() {
    let dir = scratch("edited", true);
    let _ = std::fs::write(dir.join("Caddyfile"), "their own {\n}\n");

    let done = put(
        &dir.join("Caddyfile"),
        "config/caddy/Caddyfile",
        "plugin komga",
        "comics\n",
        Some(&dir.join("materialised.json")),
    );

    assert_eq!(done, Ok(()));
    assert_eq!(recorded(&dir), Some(checksum(SHIPPED.as_bytes())));
}

#[test]
fn taking_the_region_out_gives_back_the_file_and_the_record() {
    let dir = scratch("withdraw", true);
    let record = dir.join("materialised.json");
    let file = dir.join("Caddyfile");
    let _ = put(
        &file,
        "config/caddy/Caddyfile",
        "plugin komga",
        "comics\n",
        Some(&record),
    );

    let taken = withdraw(
        &file,
        "config/caddy/Caddyfile",
        "plugin komga",
        written("comics\n"),
        Some(&record),
    );

    assert_eq!(taken, Ok(Withdrawn::Done));
    assert_eq!(read(&dir), SHIPPED);
    assert_eq!(recorded(&dir), Some(checksum(SHIPPED.as_bytes())));
}

#[test]
fn a_region_edited_since_is_left_where_it_is() {
    let dir = scratch("theirs", false);
    let file = dir.join("Caddyfile");
    let _ = put(
        &file,
        "config/caddy/Caddyfile",
        "plugin komga",
        "comics\n",
        None,
    );
    let edited = read(&dir).replace("comics\n", "comics, mine\n");
    let _ = std::fs::write(&file, &edited);

    let taken = withdraw(
        &file,
        "config/caddy/Caddyfile",
        "plugin komga",
        written("comics\n"),
        None,
    );

    assert_eq!(taken, Ok(Withdrawn::TheirsNow));
    assert_eq!(read(&dir), edited);
}

#[test]
fn a_file_that_is_gone_has_nothing_left_to_take() {
    let dir = scratch("gone", false);
    let file = dir.join("Caddyfile");
    let _ = std::fs::remove_file(&file);

    assert_eq!(
        withdraw(&file, "config/caddy/Caddyfile", "plugin komga", 0, None),
        Ok(Withdrawn::Done)
    );
}

#[test]
fn a_file_that_will_not_read_is_a_failure_rather_than_nothing() {
    let dir = scratch("unread", false);

    assert!(withdraw(&dir, "config", "plugin komga", 0, None).is_err());
    assert!(put(&dir.join("absent"), "config", "plugin komga", "x", None).is_err());
}

/// A file that reads and will not be written is a failure, and the record is left
/// as it was rather than moved to a region that did not land.
#[cfg(unix)]
#[test]
fn a_file_that_will_not_be_written_is_a_failure_and_the_record_stays() {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = scratch("readonly", true);
    let file = dir.join("Caddyfile");
    let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555));
    let _ = std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o444));

    let done = put(
        &file,
        "config/caddy/Caddyfile",
        "plugin komga",
        "komga\n",
        Some(&dir.join("materialised.json")),
    );

    let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755));
    let _ = std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644));
    assert!(done.is_err(), "{done:?}");
    assert_eq!(recorded(&dir), Some(checksum(SHIPPED.as_bytes())));
}

/// Written in place: the file keeps the mode the stack gave it, because a container
/// reading it may not be its owner.
#[cfg(unix)]
#[test]
fn a_region_leaves_the_file_readable_by_whoever_could_read_it() {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = scratch("mode", false);
    let file = dir.join("Caddyfile");
    let _ = std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644));

    let _ = put(
        &file,
        "config/caddy/Caddyfile",
        "plugin komga",
        "komga\n",
        None,
    );

    assert_eq!(
        std::fs::metadata(&file)
            .ok()
            .map(|meta| meta.permissions().mode() & 0o777),
        Some(0o644)
    );
}

#[test]
fn the_record_is_kept_beside_the_settings_file() {
    assert_eq!(
        record_beside(std::path::Path::new("/etc/lemonfiber/.env")),
        std::path::PathBuf::from("/etc/lemonfiber/materialised.json")
    );
}
