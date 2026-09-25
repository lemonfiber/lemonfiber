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
use crate::error::codes::config::CREDENTIALS_EXPOSED;
use crate::error::{Problem, Remedy, Severity};
use crate::ports::filesystem::FileSystem;

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
mod tests;
