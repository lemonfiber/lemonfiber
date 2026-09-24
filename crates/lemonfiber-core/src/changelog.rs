//! What each release changed, as the record the release pipeline wrote.
//!
//! An operator deciding whether to move asks what changed, and until now the only
//! place that was answered was a release page on a forge. A machine with no route
//! out could not reach it, and a machine with one had to be told where to look. So
//! the record travels with the binary: it is generated from the commits that made
//! each release, committed as an artefact, and compiled in.
//!
//! **It changes only when a release is tagged**, which is what makes a generated
//! file safe to commit. A pull request adds commits to the trunk and moves nothing
//! here; the next tag does.
//!
//! That also creates the one thing this has to be careful about. A kept file can
//! stop describing what shipped, in two directions. Between a tag and the refresh
//! that follows it the record is *behind* — a release exists whose notes are not
//! written yet — which is ordinary and is said rather than hidden. A record naming
//! a release this build could not have shipped in is the other direction, and it is
//! a contradiction rather than a lag: something is wrong with the file or with the
//! build, nobody can say which, and a changelog that cannot be trusted is worse
//! than none because it teaches an operator not to read it. So it is flagged, and
//! it is never called current.
//!
//! Nothing here reaches the network or the filesystem. The record is a string in
//! the binary, so the whole of what a surface shows is decided in a test.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::migration::version::{among_versions, Standing};

/// Where the generated record is kept, relative to the workspace root.
pub const RECORD_PATH: &str = "reference/changelog.json";

/// The record this build carries.
const CARRIED: &str = include_str!("../../../reference/changelog.json");

/// Whether the record describes what this build could have shipped.
///
/// The three the specification names, and the distinction between the last two is
/// the one worth keeping: being behind the tags is a lag, and contradicting them is
/// a fault.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "ChangelogState")]
pub enum State {
    /// The record holds this build's release, and claims nothing later.
    Current,
    /// This build's release has no notes yet, which is where a tag leaves things.
    Pending,
    /// The record and what this build could have shipped disagree.
    Stale,
}

/// One requirement, and every release that shipped something citing it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, schemars::JsonSchema)]
pub struct Requirement {
    /// The feature it belongs to, in words.
    pub feature: String,
    /// Where it is defined, unless it has since been withdrawn.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Whether it was withdrawn after it shipped.
    #[serde(default)]
    pub withdrawn: bool,
    /// Every version that shipped something citing it, newest first.
    pub shipped_in: Vec<String>,
}

/// One change, as a reader meets it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, schemars::JsonSchema)]
pub struct Entry {
    /// What changed, in the words it was written in.
    pub summary: String,
    /// The requirements it served, which are the link rather than the headline.
    pub requirements: Vec<String>,
    /// Where it was reviewed, where it was reviewed anywhere.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
}

/// The entries of one kind, under the name an operator reads them by.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, schemars::JsonSchema)]
pub struct Group {
    /// What this group of changes is: new, fixed, faster, or maintenance.
    pub title: String,
    /// The changes, in the order they were made.
    pub entries: Vec<Entry>,
}

/// One release, and everything the record holds about it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, schemars::JsonSchema)]
pub struct Release {
    /// The version, without the tag's leading letter.
    pub version: String,
    /// The tag it was cut from.
    pub tag: String,
    /// The day it was published, where the record of it says.
    #[serde(default)]
    pub released_on: Option<String>,
    /// What it set out to deliver, in the words the version was staged under.
    #[serde(default)]
    pub delivers: Option<String>,
    /// The version this one patched, where it is a patch.
    #[serde(default)]
    pub patches: Option<String>,
    /// The version whose goals this tag carried, where that is not its own.
    #[serde(default)]
    pub carried: Option<String>,
    /// Why it was withdrawn, where it was.
    #[serde(default)]
    pub withdrawn: Option<String>,
    /// Whether anything in it is a change an operator would notice.
    pub user_facing: bool,
    /// The changes, gathered by what kind of change each is.
    pub groups: Vec<Group>,
}

/// One release as a listing shows it: everything but what it changed.
///
/// Kept apart from [`Release`] rather than being it with the entries left out,
/// because the two are read for different things. A listing answers which releases
/// there have been and which of them was taken back; only the one being read needs
/// to carry every line of what it changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[schemars(rename = "ReleaseSummary")]
pub struct Summary {
    /// The version.
    pub version: String,
    /// The day it was published, where the record of it says.
    pub released_on: Option<String>,
    /// What it set out to deliver.
    pub delivers: Option<String>,
    /// The version this one patched, where it is a patch.
    pub patches: Option<String>,
    /// Why it was withdrawn, where it was.
    pub withdrawn: Option<String>,
    /// Whether anything in it is a change an operator would notice.
    pub user_facing: bool,
}

impl From<&Release> for Summary {
    fn from(release: &Release) -> Self {
        Self {
            version: release.version.clone(),
            released_on: release.released_on.clone(),
            delivers: release.delivers.clone(),
            patches: release.patches.clone(),
            withdrawn: release.withdrawn.clone(),
            user_facing: release.user_facing,
        }
    }
}

/// Every release, and every requirement any of them shipped something for.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Record {
    /// The releases, newest first.
    pub releases: Vec<Release>,
    /// What each cited requirement is, and which releases carried it.
    pub requirements: BTreeMap<String, Requirement>,
}

impl Record {
    /// The record this build carries, or nothing where it cannot be read.
    ///
    /// Nothing rather than a typed failure, and deliberately: the file is generated
    /// and compiled in, so an unreadable one is a defect in this build rather than
    /// something an operator did or can act on. What the surfaces do with it is say
    /// so — the state below becomes [`State::Stale`], which is the one answer that
    /// is safe when what shipped cannot be established.
    #[must_use]
    pub fn carried() -> Option<Self> {
        Self::read(CARRIED)
    }

    /// The record a document describes, or nothing where it describes none.
    #[must_use]
    pub fn read(text: &str) -> Option<Self> {
        serde_json::from_str(text).ok()
    }

    /// The release of one version, where the record holds it.
    #[must_use]
    pub fn release(&self, version: &str) -> Option<&Release> {
        self.releases.iter().find(|one| one.version == version)
    }
}

/// What a surface shows about the record, given the version asking.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Notes {
    /// Whether the record describes what this build could have shipped.
    pub state: State,
    /// What the running version changed, where the record holds its release.
    pub running: Option<Release>,
    /// Every release the record holds, newest first.
    pub releases: Vec<Summary>,
    /// What each requirement the running release cites is, and where it is defined.
    pub requirements: BTreeMap<String, Requirement>,
}

impl Notes {
    /// What a surface is told where there is no record to read at all.
    ///
    /// Stale rather than empty and current, because "nothing could be read" and
    /// "nothing has been released" are different answers and only one of them is
    /// safe to show as up to date.
    #[must_use]
    pub fn unread() -> Self {
        Self {
            state: State::Stale,
            running: None,
            releases: Vec::new(),
            requirements: BTreeMap::new(),
        }
    }
}

/// What the record says to a build of this version.
#[must_use]
pub fn notes(running: &str) -> Notes {
    told(Record::carried().as_ref(), running)
}

/// The same, from a record already in hand.
#[must_use]
pub fn told(record: Option<&Record>, running: &str) -> Notes {
    let Some(record) = record else {
        return Notes::unread();
    };
    let release = record.release(running);
    Notes {
        state: state(record, running),
        running: release.cloned(),
        releases: record.releases.iter().map(Summary::from).collect(),
        requirements: cited(record, release),
    }
}

/// Where the record stands against the version reading it.
///
/// A release later than the one asking cannot have shipped with it, and a version
/// pair that cannot be ordered leaves the question unanswerable — both are refused
/// the word "current", because the one thing this must never do is present a record
/// that contradicts what went out as though it agreed.
fn state(record: &Record, running: &str) -> State {
    for release in &record.releases {
        match among_versions(&release.version, running) {
            Standing::Later | Standing::Untellable => return State::Stale,
            Standing::Earlier | Standing::Same => {}
        }
    }
    if record.release(running).is_some() {
        State::Current
    } else {
        State::Pending
    }
}

/// The requirements one release's entries name, and nothing else.
///
/// The whole index is every requirement this project has ever shipped something
/// for. What a reader of one release needs is the handful its own entries cite, and
/// carrying the rest would put a quarter of a megabyte on every read of it.
fn cited(record: &Record, release: Option<&Release>) -> BTreeMap<String, Requirement> {
    let mut held = BTreeMap::new();
    let Some(release) = release else {
        return held;
    };
    for group in &release.groups {
        for entry in &group.entries {
            for identifier in &entry.requirements {
                if let Some(requirement) = record.requirements.get(identifier) {
                    held.insert(identifier.clone(), requirement.clone());
                }
            }
        }
    }
    held
}

#[cfg(test)]
mod tests;
