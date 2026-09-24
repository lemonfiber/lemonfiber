//! Writing a region into one of the stack's own files, and taking it back out.
//!
//! The write and its undo are the two halves of [`crate::journal::Kind::Region`], and
//! both halves do one more thing than edit the file: they keep the record of what
//! lemonfiber materialised true. That record is how the pass that writes the stack's
//! files tells an operator's edit from lemonfiber's own. A region written without
//! re-recording the file would read to that pass as the operator's edit, and the
//! file would be preserved as theirs from then on, never updated again. So the
//! checksum moves with the region, inside the same write, and only where the record
//! was already holding the file as lemonfiber last wrote it. A file the operator had
//! edited before the region went in is still theirs afterwards, and still reads that
//! way.
//!
//! Where the file is kept is the caller's business. The journal entry is written
//! before the region is, by the caller, for the reason every write is journalled
//! first: a run that dies between the two leaves a record of a region that may not be
//! there, and taking out a region that is not there is nothing.

use std::path::Path;

use crate::materialised::{checksum, Materialised};

/// Where the record of what lemonfiber materialised is kept, beside the settings file.
///
/// The same answer the lifecycle commands derive, so the record a region re-records
/// is the one the next pass over the stack reads.
pub(crate) fn record_beside(env_file: &Path) -> std::path::PathBuf {
    env_file.with_file_name("materialised.json")
}

/// Write `owner`'s region holding `body` into the file at `path`.
///
/// The file has to be there already: a region is written into a file the stack has,
/// never into one this brings into being.
///
/// # Errors
///
/// Where the file cannot be read or written, in the operating system's own words.
pub(crate) fn put(
    path: &Path,
    key: &str,
    owner: &str,
    body: &str,
    record: Option<&Path>,
) -> Result<(), String> {
    let before = std::fs::read_to_string(path).map_err(|why| why.to_string())?;
    let after = crate::region::put(&before, owner, body);
    rewritten(path, key, &before, &after, record)
}

/// What taking a region back out came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Withdrawn {
    /// It was taken out, or there was nothing left of it to take.
    Done,
    /// It is not what was written any more — edited, or with its markers edited — so
    /// it was left exactly where it is.
    TheirsNow,
}

/// Take `owner`'s region back out of the file at `path`, where it is still exactly
/// the region that was written.
///
/// A file that is gone has no region left in it, which is nothing to do. The
/// judgement before a reversal already refuses a region somebody has edited; this
/// asks again because the file is read here and not there, and a region edited in
/// between is still not lemonfiber's to take.
///
/// # Errors
///
/// Where the file is there and cannot be read or written.
pub(crate) fn withdraw(
    path: &Path,
    key: &str,
    owner: &str,
    written: u32,
    record: Option<&Path>,
) -> Result<Withdrawn, String> {
    let before = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => return Ok(Withdrawn::Done),
        Err(why) => return Err(why.to_string()),
    };
    match crate::region::within(&before, owner) {
        Some(body) if checksum(body.as_bytes()) == written => {
            let after = crate::region::without(&before, owner).unwrap_or_default();
            rewritten(path, key, &before, &after, record).map(|()| Withdrawn::Done)
        }
        _ => Ok(Withdrawn::TheirsNow),
    }
}

/// The checksum a region's body is journalled under.
#[must_use]
pub(crate) fn written(body: &str) -> u32 {
    checksum(body.as_bytes())
}

/// Write the file's new text, and carry the record of what was materialised with it
/// where the record was holding the file as it stood.
fn rewritten(
    path: &Path,
    key: &str,
    before: &str,
    after: &str,
    record: Option<&Path>,
) -> Result<(), String> {
    // Written in place, as the file it already is: its mode stays what the stack gave it,
    // because the container reading it may not be its owner, and it stays the same file,
    // because a container given one file mounts that file and not whatever replaces it.
    std::fs::write(path, after).map_err(|why| why.to_string())?;
    let mut kept: Materialised = super::record::kept(record);
    if kept.checksum(key) == Some(checksum(before.as_bytes())) {
        kept.record(key, checksum(after.as_bytes()));
        // Best effort, as every write of this record is: a record that could not be
        // kept costs the next pass preserving a file it could have left as it was,
        // which is the safe direction.
        let _ = super::record::keep(record, &kept);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
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
}
