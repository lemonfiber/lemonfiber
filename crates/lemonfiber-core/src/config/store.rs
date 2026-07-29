//! Reading and changing settings on disk.
//!
//! One setting at a time, in place, leaving the file otherwise exactly as it
//! was. The operator's own edits and comments are the reason this is not simply
//! a serialised struct.

use std::path::{Path, PathBuf};

use thiserror::Error;

use super::env::EnvFile;
use crate::error::{Code, Diagnose, Problem, Remedy, Severity, State};

/// Parts of a setting's name that mean its value must never be shown.
///
/// Matched on the name rather than the value, because a credential is not
/// recognisable by looking at it — and a rule that tried would be wrong in the
/// direction that publishes one.
const SECRET_MARKERS: &[&str] = &[
    "KEY",
    "PASS",
    "SECRET",
    "TOKEN",
    "PRIVATE",
    "CREDENTIAL",
    "AUTH",
];

/// What is shown in place of a secret.
pub const REDACTED: &str = "(set, not shown)";

/// Whether a setting's value must be withheld when configuration is displayed.
#[must_use]
pub fn is_secret(key: &str) -> bool {
    let upper = key.to_ascii_uppercase();
    SECRET_MARKERS.iter().any(|marker| upper.contains(marker))
}

/// One setting, as it is safe to display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shown {
    /// The setting's name.
    pub key: String,
    /// Its value, or a note that it is set but withheld.
    pub value: String,
    /// Whether the value was withheld.
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
    file.set(key, value);
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
    file.remove(key);
    write(path, &file.render())
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
    if let Err(err) = std::fs::write(path, text).and_then(|()| make_private(path)) {
        return Err(unwritable(path, &err));
    }
    Ok(())
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
        .map(|key| {
            let secret = is_secret(key);
            let value = file.get(key).unwrap_or_default();
            Shown {
                key: key.to_owned(),
                value: if secret && !value.is_empty() {
                    REDACTED.to_owned()
                } else {
                    value.to_owned()
                },
                secret,
            }
        })
        .collect()
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
}

/// Raised when configuration exists and cannot be read.
pub const CONFIG_UNREADABLE: Code = Code::new("CONFIG-1");

/// Raised when configuration cannot be written.
pub const CONFIG_NOT_WRITTEN: Code = Code::new("CONFIG-2");

/// Raised when there is nowhere to keep configuration.
pub const CONFIG_NOWHERE: Code = Code::new("CONFIG-3");

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
                Remedy::new("Run setup").with_detail("lemonfiber init"),
            )
            .in_state(State::Guided),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{is_secret, read, set, shown, unset, Diagnose, Failure, REDACTED};
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
            std::fs::read_to_string(&path).ok().as_deref(),
            Some("B=2\n")
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
            std::fs::read_to_string(&path).ok().as_deref(),
            Some("A=3\nB=2\n")
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
        ];
        for failure in &failures {
            assert!(!failure.to_string().is_empty());
            assert!(!failure.problem().remedies.is_empty());
        }
    }
}
