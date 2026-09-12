//! Reading and changing settings on disk.
//!
//! One setting at a time, in place, leaving the file otherwise exactly as it
//! was. The operator's own edits and comments are the reason this is not simply
//! a serialised struct.

use std::path::{Path, PathBuf};

use thiserror::Error;

use super::env::EnvFile;
use lemonfiber_ports::error::{Code, Diagnose, Problem, Remedy, Severity, State};

/// Withholding a credential from text that has no field names to read.
///
/// The rule for prose — an error's detail, a condition, a line quoted back. Which
/// settings are *displayed*, where there are names to read, is decided by the
/// allow-list in [`super::display`] instead: a keyword rule cannot answer about a
/// name nobody has thought of yet, and that is the name that leaks. A file's line has
/// names to read, so it goes through `withheld_by` with that same list.
pub use lemonfiber_ports::withheld::{is_secret, withheld, withheld_by, withheld_text, REDACTED};

/// The setting recording which lemonfiber last wrote this file.
///
/// Kept in the settings file itself rather than beside it, because it is a fact
/// about that file and has to travel with it — a marker in a second file is one a
/// restore, a copy to another machine, or an operator moving their configuration
/// by hand leaves behind, and a marker that goes missing reads as permission.
pub const WRITTEN_BY_KEY: &str = "LEMONFIBER_CONFIG_VERSION";

/// The build doing the writing, which is what a marker is compared against.
const RUNNING: &str = env!("CARGO_PKG_VERSION");

/// One setting, as it is safe to display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shown {
    /// The setting's name.
    pub key: String,
    /// Its value, or a note that it is set but withheld.
    pub value: String,
    /// Whether the value is withheld rather than displayed.
    pub secret: bool,
}

/// Read the configuration file, or an empty one where none has been written.
///
/// A missing file is not a failure: it is what a machine looks like before
/// setup has run, and reading it should say "nothing is configured" rather than
/// refuse.
///
/// # Errors
///
/// Returns [`Failure`] when a file exists and cannot be read.
pub fn read(path: &Path) -> Result<EnvFile, Failure> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(EnvFile::parse(&text)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(EnvFile::default()),
        Err(err) => Err(Failure::Unreadable {
            path: path.to_path_buf(),
            reason: err.to_string(),
        }),
    }
}

/// Change one setting, leaving the rest of the file as it was.
///
/// # Errors
///
/// Returns [`Failure`] when the file cannot be read or written.
pub fn set(path: &Path, key: &str, value: &str) -> Result<(), Failure> {
    let mut file = read(path)?;
    refuse_if_newer(path, &file)?;
    file.set(key, value);
    stamped(&mut file);
    write(path, &file.render())
}

/// Remove a setting, restoring the file to not having it — what undoes an added
/// key on a rolled-back apply.
///
/// # Errors
///
/// Returns [`Failure`] when the file cannot be read or written.
pub fn unset(path: &Path, key: &str) -> Result<(), Failure> {
    let mut file = read(path)?;
    refuse_if_newer(path, &file)?;
    file.remove(key);
    stamped(&mut file);
    write(path, &file.render())
}

/// Refuse to change settings a newer lemonfiber wrote.
///
/// A newer build may have written keys this one has never heard of, and keys it
/// reads may have changed what they mean. Rewriting such a file would not lose
/// those lines — the file is held as the lines it is made of, so everything this
/// build does not recognise survives untouched — but it would stamp this older
/// build over a file it does not understand, and an operator who downgraded to
/// test something would have no way back to the configuration they had.
///
/// So the modification is refused and the file is left exactly as it was. Reading
/// is not refused with it: an older build that cannot safely *change* this file can
/// still say what is in it and what it is running, and an operator who has just
/// been refused needs precisely that.
///
/// A file with no marker is one written before this was recorded, or by hand. It is
/// changed, and gains a marker in the doing — refusing everything of unknown
/// provenance would refuse every configuration written before this existed.
fn refuse_if_newer(path: &Path, file: &EnvFile) -> Result<(), Failure> {
    let wrote = file.get(WRITTEN_BY_KEY).unwrap_or_default();
    if crate::version::Version::is_newer(wrote, RUNNING) {
        return Err(Failure::TooNew {
            path: path.to_path_buf(),
            wrote: wrote.to_owned(),
            running: RUNNING.to_owned(),
        });
    }
    Ok(())
}

/// Record this build as the one that last wrote the file.
///
/// Every change goes through [`set`] or [`unset`], so stamping in both is stamping
/// on every path that writes settings — setup, reconfiguration, restore, seeding
/// and a rollback putting a value back all arrive here.
fn stamped(file: &mut EnvFile) {
    file.set(WRITTEN_BY_KEY, RUNNING);
}

/// Write `text` to `path`, creating the directory for it where needed and
/// keeping both private to their owner.
///
/// The one place a small lemonfiber-owned file is put on disk, so the wizard's
/// progress and change journal land the same way a setting does — and report the
/// same [`Failure::NotWritten`] where they cannot. Every file that lands here may
/// hold a credential — the indexer key, the Usenet password, the VPN private key
/// — so where the platform has the notion the directory is created `0700` and the
/// file tightened to `0600`: another user on the same machine, the common shape
/// of a self-hosted host, must not be able to read what setup wrote.
///
/// Creating the directory and writing the file are one operation as far as the
/// operator is concerned, so they share one failure rather than two that say the
/// same thing. Written with `if let` rather than `map_err`, and without a block
/// around the directory: a closure is a function of its own for coverage
/// purposes, and one that only runs on failure is a symbol no passing test
/// reaches in every build of this crate. The private-mode step is folded into the
/// write's own result for the same reason — one failure path, already exercised,
/// rather than a second that only a chmod refusal on a just-written file reaches.
/// A path with no usable parent — a filesystem root, or a bare relative name whose
/// parent is the empty string — is written in the current directory rather than
/// under `create_dir_all("")`.
///
/// # Errors
///
/// Returns [`Failure::NotWritten`] where the directory could not be created or
/// the file could not be written.
pub(crate) fn write(path: &Path, text: &str) -> Result<(), Failure> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    if let Err(err) = make_private_dir(parent) {
        return Err(unwritable(path, &err));
    }
    if let Err(err) = write_owner_only(path, text).and_then(|()| make_private(path)) {
        return Err(unwritable(path, &err));
    }
    Ok(())
}

/// Write `text`, creating the file owner-only from the outset where the platform
/// tracks a file mode, so a secret is never even briefly world-readable in the gap
/// between creation and tightening. `mode` applies only to a file this creates; an
/// existing one keeps its mode until the [`make_private`] that follows corrects it.
#[cfg(unix)]
fn write_owner_only(path: &Path, text: &str) -> std::io::Result<()> {
    use std::io::Write as _;
    use std::os::unix::fs::OpenOptionsExt as _;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(text.as_bytes())
}

/// Where the platform has no owner-only mode to set at creation, an ordinary write.
#[cfg(not(unix))]
fn write_owner_only(path: &Path, text: &str) -> std::io::Result<()> {
    std::fs::write(path, text)
}

/// Create the configuration directory, private to its owner where the platform
/// tracks ownership. The mode is set as the directory is created, so an existing
/// one — a parent like `~/.config` this does not own — is left exactly as it was.
#[cfg(unix)]
fn make_private_dir(parent: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::DirBuilderExt as _;
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(parent)
}

/// Elsewhere there is no owner-only notion to honour, so this is an ordinary
/// recursive create.
#[cfg(not(unix))]
fn make_private_dir(parent: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(parent)
}

/// Tighten a just-written file to its owner alone. Applied every write, so a file
/// left `0644` by an earlier version is corrected the next time it is touched.
#[cfg(unix)]
fn make_private(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
}

/// A no-op where the platform has no owner-only file mode to set.
#[cfg(not(unix))]
fn make_private(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

/// The failure for a location that would not take what was written to it.
fn unwritable(path: &Path, err: &std::io::Error) -> Failure {
    Failure::NotWritten {
        path: path.to_path_buf(),
        reason: err.to_string(),
    }
}

/// Every setting, with secrets withheld.
#[must_use]
pub fn shown(file: &EnvFile) -> Vec<Shown> {
    file.keys()
        .into_iter()
        .map(|key| showing(key, file.get(key).unwrap_or_default()))
        .collect()
}

/// One setting as it is safe to display: a name nobody has vouched for keeps its
/// name and loses its value.
///
/// Decided by [`super::display::in_full`] rather than by reading the name for words
/// that sound like a credential. A keyword rule answers about the names somebody
/// thought of; this surface serves whatever is in the operator's file, and the
/// setting that leaks is the one nobody thought of. So the default is to withhold and
/// the exception costs somebody a sentence on the list.
///
/// A pair rather than a whole file, because the settings a run is about to write
/// are shown before there is a file holding them — and a review that redacted by
/// its own rule would be a second rule to keep in step with this one.
#[must_use]
pub fn showing(key: &str, value: &str) -> Shown {
    let displayed = super::display::in_full(key);
    Shown {
        key: key.to_owned(),
        // An empty setting reads as empty either way. A credential that is not set and
        // one that is are different faults, and saying "(set, not shown)" about a blank
        // would report the wrong one.
        value: match (displayed, value.is_empty()) {
            (_, true) => String::new(),
            (true, false) => super::display::without_credentials(value),
            (false, false) => REDACTED.to_owned(),
        },
        secret: !displayed,
    }
}

/// Configuration could not be read or changed.
#[derive(Debug, Error)]
pub enum Failure {
    /// A configuration file exists and could not be read.
    #[error("the configuration at {path} could not be read: {reason}")]
    Unreadable {
        /// The file, in full.
        path: PathBuf,
        /// The operating system's own words.
        reason: String,
    },
    /// Configuration could not be written.
    #[error("the configuration at {path} could not be written: {reason}")]
    NotWritten {
        /// The file, in full.
        path: PathBuf,
        /// The operating system's own words.
        reason: String,
    },
    /// There is nowhere to keep configuration.
    #[error("no configuration file has been chosen")]
    Nowhere,
    /// The configuration was written by a newer lemonfiber than the one running.
    #[error("the configuration at {path} was written by lemonfiber {wrote} and this is {running}")]
    TooNew {
        /// The file, in full.
        path: PathBuf,
        /// The version that wrote it.
        wrote: String,
        /// The version being asked to change it.
        running: String,
    },
}

/// Raised when configuration exists and cannot be read.
pub const CONFIG_UNREADABLE: Code = Code::new("CONFIG-1");

/// Raised when configuration cannot be written.
pub const CONFIG_NOT_WRITTEN: Code = Code::new("CONFIG-2");

/// Raised when there is nowhere to keep configuration.
pub const CONFIG_NOWHERE: Code = Code::new("CONFIG-3");

/// Raised when configuration was written by a newer lemonfiber.
pub const CONFIG_TOO_NEW: Code = Code::new("CONFIG-5");

impl Diagnose for Failure {
    fn problem(&self) -> Problem {
        match self {
            Self::Unreadable { path, reason } => Problem::new(
                CONFIG_UNREADABLE,
                Severity::Error,
                format!("Your settings at {} could not be read", path.display()),
                "Nothing has been changed. lemonfiber will not guess at settings it cannot read, because guessing wrong here starts the wrong things.",
                Remedy::new("Check that the file is readable"),
            )
            .in_state(State::Guided)
            .with_detail(reason.clone()),
            Self::NotWritten { path, reason } => Problem::new(
                CONFIG_NOT_WRITTEN,
                Severity::Error,
                format!("Your settings at {} could not be saved", path.display()),
                "The change was not made. Your existing settings are untouched.",
                Remedy::new("Check that the location is writable and has space"),
            )
            .in_state(State::Guided)
            .with_detail(reason.clone()),
            Self::Nowhere => Problem::new(
                CONFIG_NOWHERE,
                Severity::Error,
                "lemonfiber has not been set up on this machine yet",
                "There is nowhere to keep settings until setup has chosen a location for them.",
                Remedy::new("Run setup").with_detail("lemonfiber setup"),
            )
            .in_state(State::Guided),
            Self::TooNew {
                path,
                wrote,
                running,
            } => Problem::new(
                CONFIG_TOO_NEW,
                Severity::Error,
                format!("Your settings were written by lemonfiber {wrote}, and this is {running}"),
                "Nothing has been changed. An older lemonfiber writing over settings a newer one wrote would leave you with a file neither version can make sense of, and no way back to the one you had.",
                Remedy::new(format!("Run this with lemonfiber {wrote} or newer"))
                    .with_detail("lemonfiber update self"),
            )
            .in_state(State::Guided)
            .with_detail(format!("the settings are at {}", path.display())),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{
        is_secret, read, set, shown, unset, Diagnose, Failure, REDACTED, RUNNING, WRITTEN_BY_KEY,
    };
    use crate::config::env::EnvFile;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("lemonfiber-cfg-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join(".env")
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
            Some(format!("B=2\n{WRITTEN_BY_KEY}={RUNNING}\n"))
        );
        let _ = std::fs::remove_dir_all(path.parent().unwrap_or(Path::new("/")));
    }

    #[test]
    fn changing_one_setting_leaves_the_rest_of_the_file_alone() {
        let path = scratch("preserve");
        assert!(set(&path, "A", "1").is_ok());
        assert!(set(&path, "B", "2").is_ok());
        assert!(set(&path, "A", "3").is_ok());

        assert_eq!(
            std::fs::read_to_string(&path).ok(),
            Some(format!("A=3\nB=2\n{WRITTEN_BY_KEY}={RUNNING}\n"))
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
            Failure::TooNew {
                path: "/tmp/x/.env".into(),
                wrote: "9.0.0".to_owned(),
                running: "0.1.0".to_owned(),
            },
        ];
        for failure in &failures {
            assert!(!failure.to_string().is_empty());
            assert!(!failure.problem().remedies.is_empty());
        }
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

    /// The marker is a version number, and a version number is not a credential —
    /// an operator who has just been refused has to be able to read the one thing
    /// the refusal is about.
    #[test]
    fn the_marker_is_shown_rather_than_withheld() {
        let file = EnvFile::parse(&format!("{WRITTEN_BY_KEY}=9.9.9\n"));
        assert_eq!(
            shown(&file).first().map(|entry| entry.value.clone()),
            Some("9.9.9".to_owned())
        );
    }
}
