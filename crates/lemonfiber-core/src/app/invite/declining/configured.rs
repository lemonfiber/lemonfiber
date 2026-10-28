//! The stack's own decline service, told through the files it reads in its configuration
//! directory: the key it switches invitations off with, the certificates the media server
//! may present to it, and the table it acts on.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use lemonfiber_sidecar::decline::{
    Claimed as Telling, File, Invitation, Key, Pins, Table, TokenHash,
};

use super::{path, SERVICE};
use crate::ports::filesystem::FileSystem;
use crate::ports::service::{Claimed, Declining, Failure, Invitations, KeyHeld};

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

    /// Write `text` to `file`, as the service reads it.
    fn written(&self, file: File, text: &str) -> Result<(), Failure> {
        crate::config::store::write(&path(&self.project, file), text).map_err(|failure| {
            Failure::Refused {
                service: SERVICE.to_owned(),
                detail: failure.to_string(),
            }
        })
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
