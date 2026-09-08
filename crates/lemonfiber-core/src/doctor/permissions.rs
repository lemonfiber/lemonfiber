//! Whether the files holding credentials can be read by anyone but their owner.
//!
//! lemonfiber writes these files owner-only, at creation and again on every write, so
//! on a machine where nothing else has touched them this check passes and says so. It
//! exists for the machines where something has: a file restored from an archive that
//! did not carry modes, a directory copied under a permissive umask, a recursive
//! change aimed at something else. None of those announce themselves, and the whole
//! of what owner-only permissions buy is lost silently when one happens.
//!
//! What is checked is read off the same declaration that says which of the things
//! lemonfiber keeps holds a credential, so a file added there is guarded from the day
//! it is added rather than from the day somebody remembers this module exists.
//!
//! The stack's own configuration directory is deliberately not among them, although it
//! holds every service's API key. Those files are written by the services themselves,
//! inside their containers, under a umask lemonfiber does not set and cannot change —
//! a finding about them would be permanent, and no remedy offered here would fix it.
//!
//! See `.docs/architecture/module-layout.md`.

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;

use super::{Category, Check, Finding, Verdict};
use crate::config::paths::Paths;
use crate::error::{Code, Problem, Remedy, Severity};
use crate::ports::filesystem::FileSystem;

/// Raised when a file holding a credential can be read by more than its owner.
pub const CREDENTIALS_EXPOSED: Code = Code::new("CONFIG-4");

/// The permission bits that grant anyone but the owner anything at all.
const BEYOND_THE_OWNER: u32 = 0o077;

/// The accessor whose files the services write rather than lemonfiber.
const THE_SERVICES_OWN: &str = "service_config";

/// One file whose permissions are worth checking, and what it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Guarded {
    /// What it is, in the operator's words.
    pub what: String,
    /// Where it is.
    pub at: PathBuf,
}

/// One guarded file found open, with the mode it was found at.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Open {
    /// What it is, in the operator's words.
    what: String,
    /// Where it is.
    at: PathBuf,
    /// The permission bits it was found carrying.
    mode: u32,
}

impl Open {
    /// The file and its mode, as one clause of the detail.
    fn said(&self) -> String {
        format!(
            "{} at {} is {:04o}",
            self.what,
            self.at.display(),
            self.mode
        )
    }

    /// The command that closes it.
    fn tighten(&self) -> String {
        format!("chmod go-rwx {}", self.at.display())
    }
}

/// The files lemonfiber writes that hold a credential, against a resolved layout.
#[must_use]
pub fn guarded(paths: &Paths) -> Vec<Guarded> {
    crate::stored::EVERY
        .iter()
        .filter(|entry| entry.secret && entry.accessor != THE_SERVICES_OWN)
        .map(|entry| Guarded {
            what: entry.what.to_owned(),
            at: (entry.at)(paths),
        })
        .collect()
}

/// Proves that nothing but its owner can read the files holding credentials.
pub struct PermissionsCheck {
    filesystem: Arc<dyn FileSystem>,
    guarded: Vec<Guarded>,
}

impl PermissionsCheck {
    /// A check over the given filesystem and the files to guard.
    #[must_use]
    pub fn new(filesystem: Arc<dyn FileSystem>, guarded: Vec<Guarded>) -> Self {
        Self {
            filesystem,
            guarded,
        }
    }

    /// Every guarded file readable beyond its owner, and how many were read at all.
    ///
    /// The count comes back because it is the difference between "several were checked
    /// and all were fine" and "none could be read" — and those two must not both report
    /// a pass. A file that is not there yet contributes to neither.
    async fn open(&self) -> (usize, Vec<Open>) {
        let mut read = 0_usize;
        let mut open = Vec::new();
        for one in &self.guarded {
            let Some(ownership) = self.filesystem.ownership(&one.at).await else {
                continue;
            };
            read += 1;
            if ownership.mode & BEYOND_THE_OWNER != 0 {
                open.push(Open {
                    what: one.what.clone(),
                    at: one.at.clone(),
                    mode: ownership.mode,
                });
            }
        }
        (read, open)
    }
}

#[async_trait]
impl Check for PermissionsCheck {
    fn category(&self) -> Category {
        Category::Config
    }

    async fn run(&self) -> Vec<Finding> {
        let (read, open) = self.open().await;
        vec![Finding::in_category(
            Category::Config,
            "config.credential-permissions",
            "Credential files readable only by you",
            verdict(read, &open),
        )]
    }
}

/// What the reading came to.
///
/// Nothing read is never a pass. A run over an empty set would satisfy any "all of them
/// are fine" test while having established nothing, which is exactly the shape of a
/// guard that reports green on the machine it was written for.
fn verdict(read: usize, open: &[Open]) -> Verdict {
    if read == 0 {
        return Verdict::Skipped {
            reason: "none of the files lemonfiber keeps credentials in is on this machine yet, \
                     so there are no permissions to read"
                .to_owned(),
        };
    }
    if open.is_empty() {
        return Verdict::Pass {
            note: Some(format!("{read} of them, each readable only by you")),
        };
    }
    Verdict::Warn(exposed(open))
}

/// The finding for files anyone with an account on this machine could read.
fn exposed(open: &[Open]) -> Problem {
    let each: Vec<String> = open.iter().map(Open::said).collect();
    let tighten: Vec<String> = open.iter().map(Open::tighten).collect();
    Problem::new(
        CREDENTIALS_EXPOSED,
        Severity::Warning,
        format!(
            "{} file{} holding credentials can be read by others on this machine",
            open.len(),
            crate::plural::s(open.len())
        ),
        "Anyone with their own account on this machine can read the credential in it. That is \
         the one thing owner-only permissions protect against, and it is not protecting it. \
         lemonfiber writes these files owner-only, so something else has widened them — a \
         restore from an archive that carried no permissions, a copy made under a permissive \
         umask, or a recursive change aimed at something else.",
        Remedy::new("Take the permissions back to their owner").with_detail(tighten.join("\n")),
    )
    .with_detail(each.join("; "))
}

#[cfg(test)]
mod tests {
    use super::{guarded, Guarded, PermissionsCheck, BEYOND_THE_OWNER, CREDENTIALS_EXPOSED};
    use crate::config::paths::Paths;
    use crate::doctor::Check;
    use lemonfiber_fixtures::files::Files;
    use std::path::{Path, PathBuf};

    /// Two files to guard, at paths a fake can be scripted against by their endings.
    fn two() -> Vec<Guarded> {
        vec![
            Guarded {
                what: "the settings file".to_owned(),
                at: PathBuf::from("/somewhere/lemonfiber/.env"),
            },
            Guarded {
                what: "the web interface's password".to_owned(),
                at: PathBuf::from("/somewhere/lemonfiber/admission.json"),
            },
        ]
    }

    /// What one run of the check said, as one string to read the claim out of.
    async fn said(modes: Vec<(&'static str, u32)>) -> String {
        let check = PermissionsCheck::new(Files::owning(modes), two());
        let findings = check.run().await;
        assert_eq!(findings.len(), 1, "one finding, whatever it says");
        format!("{:?}", findings.first().map(|one| one.verdict.clone()))
    }

    #[tokio::test]
    async fn a_machine_where_none_of_them_exists_yet_is_skipped_rather_than_passed() {
        let settled = said(Vec::new()).await;

        assert!(settled.contains("Skipped"), "{settled}");
        assert!(settled.contains("on this machine yet"), "{settled}");
    }

    /// The trap this counts against: nothing open is only a pass once something was read.
    #[tokio::test]
    async fn files_read_and_none_of_them_open_is_a_pass_that_says_how_many() {
        let settled = said(vec![(".env", 0o600), ("admission.json", 0o600)]).await;

        assert!(settled.contains("Pass"), "{settled}");
        assert!(
            settled.contains("2 of them, each readable only by you"),
            "{settled}"
        );
    }

    #[tokio::test]
    async fn a_file_anyone_could_read_is_a_warning_naming_it_and_its_mode() {
        let settled = said(vec![(".env", 0o644), ("admission.json", 0o600)]).await;

        assert!(settled.contains("Warn"), "{settled}");
        assert!(settled.contains(CREDENTIALS_EXPOSED.as_str()), "{settled}");
        assert!(settled.contains("0644"), "{settled}");
        assert!(settled.contains("/somewhere/lemonfiber/.env"), "{settled}");
        assert!(settled.contains("1 file "), "{settled}");
    }

    #[tokio::test]
    async fn the_remedy_is_the_command_that_closes_each_of_the_open_ones() {
        let settled = said(vec![(".env", 0o644), ("admission.json", 0o640)]).await;

        assert!(
            settled.contains("chmod go-rwx /somewhere/lemonfiber/.env"),
            "{settled}"
        );
        assert!(
            settled.contains("chmod go-rwx /somewhere/lemonfiber/admission.json"),
            "{settled}"
        );
        assert!(settled.contains("2 files"), "{settled}");
    }

    /// A file that is not there is neither read nor reported, so one missing file
    /// does not turn a genuine finding into a skip.
    #[tokio::test]
    async fn one_file_absent_and_one_open_still_reports_the_open_one() {
        let settled = said(vec![("admission.json", 0o644)]).await;

        assert!(settled.contains("Warn"), "{settled}");
        assert!(settled.contains("1 file "), "{settled}");
        assert!(!settled.contains(".env is"), "{settled}");
    }

    #[test]
    fn an_owner_only_mode_grants_nothing_beyond_the_owner_and_a_wider_one_does() {
        for mode in [0o600_u32, 0o700, 0o400] {
            assert_eq!(mode & BEYOND_THE_OWNER, 0, "{mode:04o}");
        }
        for mode in [0o644_u32, 0o640, 0o604, 0o666, 0o755] {
            assert_ne!(mode & BEYOND_THE_OWNER, 0, "{mode:04o}");
        }
    }

    /// What is guarded is what was declared to hold a credential, and the one
    /// directory the services write is left out of it.
    #[test]
    fn the_guarded_files_are_the_declared_secret_ones_the_services_do_not_write() {
        let paths = Paths::rooted(Path::new("/config"), Path::new("/data"));
        let files = guarded(&paths);
        let what: Vec<&str> = files.iter().map(|one| one.what.as_str()).collect();

        assert!(!files.is_empty());
        assert_eq!(
            files.len(),
            crate::stored::EVERY
                .iter()
                .filter(|entry| entry.secret)
                .count()
                - 1,
            "{what:?}"
        );
        assert!(what.contains(&"the settings file"), "{what:?}");
        // Named rather than inferred from a path: the directory the services write
        // their own keys into is the one deliberate omission, and a count alone would
        // not say which one was left out.
        let excluded = crate::stored::EVERY
            .iter()
            .find(|entry| entry.accessor == "service_config")
            .map(|entry| entry.what);
        assert!(
            excluded.is_some(),
            "the services' own directory is declared"
        );
        assert!(
            !what.iter().any(|one| Some(*one) == excluded),
            "{what:?} still holds {excluded:?}"
        );
    }
}
