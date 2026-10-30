//! The stack's own decline service, told through the files it reads in its configuration
//! directory: the key it switches invitations off with, the certificates the media server
//! may present to it, and the table it acts on.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use lemonfiber_sidecar::decline::{
    Claimed as Telling, File, Invitation, Key, Lapses, Left, Outcome, Pins, Refusals, Table,
    TokenHash,
};

use super::{path, SERVICE};
use crate::ports::filesystem::{Beneath, FileSystem};
use crate::ports::service::{
    Claimed, Declined, Declining, Failure, Invitations, KeyHeld, Lapsed, LeftBecause, TakenBack,
};

/// The stack's own decline service, beneath the project its configuration is mounted from.
pub(crate) struct Configured {
    /// The project root the stack's configuration directories sit under.
    project: PathBuf,
    /// What the key is read back through, beneath the directory the service owns.
    files: Arc<dyn FileSystem>,
}

impl Configured {
    /// The decline service under `project`, its key read back through `files`.
    pub(crate) fn at(project: &Path, files: Arc<dyn FileSystem>) -> Self {
        Self {
            project: project.to_path_buf(),
            files,
        }
    }

    /// What `file` holds, read back beneath the directory the service owns, or nothing
    /// where it holds nothing that can be read.
    async fn read(&self, file: File) -> Option<String> {
        let at = path(&self.project, file);
        crate::app::targets::read_owned(self.files.as_ref(), &at, crate::within::directory_of(&at))
            .await
    }

    /// What one of the records the service keeps holds: nothing where it is not there, and
    /// a refusal where something other than a plain file of the service's own directory
    /// is, which is never followed.
    async fn record(&self, file: File) -> Result<Option<String>, Failure> {
        let at = path(&self.project, file);
        match self
            .files
            .read_beneath(&at, crate::within::directory_of(&at))
            .await
        {
            Beneath::Read(text) => Ok(Some(text)),
            Beneath::Absent => Ok(None),
            Beneath::Escaped => Err(Failure::Refused {
                service: SERVICE.to_owned(),
                detail: format!("its record {} is not a file of its own", file.name()),
            }),
        }
    }

    /// Write `text` to `file`, as the service reads it.
    fn written(&self, file: File, text: &str) -> Result<(), Failure> {
        crate::config::store::write(&path(&self.project, file), text).map_err(|failure| {
            Failure::Refused {
                service: SERVICE.to_owned(),
                detail: failure.to_string(),
            }
        })
    }

    /// What [`Declining::refusals`] answers, read from the service's refusal record.
    async fn declined(&self) -> Result<Vec<Declined>, Failure> {
        let refusals = match self.record(File::Refusals).await? {
            Some(text) => Refusals::read(&text).map_err(|why| unread(&why))?,
            None => Refusals::default(),
        };
        Ok(refusals
            .refusals
            .into_iter()
            .map(|one| Declined {
                token: one.token.as_str().to_owned(),
                account: one.account,
                at: one.at,
            })
            .collect())
    }

    /// What [`Declining::lapses`] answers, read from the service's lapse record.
    async fn lapsed(&self) -> Result<Vec<Lapsed>, Failure> {
        let lapses = match self.record(File::Lapses).await? {
            Some(text) => Lapses::read(&text).map_err(|why| unread(&why))?,
            None => Lapses::default(),
        };
        Ok(lapses
            .lapses
            .into_iter()
            .map(|one| Lapsed {
                token: one.token.as_str().to_owned(),
                account: one.account,
                name: one.name,
                issued: one.issued,
                at: one.at,
                outcome: taken_back(one.outcome),
            })
            .collect())
    }
}

#[async_trait]
impl Declining for Configured {
    async fn hold(&self, key: &str, pins: &[String]) -> Result<(), Failure> {
        let refused = |unreadable: lemonfiber_sidecar::Unreadable| Failure::Refused {
            service: SERVICE.to_owned(),
            detail: unreadable.to_string(),
        };
        let key = Key::read(key).map_err(refused)?;
        let pins = Pins::of(pins).map_err(refused)?;
        self.written(File::Pins, &pins.written())?;
        self.written(File::Key, &key.written())
    }

    async fn held(&self) -> Result<Option<KeyHeld>, Failure> {
        let pins = self
            .read(File::Pins)
            .await
            .and_then(|text| Pins::read(&text).ok())
            .unwrap_or_default();
        Ok(self
            .read(File::Key)
            .await
            .and_then(|text| Key::read(&text).ok())
            .map(|key| KeyHeld {
                key: key.fingerprint(),
                pins: pins.listed().to_vec(),
            }))
    }

    async fn apply_invitations(&self, invitations: &Invitations) -> Result<(), Failure> {
        self.written(File::Table, &table(invitations).written())
    }

    async fn refusals(&self) -> Result<Vec<Declined>, Failure> {
        self.declined().await
    }

    async fn lapses(&self) -> Result<Vec<Lapsed>, Failure> {
        self.lapsed().await
    }

    async fn page(&self) -> Result<String, Failure> {
        Ok(PAGE.to_owned())
    }
}

/// Where the stack's own decline service serves the page an invitation is declined on.
const PAGE: &str = "/decline/{token}";

/// The failure of a record the service keeps that could not be read.
fn unread(unreadable: &lemonfiber_sidecar::Unreadable) -> Failure {
    Failure::Refused {
        service: SERVICE.to_owned(),
        detail: format!("its record could not be read: {unreadable}"),
    }
}

/// What the service did with a lapsed invitation's account, as its contract says it.
const fn taken_back(outcome: Outcome) -> TakenBack {
    match outcome {
        Outcome::Removed => TakenBack::Removed,
        Outcome::SwitchedOff => TakenBack::SwitchedOff,
        Outcome::Left(left) => TakenBack::Left {
            why: match left {
                Left::Claimed => LeftBecause::Claimed,
                Left::Administrator => LeftBecause::Administrator,
                Left::Disabled => LeftBecause::Disabled,
                Left::Gone => LeftBecause::Gone,
                Left::Reoffered => LeftBecause::Reoffered,
                Left::Unmatched => LeftBecause::Unmatched,
            },
        },
    }
}

/// `invitations` as the table the service reads.
fn table(invitations: &Invitations) -> Table {
    Table::of(
        match invitations.claimed {
            Claimed::HasPassword => Telling::HasPassword,
            Claimed::OwnWrites => Telling::OwnWrites,
            Claimed::Unknown => Telling::Unknown,
        },
        invitations
            .invitations
            .iter()
            .map(|one| Invitation {
                token: TokenHash::written(&one.token),
                account: one.account.clone(),
                name: one.name.clone(),
                issued: one.issued,
                lapses: one.lapses,
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests;
