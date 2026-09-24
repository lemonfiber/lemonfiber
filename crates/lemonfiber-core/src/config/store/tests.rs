use std::path::{Path, PathBuf};

use super::{
    is_secret, read, set, shown, unset, Diagnose, Failure, REDACTED, RUNNING, WRITTEN_BY_KEY,
};
use crate::config::env::EnvFile;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("lemonfiber-cfg-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir.join(".env")
}

/// What stands in for a checksum of this file: a write cannot lose anything,
/// because it replaces one line and appends at most one more.
///
/// The module doc above argues that a whole-file comparison is the wrong
/// instrument here, and the argument rests entirely on this. A setting this build
/// has never heard of, a comment, and the blank lines between them all have to
/// come back exactly as they went in — otherwise something an operator wrote is
/// being overwritten with no diff and no consent, which is the thing the stack
/// directory keeps a record of what it wrote to avoid.
///
/// Asserted as a prefix rather than line by line, which is the stronger claim:
/// what they wrote is still the opening of the file, in order, unshifted, with
/// the new setting after it.
#[test]
fn a_write_leaves_everything_it_does_not_name_byte_for_byte() {
    let path = scratch("untouched");
    let theirs = "# my own notes about this stack\n\n\
                  SOMETHING_LEMONFIBER_HAS_NEVER_HEARD_OF=mine\n\nPUID=1000";
    assert!(crate::config::store::write(&path, theirs).is_ok());

    assert!(set(&path, "PGID", "1000").is_ok());

    let after = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(
        after.starts_with(theirs),
        "a write disturbed what the operator had written: {after}"
    );
    // And the setting that was asked for is the one that arrived.
    assert_eq!(
        read(&path)
            .ok()
            .and_then(|file| file.get("PGID").map(str::to_owned)),
        Some("1000".to_owned())
    );
}

/// Only on unix: the guarantee is a file mode, which is the platform's own
/// notion. A credential setup writes must not be readable by another user.
#[cfg(unix)]
#[test]
fn a_setting_is_written_private_to_its_owner() {
    use std::os::unix::fs::PermissionsExt as _;

    let path = scratch("private");
    assert!(set(&path, "USENET_PASS", "hunter2").is_ok());

    let mode = |p: &Path| std::fs::metadata(p).map(|m| m.permissions().mode() & 0o777);
    assert_eq!(
        mode(&path).ok(),
        Some(0o600),
        "the file is readable only by its owner"
    );
    assert_eq!(
        mode(path.parent().unwrap_or(Path::new("/"))).ok(),
        Some(0o700),
        "and so is the directory that holds it"
    );
    let _ = std::fs::remove_dir_all(path.parent().unwrap_or(Path::new("/")));
}

#[test]
fn a_machine_with_no_settings_reads_as_nothing_configured() {
    let missing = Path::new("/lemonfiber/no/such/config/.env");
    assert_eq!(read(missing).ok().map(|file| file.keys().len()), Some(0));
}

#[test]
fn a_setting_survives_a_round_trip_through_disk() {
    let path = scratch("round-trip");
    assert!(set(&path, "LEMONFIBER_USENET", "on").is_ok());
    assert_eq!(
        read(&path)
            .ok()
            .and_then(|file| file.get("LEMONFIBER_USENET").map(ToOwned::to_owned)),
        Some("on".to_owned())
    );
    let _ = std::fs::remove_dir_all(path.parent().unwrap_or(Path::new("/")));
}

#[test]
fn unsetting_a_setting_removes_it_and_leaves_the_rest() {
    let path = scratch("unset");
    assert!(set(&path, "A", "1").is_ok());
    assert!(set(&path, "B", "2").is_ok());
    assert!(unset(&path, "A").is_ok());

    assert_eq!(
        std::fs::read_to_string(&path).ok(),
        Some(format!("{WRITTEN_BY_KEY}={RUNNING}\nB=2\n"))
    );
    let _ = std::fs::remove_dir_all(path.parent().unwrap_or(Path::new("/")));
}

#[test]
fn changing_one_setting_leaves_the_rest_of_the_file_alone() {
    let path = scratch("preserve");
    assert!(set(&path, "A", "1").is_ok());
    assert!(set(&path, "B", "2").is_ok());
    assert!(set(&path, "A", "3").is_ok());

    // The marker sits where the first write appended it and is rewritten in
    // place after, rather than moving to the end each time. That is the whole
    // point of writing this file line by line: a setting an operator put under a
    // comment stays under it, and so does this.
    assert_eq!(
        std::fs::read_to_string(&path).ok(),
        Some(format!("A=3\n{WRITTEN_BY_KEY}={RUNNING}\nB=2\n"))
    );
    let _ = std::fs::remove_dir_all(path.parent().unwrap_or(Path::new("/")));
}

#[test]
fn a_setting_that_cannot_be_saved_says_where_and_changes_nothing() {
    // A file where a directory needs to be, which is the closest thing to a
    // permission failure that behaves the same way on every platform.
    let blocker = scratch("blocked");
    if let Some(parent) = blocker.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&blocker, "in the way");

    let beneath = blocker.join("deeper").join(".env");
    let refusal = set(&beneath, "A", "1").err();
    assert_eq!(
        refusal
            .as_ref()
            .map(|failure| failure.to_string().contains("deeper")),
        Some(true),
        "the refusal names the path in full"
    );
    assert_eq!(
        refusal.map(|failure| failure.problem().remedies.is_empty()),
        Some(false),
        "and offers something to do about it"
    );
    assert!(!beneath.exists(), "nothing was written");

    let _ = std::fs::remove_dir_all(blocker.parent().unwrap_or(Path::new("/")));
}

#[test]
fn a_value_carrying_a_line_break_writes_no_setting_at_all() {
    // The shape a container can put there without anybody typing it: an API
    // key read back out of a service's own configuration file, carrying a
    // break and two settings that would run the rest of the stack as root.
    let path = scratch("spanning");
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&path, "TZ=UTC\n");

    let refusal = set(&path, "INDEXER_APIKEY", "abc\nPUID=0\nPGID=0").err();
    assert!(
        matches!(refusal, Some(Failure::SpansLines { ref key, .. }) if key == "INDEXER_APIKEY"),
        "the refusal names the setting, and is its own kind of failure"
    );
    assert_eq!(
        refusal.map(|failure| failure.problem().remedies.is_empty()),
        Some(false),
        "and offers something to do about it"
    );

    let after = std::fs::read_to_string(&path).ok();
    assert_eq!(
        after,
        Some("TZ=UTC\n".to_owned()),
        "the file is byte for byte what it was — not the key, not the \
         injected settings, and not the marker a write would have moved"
    );

    // A carriage return alone is the same defect on the other line ending,
    // and a key is as capable of carrying one as a value.
    assert!(set(&path, "A", "1\rB=2").is_err());
    assert!(set(&path, "A\nB", "1").is_err());
    assert_eq!(
        std::fs::read_to_string(&path).ok(),
        Some("TZ=UTC\n".to_owned())
    );

    let _ = std::fs::remove_dir_all(path.parent().unwrap_or(Path::new("/")));
}

#[test]
fn a_configuration_that_cannot_be_read_is_refused_rather_than_assumed_empty() {
    let blocker = scratch("unreadable");
    if let Some(parent) = blocker.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&blocker, "in the way");

    assert!(matches!(
        read(&blocker.join("deeper").join(".env")),
        Err(Failure::Unreadable { .. })
    ));
    let _ = std::fs::remove_dir_all(blocker.parent().unwrap_or(Path::new("/")));
}

/// Only on unix: the check needs a directory that can be read and not
/// written, and permissions are the portable way to arrange that here.
#[cfg(unix)]
#[test]
fn a_setting_that_cannot_be_written_leaves_the_existing_file_alone() {
    use std::os::unix::fs::PermissionsExt as _;

    // Built as a directory and a name rather than derived with `parent()`,
    // which would need a branch for a case that cannot happen.
    let parent =
        std::env::temp_dir().join(format!("lemonfiber-cfg-{}-read-only", std::process::id()));
    let path = parent.join(".env");
    let _ = std::fs::remove_dir_all(&parent);
    let _ = std::fs::create_dir_all(&parent);
    let _ = std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o500));

    let refusal = set(&path, "A", "1").err();
    assert_eq!(
        refusal
            .as_ref()
            .map(|failure| matches!(failure, Failure::NotWritten { .. })),
        Some(true),
        "a directory that cannot be written to is reported as such"
    );
    assert_eq!(
        refusal.map(|failure| failure.problem().remedies.is_empty()),
        Some(false)
    );

    let _ = std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o700));
    let _ = std::fs::remove_dir_all(&parent);
}

/// Only on unix, for the same reason as the test above.
#[cfg(unix)]
#[test]
fn a_directory_that_cannot_be_created_is_reported_as_such() {
    use std::os::unix::fs::PermissionsExt as _;

    // The file itself does not exist and its parent does not either, so
    // reading succeeds as "nothing configured" and creating the directory
    // is what fails — the other half of writing.
    let outer =
        std::env::temp_dir().join(format!("lemonfiber-cfg-{}-no-mkdir", std::process::id()));
    let _ = std::fs::remove_dir_all(&outer);
    let _ = std::fs::create_dir_all(&outer);
    let _ = std::fs::set_permissions(&outer, std::fs::Permissions::from_mode(0o500));

    let path = outer.join("inner").join(".env");
    assert!(matches!(
        set(&path, "A", "1"),
        Err(Failure::NotWritten { .. })
    ));

    let _ = std::fs::set_permissions(&outer, std::fs::Permissions::from_mode(0o700));
    let _ = std::fs::remove_dir_all(&outer);
}

#[test]
fn a_credential_is_recognised_by_its_name_rather_than_its_value() {
    for key in [
        "WIREGUARD_PRIVATE_KEY",
        "QBITTORRENT_PASSWORD",
        // A password key that says PASS but not PASSWORD, which the stack's
        // own .env.example ships and an earlier marker list showed in the
        // clear.
        "HOMEPAGE_VAR_QBITTORRENT_PASS",
        "SONARR_API_KEY",
        "some_token",
        "SHARED_SECRET",
        "CREDENTIAL_X",
        "NORDVPN_AUTH",
    ] {
        assert!(is_secret(key), "{key} holds a credential");
    }
    for key in ["DATA_ROOT", "TZ", "PUID", "LAN_BIND", "LEMONFIBER_USENET"] {
        assert!(!is_secret(key), "{key} does not");
    }
}

#[test]
fn a_credential_is_never_shown() {
    let file = EnvFile::parse("DATA_ROOT=/media\nWIREGUARD_PRIVATE_KEY=abc123secret\n");
    let displayed = shown(&file);

    let rendered: Vec<String> = displayed
        .iter()
        .map(|entry| format!("{}={}", entry.key, entry.value))
        .collect();
    assert_eq!(
        rendered,
        vec![
            "DATA_ROOT=/media".to_owned(),
            format!("WIREGUARD_PRIVATE_KEY={REDACTED}"),
        ]
    );
    assert!(
        !rendered.join("\n").contains("abc123secret"),
        "the value must not appear anywhere"
    );
}

/// Not withheld, absent. Shown, it would read as something an operator may set;
/// withheld, as a credential they are not being trusted with. It is neither.
#[test]
fn the_marker_is_left_out_of_the_listing_rather_than_withheld() {
    let file = EnvFile::parse(&format!("DATA_ROOT=/media\n{WRITTEN_BY_KEY}=9.9.9\n"));
    assert_eq!(
        shown(&file)
            .into_iter()
            .map(|entry| entry.key)
            .collect::<Vec<String>>(),
        vec!["DATA_ROOT".to_owned()]
    );
}

#[test]
fn a_credential_that_is_not_set_says_so_rather_than_pretending() {
    let file = EnvFile::parse("WIREGUARD_PRIVATE_KEY=\n");
    assert_eq!(
        shown(&file).first().map(|entry| entry.value.clone()),
        Some(String::new()),
        "an empty credential is empty, not withheld"
    );
}

#[test]
fn every_failure_says_something_and_offers_something() {
    let failures = [
        Failure::Unreadable {
            path: "/tmp/x/.env".into(),
            reason: "denied".to_owned(),
        },
        Failure::NotWritten {
            path: "/tmp/x/.env".into(),
            reason: "full".to_owned(),
        },
        Failure::Nowhere,
        Failure::SpansLines {
            path: "/tmp/x/.env".into(),
            key: "INDEXER_APIKEY".to_owned(),
        },
        Failure::TooNew {
            path: "/tmp/x/.env".into(),
            wrote: "9.0.0".to_owned(),
            running: "0.1.0".to_owned(),
        },
    ];
    let mut codes = std::collections::BTreeSet::new();
    for failure in &failures {
        // Named exhaustively rather than swept past: a variant added without
        // a sample above stops this compiling, where a bare loop would have
        // gone on passing about the ones the array happened to hold.
        match failure {
            Failure::Unreadable { .. }
            | Failure::NotWritten { .. }
            | Failure::Nowhere
            | Failure::SpansLines { .. }
            | Failure::TooNew { .. } => {}
        }
        assert!(!failure.to_string().is_empty());
        assert!(!failure.problem().remedies.is_empty());
        codes.insert(failure.problem().code.as_str());
    }
    assert_eq!(
        codes.len(),
        failures.len(),
        "and each of them raises a code of its own, so a log names which"
    );
}

/// The file a newer build left behind, written by hand rather than through
/// [`set`] — which would stamp it with the running version and so could not
/// produce the situation being tested.
fn written_by(path: &Path, version: &str) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(
        path,
        format!("DATA_ROOT=/media\n{WRITTEN_BY_KEY}={version}\n"),
    );
}

#[test]
fn settings_a_newer_lemonfiber_wrote_are_refused_rather_than_changed() {
    let path = scratch("too-new");
    written_by(&path, "99.0.0");
    let before = std::fs::read_to_string(&path).unwrap_or_default();

    let refusal = set(&path, "DATA_ROOT", "/elsewhere").err();
    assert_eq!(
        refusal.as_ref().map(|failure| matches!(
            failure,
            Failure::TooNew { wrote, running, .. }
                if wrote == "99.0.0" && running == RUNNING
        )),
        Some(true),
        "the refusal names both versions"
    );
    assert_eq!(
        refusal.map(|failure| failure.problem().remedies.is_empty()),
        Some(false),
        "and offers something to do about it"
    );
    assert_eq!(
        std::fs::read_to_string(&path).ok(),
        Some(before),
        "the file is exactly as it was"
    );

    let _ = std::fs::remove_dir_all(path.parent().unwrap_or(Path::new("/")));
}

#[test]
fn removing_a_setting_a_newer_lemonfiber_wrote_is_refused_too() {
    let path = scratch("too-new-unset");
    written_by(&path, "99.0.0");
    let before = std::fs::read_to_string(&path).unwrap_or_default();

    assert!(matches!(
        unset(&path, "DATA_ROOT"),
        Err(Failure::TooNew { .. })
    ));
    assert_eq!(
        std::fs::read_to_string(&path).ok(),
        Some(before),
        "removing is a change, and is refused the same way"
    );

    let _ = std::fs::remove_dir_all(path.parent().unwrap_or(Path::new("/")));
}

#[test]
fn settings_an_older_lemonfiber_wrote_are_changed_and_the_marker_moves_on() {
    let path = scratch("older");
    written_by(&path, "0.0.1");

    assert!(set(&path, "DATA_ROOT", "/elsewhere").is_ok());
    assert_eq!(
        read(&path)
            .ok()
            .and_then(|file| file.get(WRITTEN_BY_KEY).map(ToOwned::to_owned)),
        Some(RUNNING.to_owned()),
        "the build that wrote it last is the one recorded"
    );

    let _ = std::fs::remove_dir_all(path.parent().unwrap_or(Path::new("/")));
}

#[test]
fn settings_with_no_marker_are_changed_and_gain_one() {
    let path = scratch("unmarked");
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&path, "DATA_ROOT=/media\n");

    assert!(set(&path, "TZ", "Pacific/Auckland").is_ok());
    assert_eq!(
        read(&path)
            .ok()
            .and_then(|file| file.get(WRITTEN_BY_KEY).map(ToOwned::to_owned)),
        Some(RUNNING.to_owned()),
        "a file written before the marker existed is not refused over not having one"
    );

    let _ = std::fs::remove_dir_all(path.parent().unwrap_or(Path::new("/")));
}

/// A hand-edited marker is not grounds to refuse. Nothing can be concluded from
/// a version that will not parse, and concluding "newer" from one would lock an
/// operator out of their own settings over a typo.
#[test]
fn a_marker_that_cannot_be_read_is_not_treated_as_newer() {
    let path = scratch("unreadable-marker");
    written_by(&path, "tomorrow's build");

    assert!(set(&path, "TZ", "Pacific/Auckland").is_ok());
    assert_eq!(
        read(&path)
            .ok()
            .and_then(|file| file.get(WRITTEN_BY_KEY).map(ToOwned::to_owned)),
        Some(RUNNING.to_owned())
    );

    let _ = std::fs::remove_dir_all(path.parent().unwrap_or(Path::new("/")));
}

/// Where an operator actually reads the two numbers: in the refusal that is about
/// them, not in a settings listing they would have to go and find.
///
/// The running side is read off the constant rather than written into the
/// sentence, so a release cannot move the version and leave the message claiming
/// the one before it.
#[test]
fn the_refusal_names_the_version_that_wrote_it_and_the_one_refusing() {
    let path = scratch("both-versions");
    written_by(&path, "99.0.0");

    let said = set(&path, "DATA_ROOT", "/elsewhere").err().map(|failure| {
        let problem = failure.problem();
        (
            problem.summary.clone(),
            problem.remedies.first().map(|remedy| remedy.action.clone()),
        )
    });

    // Asserted whole rather than by two `contains` with a formatted message
    // between them. A closure in an assertion's *message* only runs when the
    // assertion fails, which makes it a function no passing test enters and a
    // line the coverage gate counts against this file — and asking for the
    // exact sentence is the stronger claim anyway.
    assert_eq!(
        said,
        Some((
            format!("Your settings were written by lemonfiber 99.0.0, and this is {RUNNING}"),
            Some("Run this with lemonfiber 99.0.0 or newer".to_owned()),
        )),
        "the refusal names the version that wrote the file and the one refusing it"
    );

    let _ = std::fs::remove_dir_all(path.parent().unwrap_or(Path::new("/")));
}
