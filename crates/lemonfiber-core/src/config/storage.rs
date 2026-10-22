//! Where the stack's data is kept: a path on this machine, or an NFS export.
//!
//! One setting, `DATA_ROOT`, says which. A path is bound into every service that takes
//! data; `nfs://<server>/<export>` becomes one NFS volume the project shares. The server
//! and the export are written into the mount options, so anything that would change those
//! options is refused here, naming it.

use std::net::{IpAddr, Ipv6Addr};
use std::path::PathBuf;

use crate::error::codes::storage::ROOT_REFUSED;
use crate::error::{Problem, Remedy, State};

/// How an NFS export is written as the data root.
pub const NFS: &str = "nfs://";

/// Where the stack's data is kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Storage {
    /// A path on this machine, bound at `/data`.
    Bind(PathBuf),
    /// An NFS export, mounted at `/data` as one volume.
    Nfs(Export),
}

/// An NFS export, as the mount options name it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Export {
    /// The host name or address serving it.
    pub server: String,
    /// The absolute path it is exported at.
    pub path: String,
}

/// Why a data root written as an NFS export will not do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    /// The server is not a host name or an address.
    Server(String),
    /// The export is not an absolute path, or holds a character that would change the
    /// mount options written.
    Export(String),
}

impl Refused {
    /// What a data root written `written` is refused with wherever it is read: no project
    /// is written from it, and the doctor names it.
    #[must_use]
    pub fn problem(&self, written: &str) -> Problem {
        Problem::new(
            ROOT_REFUSED,
            format!("DATA_ROOT {written} cannot be the data root"),
            "Written as it stands, it would mount something other than what it says, so no \
             project is written from it.",
            Remedy::new(
                "Set it again with lemonfiber config set DATA_ROOT <path or nfs://server/export>",
            ),
        )
        .in_state(State::Guided)
        .with_detail(self.said())
    }

    /// The sentence naming what is refused.
    #[must_use]
    pub fn said(&self) -> String {
        match self {
            Self::Server(server) => format!(
                "the NFS server \"{server}\" is not a host name or an address, so it cannot be \
                 written into the mount options"
            ),
            Self::Export(path) => format!(
                "the NFS export \"{path}\" is not an absolute path free of commas, quotes, \
                 whitespace and control characters, so it cannot be written into the mount \
                 options"
            ),
        }
    }
}

impl Storage {
    /// What `written` names, or nothing where it is blank.
    ///
    /// # Errors
    ///
    /// Where it is written as an NFS export whose server or export will not do.
    pub fn read(written: &str) -> Result<Option<Self>, Refused> {
        let written = written.trim();
        if written.is_empty() {
            return Ok(None);
        }
        match written.strip_prefix(NFS) {
            Some(rest) => Export::read(rest).map(|export| Some(Self::Nfs(export))),
            None => Ok(Some(Self::Bind(PathBuf::from(written)))),
        }
    }

    /// The path on this machine, where the data is kept on one.
    #[must_use]
    pub fn on_this_machine(&self) -> Option<&PathBuf> {
        match self {
            Self::Bind(path) => Some(path),
            Self::Nfs(_) => None,
        }
    }

    /// The export, where the data is kept on one.
    #[must_use]
    pub const fn export(&self) -> Option<&Export> {
        match self {
            Self::Bind(_) => None,
            Self::Nfs(export) => Some(export),
        }
    }
}

impl Export {
    /// The export `rest`, the part of an `nfs://` value after the scheme, names.
    fn read(rest: &str) -> Result<Self, Refused> {
        let (server, path) = match rest.strip_prefix('[') {
            Some(bracketed) => match bracketed.split_once(']') {
                Some((server, path)) if server.parse::<Ipv6Addr>().is_ok() => (server, path),
                Some((server, _)) => return Err(Refused::Server(server.to_owned())),
                None => return Err(Refused::Server(rest.to_owned())),
            },
            None => rest
                .find('/')
                .map_or((rest, ""), |at| (&rest[..at], &rest[at..])),
        };
        if !served_by(server) {
            return Err(Refused::Server(server.to_owned()));
        }
        if !exported_at(path) {
            return Err(Refused::Export(path.to_owned()));
        }
        Ok(Self {
            server: server.to_owned(),
            path: path.to_owned(),
        })
    }

    /// The value `DATA_ROOT` holds for this export.
    #[must_use]
    pub fn written(&self) -> String {
        let server = if self.server.contains(':') {
            format!("[{}]", self.server)
        } else {
            self.server.clone()
        };
        format!("{NFS}{server}{}", self.path)
    }
}

/// Whether `server` is an address or a host name.
fn served_by(server: &str) -> bool {
    server.parse::<IpAddr>().is_ok() || named(server)
}

/// Whether `server` is a host name: dot-separated labels of letters, digits and hyphens,
/// none starting or ending with a hyphen.
fn named(server: &str) -> bool {
    !server.is_empty()
        && server.len() <= 253
        && server.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .chars()
                    .all(|one| one.is_ascii_alphanumeric() || one == '-')
        })
}

/// Whether `path` is an absolute path nothing in the mount options would read as more
/// than a path.
fn exported_at(path: &str) -> bool {
    path.starts_with('/')
        && !path.chars().any(|one| {
            one == ','
                || one == '"'
                || one == '\''
                || one == '\\'
                || one.is_whitespace()
                || one.is_control()
        })
}

#[cfg(test)]
mod tests;
