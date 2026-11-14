//! What a recounter keeps beneath the adapter's configuration directory between asks and
//! restarts: where it sends, how it hears, the secret it is pushed at and its cursor,
//! readable by the adapter's user alone and replaced whole.

use std::io::{ErrorKind, Read as _, Write as _};
use std::os::unix::fs::OpenOptionsExt as _;
use std::path::{Path, PathBuf};

use lemonfiber_ports::service::{Heard, Hearing};
use serde::{Deserialize, Serialize};

/// The file in an adapter's configuration directory a recounter keeps what it keeps in.
pub const EVENTS_FILE: &str = "events.json";

/// The file a recounter writes before renaming it over [`EVENTS_FILE`].
const STAGED_FILE: &str = "events.json.new";

/// The most of [`EVENTS_FILE`] read, in bytes.
const KEPT_MOST: u64 = 16 * 1024;

/// Read and written by the adapter's user alone.
const OWNER_ONLY: u32 = 0o600;

/// What a recounter keeps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Kept {
    /// The core's address events are sent to.
    pub address: String,
    /// The SHA-256 of the certificate the core presents there.
    pub pin: String,
    /// The seconds apart it reads the history where it polls.
    pub every: u32,
    /// How it hears the upstream.
    pub hearing: Hearing,
    /// The upstream's own position of the newest happening sent.
    pub cursor: String,
    /// The secret the upstream pushes at, where it pushes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret: Option<String>,
}

impl Kept {
    /// How it hears, as `events` answers it.
    pub(crate) const fn heard(&self) -> Heard {
        match self.hearing {
            Hearing::Push => Heard {
                hearing: Hearing::Push,
                every: None,
            },
            Hearing::Poll => Heard {
                hearing: Hearing::Poll,
                every: Some(self.every),
            },
        }
    }
}

/// What is kept beneath `configuration`, or nothing where nothing is.
///
/// # Errors
///
/// The error reading it, or [`ErrorKind::InvalidData`] where it is larger than
/// [`KEPT_MOST`] or is not what a recounter keeps.
pub(crate) fn read(configuration: &Path) -> std::io::Result<Option<Kept>> {
    let file = match std::fs::File::open(at(configuration)) {
        Ok(file) => file,
        Err(why) if why.kind() == ErrorKind::NotFound => return Ok(None),
        Err(why) => return Err(why),
    };
    let mut written = Vec::new();
    file.take(KEPT_MOST + 1).read_to_end(&mut written)?;
    if written.len() as u64 > KEPT_MOST {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            "it is larger than a recounter keeps",
        ));
    }
    serde_json::from_slice(&written).map(Some).map_err(|_| {
        std::io::Error::new(ErrorKind::InvalidData, "it is not what a recounter keeps")
    })
}

/// `kept` written beside what is kept beneath `configuration` and renamed over it.
///
/// # Errors
///
/// The error writing or renaming it.
pub(crate) fn write(configuration: &Path, kept: &Kept) -> std::io::Result<()> {
    let staged = configuration.join(STAGED_FILE);
    let written = serde_json::to_vec(kept).map_err(std::io::Error::other)?;
    match std::fs::remove_file(&staged) {
        Err(why) if why.kind() != ErrorKind::NotFound => return Err(why),
        _ => {}
    }
    let replaced = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(OWNER_ONLY)
        .open(&staged)
        .and_then(|mut file| file.write_all(&written).and_then(|()| file.sync_all()))
        .and_then(|()| std::fs::rename(&staged, at(configuration)));
    if replaced.is_err() {
        let _ = std::fs::remove_file(&staged);
    }
    replaced?;
    std::fs::File::open(configuration).and_then(|directory| directory.sync_all())
}

/// What is kept beneath `configuration` removed, where anything is.
///
/// # Errors
///
/// The error removing it.
pub(crate) fn dropped(configuration: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(at(configuration)) {
        Err(why) if why.kind() != ErrorKind::NotFound => Err(why),
        _ => Ok(()),
    }
}

/// Where what is kept beneath `configuration` is.
fn at(configuration: &Path) -> PathBuf {
    configuration.join(EVENTS_FILE)
}

#[cfg(test)]
mod tests;
