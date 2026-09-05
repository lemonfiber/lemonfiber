//! The arguments an action was given.
//!
//! One carrier for every action rather than one shape per action, so a caller sends
//! the field an action names and nothing else. A name the carrier does not hold is
//! refused rather than ignored: a caller who spelled `service` where `services` was
//! meant has been told, instead of watching a whole form stop.
//!
//! **Which action may be given which is next door**, in [`takers`](super::takers),
//! and the split is where this file kept growing: the carrier gains a field when a
//! request gains an argument, and the table gains a list every time — so the two
//! were crossing the line cap together while answering different questions. This one
//! says what a caller may write; that one says who may write it.

mod takers;

pub use takers::{
    unwanted, TAKES_AGREED, TAKES_AGREEMENT, TAKES_ALLOWANCE, TAKES_ARCHIVE, TAKES_BUNDLING,
    TAKES_CHECK, TAKES_CONSENT, TAKES_DISRUPTION, TAKES_DOWNLOAD, TAKES_FORMS, TAKES_ITEM,
    TAKES_NAME, TAKES_NARROWING, TAKES_POLICY, TAKES_PRESET, TAKES_REASON, TAKES_REQUEST,
    TAKES_SERVICE, TAKES_SERVICES, TAKES_SETTING, TAKES_TERM, TAKES_WAITING,
};

use lemonfiber_core::app::Waiting;
use lemonfiber_core::bundle::Filenames;
use serde::Deserialize;

/// The arguments an action was given, mirroring the flags its command takes.
#[derive(Debug, Default, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Arguments {
    /// The forms to act on.
    pub forms: Vec<String>,
    /// The services to act on, leaving the rest of the form alone.
    pub services: Vec<String>,
    /// Whether anything still downloading is let finish before the stop.
    pub wait: Waiting,
    /// The one service to act on instead of the whole stack.
    pub service: Option<String>,
    /// Who an invitation is for, as they will sign in.
    pub name: Option<String>,
    /// The libraries an invitation lets them open, by the names the media server
    /// gives those libraries. Empty is every one.
    pub libraries: Vec<String>,
    /// The age above which an invitation has the media server hold things back.
    pub age_limit: Option<u32>,
    /// What is to happen to content the media server has no rating for.
    ///
    /// A word rather than a switch, because the choice has a cost either way and a
    /// switch has a default nobody can see: `block` holds unrated content back, and
    /// `allow` lets it through. Carried as it was written and read next door, so a
    /// word this build does not know is refused by name rather than falling to
    /// whichever answer the shape happened to default to.
    pub unrated: Option<String>,
    /// The setting to change.
    pub key: Option<String>,
    /// What to change it to.
    pub value: Option<String>,
    /// The quality preset, or the music format, as it is written.
    pub preset: Option<String>,
    /// The media type a quality choice applies to.
    pub media_type: Option<String>,
    /// The backup to restore from, by the name it was written under.
    pub archive: Option<String>,
    /// Whether re-pointing to this machine's data root was accepted.
    pub repoint: bool,
    /// Whether to produce the bundle, rather than say what one would hold.
    pub write: bool,
    /// How many log lines to take from each service.
    pub logs: Option<u32>,
    /// Whether media filenames are shown rather than replaced.
    pub filenames: Filenames,
    /// The settings to show as they are, named as the bundle names them.
    pub reveal: Vec<String>,
    /// The group of checks, or the one check, a diagnosis is narrowed to.
    pub only: Option<String>,
    /// The check a warning is being answered for.
    pub check: Option<String>,
    /// Whether the checks that disturb the running system are included.
    pub disruptive: Disturbing,
    /// What was read before answering — the offer a repair's yes was read in, the
    /// listing a restore's was — as it named itself.
    pub offer: Option<String>,
    /// The checks whose repairs were agreed to, as that offer names them.
    pub agreed: Vec<String>,
    /// Whether a cost the action would incur was agreed to in advance.
    pub confirm: bool,
    /// The one thing to add end to end, as it would be said.
    pub item: Option<String>,
    /// What to follow, named as a person would say it.
    pub term: Option<String>,
    /// The season a followed show is narrowed to, or every season where absent.
    pub season: Option<u32>,
    /// The completed download the client is to be asked to let go, by the name both
    /// sides use.
    pub download: Option<String>,
    /// What is to happen to what the household asks for, as it is written.
    ///
    /// A word rather than a switch, for the reason `unrated` is one: there are three
    /// answers rather than two, and a word this build does not know is refused by name
    /// instead of falling to whichever arrangement the shape happened to default to.
    pub policy: Option<String>,
    /// How many requests a period allows.
    pub requests: Option<u32>,
    /// How long that period is, in days.
    pub days: Option<u32>,
    /// The request being ruled on, by the number the request service files it under.
    pub request: Option<i64>,
    /// Why a request is being turned down.
    pub reason: Option<String>,
}

/// Whether a run includes the checks that disturb a running system.
///
/// A two-variant reading of the bare word a surface carries rather than a fourth
/// flag among three: `--fix-disruptive` on a command line and a `disruptive` in a
/// request body are one word that is there or is not, and which way round it reads
/// is decided here, once. A surface that read it the other way round would drop the
/// default route out from under a stack nobody asked it to disturb.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(from = "bool")]
pub enum Disturbing {
    /// Left out, which is what an ordinary run does.
    #[default]
    Left,
    /// Included, because the operator asked for them.
    Included,
}

impl From<bool> for Disturbing {
    fn from(asked: bool) -> Self {
        if asked {
            Self::Included
        } else {
            Self::Left
        }
    }
}

impl Disturbing {
    /// Whether the command this reaches is told to include them.
    #[must_use]
    pub const fn included(self) -> bool {
        matches!(self, Self::Included)
    }
}
