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
//! **One file per plugin rather than one file for all of them.** A single shared
//! document would be rewritten by every install and every removal, which makes each
//! of those a change to a file that already existed — and the honest journal entry
//! for that is not `Made`. Per plugin, an install creates exactly one document and a
//! removal removes exactly the one it created, so what the record says and what the
//! reversal does are the same sentence.
//!
//! **The exception is the stack's own wiring.** The proxy reads one file and the
//! dashboard another, and a plugin's service is reachable through the first and
//! listed on the second only if it is written *into* them. So those two writes are a
//! region each, marked out inside the file and owned by the plugin
//! ([`crate::region`]), and journalled as [`crate::journal::Kind::Region`]: the
//! reversal takes out exactly the region, and refuses where somebody has edited it.
//! What goes in them is [`super::fronting`]'s to say.
//!
//! Nothing here touches a disk. It is given a record and a stack directory and
//! answers with a list, so every arrangement can be put in front of a test without
//! one.

use std::path::{Path, PathBuf};

use super::installed::Installed;

/// The directory beneath the stack where a plugin's Compose document is written.
///
/// Inside the stack directory because Compose resolves `extends.file` against the
/// document that declares it, and the entry lemonfiber writes extends the stack's
/// own template by a relative path. A document kept anywhere else would name a
/// template that is not there.
const OVERLAYS: &str = "compose/plugins";

/// The directory beneath the stack holding each service's own configuration.
///
/// The same one the bundled services use, and the same one the generated container
/// mounts from — `./config/<service>` in the entry is this directory, resolved
/// against the project directory Compose is pointed at.
const CONFIGURATION: &str = "config";

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
    /// A directory brought into being.
    Directory,
    /// A file written whole, holding this.
    Document(String),
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
    /// A directory the install makes.
    const fn directory(path: PathBuf) -> Self {
        Self {
            path,
            lands: Lands::Directory,
        }
    }

    /// A file the install writes, and what it holds.
    const fn file(path: PathBuf, content: String) -> Self {
        Self {
            path,
            lands: Lands::Document(content),
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
        matches!(self.lands, Lands::Directory)
    }
}

/// Where a plugin's Compose document is written, beneath the stack directory.
///
/// Public because the settings have to layer it on every invocation, and a caller
/// that rebuilt the path itself would be a second answer to where it is.
#[must_use]
pub fn overlay(stack: &Path, plugin: &str) -> PathBuf {
    stack.join(OVERLAYS).join(format!("{plugin}.yml"))
}

/// Where one of a plugin's services keeps its own configuration.
///
/// Not published, unlike the document's path: nothing outside this module has had to
/// ask yet, and a name nothing reads is one that drifts from what it describes.
fn configuration(stack: &Path, service: &str) -> PathBuf {
    stack.join(CONFIGURATION).join(service)
}

/// Every Compose document the installed plugins contribute, in the order Compose
/// reads them.
///
/// Read off the register rather than by listing the directory the documents sit in.
/// A listing would run whatever it found there, which turns a file somebody dropped
/// in by hand into a service in the stack — and the register is the only thing that
/// says what this machine agreed to install.
///
/// A plugin whose document has gone missing is named anyway rather than skipped.
/// Compose refuses a file it cannot read, which is the honest outcome: the register
/// says the plugin is installed, and a run that quietly started the stack without it
/// would be answering for a stack the operator does not have.
///
/// Named by id and joined here rather than carried as resolved paths, because where
/// a document lives depends on which directory this invocation calls the project
/// root — and a path settled anywhere else would be right for the embedded stack and
/// quietly wrong for one the operator named.
///
/// In the register's own order, which is the plugins' ids — so the invocation is the
/// same twice and a cached Compose project does not churn for no reason.
#[must_use]
pub fn documents(installed: &[String], stack: &Path) -> Vec<PathBuf> {
    installed
        .iter()
        .map(|plugin| overlay(stack, plugin))
        .collect()
}

/// Everything installing this plugin writes, in the order it is written.
///
/// Ordered, and the order is load-bearing twice over. The configuration directories
/// come first because the Compose document mounts them, so a stop between the two
/// leaves directories nothing references rather than an entry mounting a directory
/// that is not there. And within the directories a parent comes before its child, so
/// a reversal walking the record backwards removes the child first.
///
/// The document comes after them for the same reason the applied marker is last in an
/// apply: it is the write that makes the rest take effect, so a run that stopped short
/// of it has changed nothing Compose will read. The proxy's region and the dashboard's
/// follow it, because a route to a service that is not declared is a route to nothing.
/// Each is planned only where it would hold something, so a plugin whose services
/// nothing reaches writes into neither.
#[must_use]
pub fn writes(installed: &Installed, stack: &Path) -> Vec<Write> {
    let mut planned: Vec<Write> = installed
        .services
        .iter()
        .map(|placed| Write::directory(configuration(stack, &placed.service)))
        .collect();
    planned.push(Write::file(
        overlay(stack, &installed.plugin),
        super::container::written(installed),
    ));
    let owner = super::fronting::owner(&installed.plugin);
    for (key, body) in [
        (super::fronting::PROXY, super::fronting::proxied(installed)),
        (
            super::fronting::DASHBOARD,
            super::fronting::listed(installed),
        ),
    ] {
        if !body.is_empty() {
            planned.push(Write::region(stack, key, owner.clone(), body));
        }
    }
    planned
}

#[cfg(test)]
mod tests;
