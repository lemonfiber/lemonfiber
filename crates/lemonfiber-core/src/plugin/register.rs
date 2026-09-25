//! Every plugin this machine has installed, as one record kept beside the settings.
//!
//! Apart from what one plugin's record holds, in [`super::installed`], because the two
//! answer different questions: that one is what installing a plugin settled, and this
//! is the file the machine reads back to know which plugins it has — where a record
//! that will not read is a refusal rather than an empty answer.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::installed::Installed;

/// Why a register could not be read.
///
/// Two ways, and neither of them is *there is no file*. No file, an empty file and a
/// machine that has never installed anything are one answer — an empty register — and
/// that answer is not a fault. These two are: a file that is there and will not parse,
/// and one that parses and says two different things about the same plugin.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum Unreadable {
    /// The record is there and this build cannot read it.
    #[error("the record of installed plugins could not be read: {0}")]
    Damaged(String),

    /// The record names one plugin twice.
    #[error("the record of installed plugins holds {0} twice, so what is installed under that name cannot be said")]
    Twice(String),
}

/// Why a plugin could not be recorded as installed.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{plugin} is already installed, at version {version}")]
pub struct Already {
    /// Which plugin the record already holds.
    pub plugin: String,
    /// The version it holds for it.
    pub version: String,
}

/// Every plugin this machine has installed.
///
/// Kept in one file beside the settings rather than one file per plugin: what an
/// operator asks is *what is installed*, a directory answers that only by being
/// listed, and a half-written directory has no shape a read can refuse.
///
/// The order is the plugins' own ids, so the file reads the same twice and a diff of
/// it says what changed rather than where something was appended.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Register {
    /// One record per installed plugin.
    #[serde(default)]
    installed: Vec<Installed>,
}

impl Register {
    /// Nothing installed.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            installed: Vec::new(),
        }
    }

    /// What the record holds, or why it cannot be said.
    ///
    /// **A damaged record is refused rather than read as empty**, which is where this
    /// parts company with every other small record beside the settings. Those hold an
    /// answer that can be given again: a forgotten preference is asked for a second
    /// time, and the cost is a question. This one is the only memory that a stranger's
    /// service is on this machine at all — reading it as empty would report a stack
    /// with a plugin in it as a stack with none, and every later reading of what is
    /// installed, what is overridden and what a removal would put back would be
    /// confidently wrong.
    ///
    /// # Errors
    ///
    /// [`Unreadable`] where the text is not a register this build can read, or names
    /// one plugin twice.
    pub fn parse(text: &str) -> Result<Self, Unreadable> {
        if text.trim().is_empty() {
            return Ok(Self::empty());
        }
        let read: Self =
            serde_json::from_str(text).map_err(|why| Unreadable::Damaged(why.to_string()))?;
        read.once_each()?;
        read.as_written()?;
        Ok(read)
    }

    /// Whether every value the record carries is one this build would have written.
    ///
    /// Each of them is written again, into a Compose document, a proxy stanza and a
    /// dashboard entry, every time the plugin's files are derived — and the reader
    /// that held them to the format ran once, on a manifest, long before. So a record
    /// is held on the way in to the same definitions the reader used: a service name
    /// that is one label, a registry path, a digest, a directory written in its own
    /// alphabet, a hostname that is one label, and prose nothing downstream expands.
    /// A record that fails is one this build did not write, and it is refused rather
    /// than acted on.
    fn as_written(&self) -> Result<(), Unreadable> {
        use lemonfiber_plugin::refusing::carried::{
            is_digest, is_directory, is_label, is_plugin_id, is_reference, substituted,
        };
        let refused = |what: String| {
            Err(Unreadable::Damaged(format!(
                "it holds {what} this build would not have written"
            )))
        };
        for one in &self.installed {
            let plugin = one.plugin.as_str();
            if !is_plugin_id(plugin) {
                return refused(format!("a plugin id, {plugin:?},"));
            }
            for placed in &one.services {
                let directory = &placed.config_path;
                let hostname = placed
                    .reached
                    .as_ref()
                    .and_then(|reached| reached.hostname());
                let group = placed.reached.as_ref().and_then(|reached| reached.group());
                let written = is_label(&placed.service)
                    && is_reference(&placed.image)
                    && is_digest(&placed.digest)
                    && directory.starts_with('/')
                    && !directory.contains("..")
                    && is_directory(directory)
                    && hostname.is_none_or(is_label)
                    && [
                        Some(placed.name.as_str()),
                        Some(placed.description.as_str()),
                        group,
                    ]
                    .into_iter()
                    .flatten()
                    .all(|prose| substituted(prose).is_none());
                if !written {
                    return refused(format!(
                        "{plugin}'s service {:?} with a value",
                        placed.service
                    ));
                }
            }
        }
        Ok(())
    }

    /// Whether any plugin appears twice.
    ///
    /// Checked on the way in rather than trusted to the writer. The id is the name a
    /// plugin is installed and journalled under, so two records for one name is two
    /// answers to *what is installed as this* — and a reader taking the first would
    /// silently prefer whichever was written earlier.
    fn once_each(&self) -> Result<(), Unreadable> {
        let mut seen: Vec<&str> = Vec::new();
        for one in &self.installed {
            if seen.contains(&one.plugin.as_str()) {
                return Err(Unreadable::Twice(one.plugin.clone()));
            }
            seen.push(&one.plugin);
        }
        Ok(())
    }

    /// As it is kept: sorted by plugin id, one trailing newline.
    ///
    /// `None` only where it will not serialise, which strings and numbers cannot.
    #[must_use]
    pub fn to_json(&self) -> Option<String> {
        serde_json::to_string_pretty(self)
            .ok()
            .map(|text| text + "\n")
    }

    /// What is installed, in the order the record keeps them.
    #[must_use]
    pub fn installed(&self) -> &[Installed] {
        &self.installed
    }

    /// What is recorded for this plugin, where anything is.
    #[must_use]
    pub fn holds(&self, plugin: &str) -> Option<&Installed> {
        self.installed.iter().find(|one| one.plugin == plugin)
    }

    /// Record an install, keeping the order the file is read back in.
    ///
    /// # Errors
    ///
    /// [`Already`] where this plugin is recorded. An install over an install is an
    /// update, which reverses one set of changes and applies another; treating it as
    /// a write would leave the record describing the new version and the machine
    /// carrying both.
    pub fn record(&mut self, one: Installed) -> Result<(), Already> {
        if let Some(held) = self.holds(&one.plugin) {
            return Err(Already {
                plugin: held.plugin.clone(),
                version: held.version.clone(),
            });
        }
        let at = self
            .installed
            .partition_point(|held| held.plugin < one.plugin);
        self.installed.insert(at, one);
        Ok(())
    }

    /// Take a plugin out of the record.
    ///
    /// Silent about a name it does not hold, because the caller has already refused
    /// that case by name and a second refusal here would be a second answer to a
    /// question already settled. What this promises is the state afterwards: whatever
    /// it held about that plugin, it holds nothing now.
    pub fn forget(&mut self, plugin: &str) {
        self.installed.retain(|one| one.plugin != plugin);
    }
}
