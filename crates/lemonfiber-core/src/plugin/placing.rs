//! Where an install puts what it writes, and what each of those writes is.
//!
//! The install record settles *what* was decided; this settles *where that lands on
//! the machine*. Kept apart from both the record and the container for the reason
//! the container is kept apart from the record: a copy of a derivation is free to
//! disagree with the derivation, so there is one place that turns an installed
//! plugin into a list of paths and one place that turns it into a container.
//!
//! **Almost every write is a path lemonfiber creates, and that is what makes the
//! reversal ordinary.** A file the install makes is journalled as
//! [`crate::journal::Kind::Made`], which the rollback layer already classifies as
//! reversible in full and already undoes by removing exactly the path that was made.
//!
//! **The project is the one file every plugin shares.** Its entries are written into the
//! project lemonfiber generates for the whole stack, so an install writes the project
//! over and journals what it held before ([`crate::journal::Kind::Rewritten`]), or makes
//! it where there was none.
//!
//! **The exception is the stack's own proxy.** It reads one file, and a plugin's service
//! is reachable through it only if it is written *into* that file. So that write is a
//! region, marked out inside the file and owned by the plugin ([`crate::region`]), and
//! journalled as [`crate::journal::Kind::Region`]: the reversal takes out exactly the
//! region, and refuses where somebody has edited it. What goes in it is
//! [`super::fronting`]'s to say.
//!
//! Nothing here touches a disk. It is given a record and a stack directory and
//! answers with a list, so every arrangement can be put in front of a test without
//! one.

use std::path::{Path, PathBuf};

use super::container::Reading;
use super::installed::Installed;
use crate::project::entry::{CONFIGURATION_TREE, MOUNT_TREE};

/// One thing an install puts on the machine.
///
/// A path and what lands at it. The two are one type rather than two because the
/// caller does the same thing with every one — journals it, then makes it — and a
/// caller holding two lists is a caller free to journal one and write the other.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Write {
    /// Where it goes.
    pub path: PathBuf,
    /// What lands there.
    pub lands: Lands,
}

/// What one write puts at its path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Lands {
    /// A directory brought into being beneath the stack directory, reached through no
    /// link on the way.
    Directory {
        /// The stack directory it lies beneath.
        beneath: PathBuf,
    },
    /// The generated project, written whole with the plugin's entries in it.
    Document {
        /// What the project held before, where there was one.
        previous: Option<String>,
        /// What it is to hold.
        text: String,
    },
    /// The key lemonfiber asks the plugin's adapter with, minted as it is written.
    Key,
    /// A service's own secrets file, each variable's value read as it is written.
    Secrets(Vec<super::container::Variable>),
    /// A file the plugin's source ships for one of its services, as shipped, each secret
    /// it names written in where it is `seeded`.
    Starting {
        /// What the source ships.
        bytes: Vec<u8>,
        /// Whether it names a secret, and so is written with its value.
        seeded: bool,
    },
    /// The credentials an adapter reads its upstream's with: each field, to the id of the
    /// plugin's secret it holds.
    Upstream(std::collections::BTreeMap<String, String>),
    /// A region inside a file the stack already has, holding this.
    Region {
        /// The file beneath the stack directory, as the record of what lemonfiber
        /// materialised names it.
        key: String,
        /// Whose region it is, as its markers name it.
        owner: String,
        /// What it holds.
        body: String,
    },
}

impl Write {
    /// A directory the install makes beneath the stack directory `stack`.
    fn directory(path: PathBuf, stack: &Path) -> Self {
        Self {
            path,
            lands: Lands::Directory {
                beneath: stack.to_path_buf(),
            },
        }
    }

    /// The generated project at `path`, holding `previous` now and `text` once written.
    #[must_use]
    pub const fn project(path: PathBuf, previous: Option<String>, text: String) -> Self {
        Self {
            path,
            lands: Lands::Document { previous, text },
        }
    }

    /// The key file an adapter service is asked under.
    const fn key(path: PathBuf) -> Self {
        Self {
            path,
            lands: Lands::Key,
        }
    }

    /// A region the install writes into one of the stack's own files.
    fn region(stack: &Path, key: &str, owner: String, body: String) -> Self {
        Self {
            path: stack.join(key),
            lands: Lands::Region {
                key: key.to_owned(),
                owner,
                body,
            },
        }
    }

    /// Whether this write is a directory rather than a file.
    #[must_use]
    pub const fn is_directory(&self) -> bool {
        matches!(self.lands, Lands::Directory { .. })
    }
}

/// Where the key lemonfiber asks a plugin's adapter service with is kept: in that
/// service's own configuration directory, where its container reads it.
#[must_use]
pub fn key_file(stack: &Path, service: &str) -> PathBuf {
    configuration(stack, service).join(lemonfiber_contract::adapter::KEY_FILE)
}

/// Where one of a plugin's services keeps the sources of its further mounts.
#[must_use]
pub fn mount_tree(stack: &Path, service: &str) -> PathBuf {
    stack.join(MOUNT_TREE).join(service)
}

/// Every further mount source of an installed plugin's services the model allows, each
/// inside its service's mount tree.
#[must_use]
pub fn mount_sources(installed: &Installed, stack: &Path) -> Vec<PathBuf> {
    installed
        .services
        .iter()
        .flat_map(|placed| {
            let tree = mount_tree(stack, &placed.service);
            super::container::mounts(placed).map(move |mount| tree.join(&mount.from))
        })
        .collect()
}

/// Where one of a plugin's services keeps its own configuration.
///
/// Not published, unlike the document's path: nothing outside this module has had to
/// ask yet, and a name nothing reads is one that drifts from what it describes.
pub(crate) fn configuration(stack: &Path, service: &str) -> PathBuf {
    stack.join(CONFIGURATION_TREE).join(service)
}

/// Everything installing this plugin writes, in the order it is written.
///
/// Ordered, and the order is load-bearing twice over. The configuration directories
/// come first because the project mounts them, so a stop between the two leaves
/// directories nothing references rather than an entry mounting a directory that is not
/// there. And within the directories a parent comes before its child, so a reversal
/// walking the record backwards removes the child first.
///
/// `project`, where the project changes, comes after them for the same reason the applied
/// marker is last in an apply: it is the write that makes the rest take effect, so a run
/// that stopped short of it has changed nothing Compose will read. The proxy's region
/// follows it, because a route to a service that is not declared is a route to nothing,
/// and is planned only where it would hold something, so a plugin whose services the
/// household does not reach writes none.
#[must_use]
pub fn writes(
    installed: &Installed,
    stack: &Path,
    reading: &Reading<'_>,
    starting: Vec<Write>,
    project: Option<Write>,
) -> Vec<Write> {
    let mut planned: Vec<Write> = installed
        .services
        .iter()
        .map(|placed| Write::directory(configuration(stack, &placed.service), stack))
        .collect();
    for placed in &installed.services {
        let mut writable = super::container::mounts(placed).peekable();
        if writable.peek().is_none() {
            continue;
        }
        let tree = mount_tree(stack, &placed.service);
        planned.push(Write::directory(tree.clone(), stack));
        planned.extend(
            writable
                .filter(|mount| !mount.ro)
                .map(|mount| Write::directory(tree.join(&mount.from), stack)),
        );
    }
    planned.extend(
        installed
            .services
            .iter()
            .filter(|placed| !placed.speaks.is_empty())
            .map(|placed| Write::key(key_file(stack, &placed.service))),
    );
    planned.extend(starting);
    planned.extend(
        installed
            .services
            .iter()
            .filter(|placed| !placed.upstream.is_empty())
            .map(|placed| Write {
                path: configuration(stack, &placed.service)
                    .join(lemonfiber_contract::adapter::UPSTREAM_FILE),
                lands: Lands::Upstream(placed.upstream.clone()),
            }),
    );
    planned.extend(installed.services.iter().filter_map(|placed| {
        super::container::secrets_file(placed, reading).map(|path| Write {
            path,
            lands: Lands::Secrets(super::container::variables(installed, placed, reading)),
        })
    }));
    planned.extend(project);
    let body = super::fronting::proxied(installed);
    if !body.is_empty() {
        planned.push(Write::region(
            stack,
            super::fronting::PROXY,
            super::owning::owner(&installed.plugin),
            body,
        ));
    }
    planned
}

#[cfg(test)]
mod tests;
