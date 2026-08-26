//! The arguments an action was given, and which actions take which.
//!
//! One carrier for every action rather than one shape per action, so a caller
//! sends the field an action names and nothing else. A name the carrier does not
//! hold is refused rather than ignored: a caller who spelled `service` where
//! `services` was meant has been told, instead of watching a whole form stop.
//!
//! A name the carrier holds and the action's command has nowhere to put is refused
//! too. Those are the fields that would have changed what the action did, so
//! dropping one answers a different request from the one that was asked — an
//! agreement that turns out to guard nothing, a preset named to an upgrade that
//! then re-fetches the library at the preset already recorded, or a bundle asked to
//! show a setting by an action that writes no bundle.
//!
//! And a pair the carrier holds that name two different requests when they arrive
//! together is refused as a pair, because there is no one of them to drop.
//!
//! Which action takes what is a list per argument rather than a match arm, so that
//! the answer can be read, counted and held against the flags the command line
//! declares.

use lemonfiber_core::app::Waiting;
use lemonfiber_core::bundle::Filenames;
use serde::Deserialize;

use super::Refused;

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
    /// The offer the repairs agreed to were read in, as it named itself.
    pub offer: Option<String>,
    /// The checks whose repairs were agreed to, as that offer names them.
    pub agreed: Vec<String>,
    /// Whether a cost the action would incur was agreed to in advance.
    pub confirm: bool,
    /// The one thing to add end to end, as it would be said.
    pub item: Option<String>,
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

/// The actions whose command carries the forms it was given.
///
/// The lifecycle five, and the guard. Seeding, adopting, resetting, changing a
/// setting and choosing a quality are whole-stack requests on every surface: no
/// command has a field to narrow them by and the command line declares no argument
/// for one, so a form named to any of them is a narrowing that was never available.
///
/// A guard's forms are not a narrowing at all. They are what it stops when the data
/// location goes, which is why the command line makes them the one thing it will
/// not run without — a watch over nothing would notice a drive vanish and have
/// nothing to do about it.
pub const TAKES_FORMS: &[&str] = &["up", "down", "switch", "restart", "pull", "watch"];

/// The actions whose command carries the operator's agreement.
///
/// Six, and no others. Each names something that cannot be taken back once it is
/// done: quality this host would have to transcode in software, bandwidth spent
/// re-fetching a library that is already here, hand-edits to the stack files
/// discarded, a configuration overwritten by an archive, a credential printed
/// into a file people post in public, and a machine changed under an operator who
/// only asked what was wrong with it. Unconfirmed, each of the six reports what
/// it would cost and changes nothing, so the agreement is a fork inside the
/// command rather than a gate in front of it.
///
/// A repair is the one where it is the whole of the design rather than a guard in
/// front of it. Unconfirmed it *is* the offer — each repair with what it would do
/// and what else changes if it does — and the yes that follows names the offer it
/// was read in through [`TAKES_CONSENT`]. Given alone it is the standing consent
/// the command line spells `--yes`: a decision taken before there was an offer to
/// read, which is a different thing from skipping being told.
///
/// Four of them the command line declares a `--confirm` for, and a repair declares
/// `--yes`, which is the same fork under the name that fits it. A restore is the one it
/// answers for itself: it lists what the archive holds and then restores, which is
/// the same two commands in one run, because the operator who typed it is there to
/// read the listing. A browser is not, so it is given the listing and asked again.
///
/// A teardown is not one of them. `down` removes what a form started and `up` puts
/// it back, so there is no cost to agree to in advance — and the question the
/// command line does ask before a teardown is not this one. It asks whether to let
/// a download in flight finish, which is answered by waiting rather than by
/// agreeing, and a machine-readable run is never asked it at all.
///
/// Choosing for music is inside `quality-set` and drops the agreement, because
/// picking an audio format is not a choice this host has to transcode for. The
/// command line drops it there too.
pub const TAKES_AGREEMENT: &[&str] = &[
    "quality-set",
    "quality-upgrade",
    "repair",
    "reset",
    "restore",
    "support",
];

/// The actions whose command carries the operator's consent to *this* offer.
///
/// A repair and nothing else. The two are one group because neither means anything
/// without the other: repairs are agreed to out of an offer, and an offer with no
/// repair agreed to out of it is consent that has lost its subject. Together with
/// the agreement beside them they are the whole of what a consent that crossed a
/// request boundary has to say — which offer, and which of it.
///
/// No other action needs one, because no other action shows the operator something
/// and then acts on what they answered. Everywhere else the reply is the answer.
pub const TAKES_CONSENT: &[&str] = &["repair"];

/// The actions whose command carries whether the disturbing checks are included.
///
/// Three. The command line spells it as two flags because clap keys an argument by
/// the field it sits on, so widening the suite while accepting a warning is
/// `doctor --disruptive` and widening it while repairing is
/// `doctor --fix --fix-disruptive`. One argument here, because what it asks for is
/// one thing: run the checks that disturb a running system.
///
/// On two of the three it widens a request that is about something else. On the
/// third it *is* the request, and there it is required rather than optional, for
/// the reason [`TAKES_CHECK`] is required on the action it belongs to: a diagnosis
/// that disturbs nothing is the read this surface already serves, and an action
/// answering it would be a second way to ask one thing.
///
/// It is not consent to a repair. Which checks run and which repairs are agreed to
/// are separate decisions, and an action that took one for the other would have an
/// operator widening the suite and finding it had also carried something out.
///
/// The diagnosis this surface serves as a read takes no such argument. A check that
/// disturbs a running system changes something, and changes are asked for here.
pub const TAKES_DISRUPTION: &[&str] = &["repair", "accept", "diagnose"];

/// The action whose command carries what the run is narrowed to.
///
/// A diagnosis and nothing else, and it is the one argument this surface takes at
/// both doors under one name: `/api/checks` narrows by `only` as a query parameter,
/// and this narrows by `only` as a field, because it is the same narrowing of the
/// same suite.
///
/// It matters most on the action. The checks a widened run adds are the two that
/// cost something — the tunnel goes away for as long as the killswitch test needs
/// it away, and a live release search spends one of the indexers' daily allowance —
/// and each of those findings tells the operator to run *that one*. Without a
/// narrowing here, following either instruction from a browser would mean running
/// both.
pub const TAKES_NARROWING: &[&str] = &["diagnose"];

/// The action whose command carries the check whose warning is being answered.
///
/// Accepting and nothing else. It is required rather than optional: an accept that
/// names no check is a diagnosis, which this surface already serves as a read.
pub const TAKES_CHECK: &[&str] = &["accept"];

/// The actions whose command carries the services it was given.
///
/// Three, and each of them a name for something narrower than the form. Starting
/// named services is `Command::Start` and stopping them is `Command::Halt` — each a
/// different request from the whole-form one rather than that request with an
/// argument, which is how Compose spells them too. A restart is the one that takes
/// the same command either way, because restarting a form is restarting every
/// service in it.
pub const TAKES_SERVICES: &[&str] = &["up", "down", "restart"];

/// The action whose command carries whether to let the downloads finish.
///
/// A teardown and nothing else. It is the one action that takes something away
/// while it may be in the middle of arriving, and the wait is inside the command
/// rather than in front of it — so a caller that cannot sit in a loop asks for it
/// by saying so, and what it gets back is a name for work that goes on after the
/// reply.
///
/// Not for the stop of named services beside it. What is in flight is a question
/// about the download clients a form holds, so a wait asked of two services that
/// are not download clients would hold up a stop for downloads stopping them
/// cannot interrupt — which is why the two are refused together rather than one of
/// them being dropped, and why the command line declares them in conflict.
pub const TAKES_WAITING: &[&str] = &["down"];

/// The action whose command carries the one service it was given.
///
/// A capture of one service's configuration is the whole of what the command line
/// declares `--service` for, and it is one name rather than a list because the
/// scope recorded in an archive is one scope. Apart from [`TAKES_SERVICES`] for the
/// same reason the command line spells them differently: those narrow what a
/// lifecycle command touches, and this decides what an archive covers.
pub const TAKES_SERVICE: &[&str] = &["backup"];

/// The action whose command carries the setting it was given.
///
/// One setting is read or written by name, and no other action is about a setting
/// at all.
pub const TAKES_SETTING: &[&str] = &["config-set"];

/// The action whose command carries the quality it was given.
///
/// Choosing is the only one of the three quality actions that takes a preset or a
/// media type. Re-asserting the recorded choice re-asserts what is recorded, and
/// upgrading re-fetches at what is recorded — neither takes a new choice, and the
/// command line declares neither argument on them.
///
/// `quality-upgrade` is where dropping one costs the most. A caller that named a
/// preset and a media type asked to upgrade one kind of media to one quality; the
/// command re-fetches the whole library at the quality already recorded, which is
/// a far larger download than the one the agreement beside it was given for.
pub const TAKES_PRESET: &[&str] = &["quality-set"];

/// The action whose command carries the archive it is from.
///
/// A restore and nothing else. The name and the acceptance of a re-point are one
/// group because neither means anything without the other: a re-point is accepted
/// for the archive being restored, and there is no archive being restored anywhere
/// but here.
pub const TAKES_ARCHIVE: &[&str] = &["restore"];

/// The action whose command carries what goes in a bundle.
///
/// A support run and nothing else. All four decide what the file holds — whether
/// there is a file at all, how much log to take, whether media filenames survive,
/// and which settings are printed as they are — and no other command produces a
/// file whose contents a caller chooses.
pub const TAKES_BUNDLING: &[&str] = &["support"];

/// The action whose command carries the one thing it was told to add.
///
/// A walkthrough and nothing else. It is the only request that is about a single
/// piece of content rather than about the stack — and naming none is a request in
/// its own right rather than an omission, because a walk asked for nothing in
/// particular suggests something likely to work, which is what an operator with an
/// empty library needs.
pub const TAKES_ITEM: &[&str] = &["walkthrough"];

/// The argument this action's command has nowhere to put, where one was given.
///
/// Only for a name this surface offers — a name it does not offer is absent before
/// it is anything else, and saying what its arguments should have been would be
/// answering about an action that does not exist.
pub fn unwanted(action: &str, given: &Arguments, offered: &[&str]) -> Option<Refused> {
    let carried: [(&str, bool, &[&str]); 21] = [
        ("forms", !given.forms.is_empty(), TAKES_FORMS),
        ("services", !given.services.is_empty(), TAKES_SERVICES),
        (
            "wait",
            matches!(given.wait, Waiting::ForTheDownloads),
            TAKES_WAITING,
        ),
        ("service", given.service.is_some(), TAKES_SERVICE),
        ("key", given.key.is_some(), TAKES_SETTING),
        ("value", given.value.is_some(), TAKES_SETTING),
        ("preset", given.preset.is_some(), TAKES_PRESET),
        ("media_type", given.media_type.is_some(), TAKES_PRESET),
        ("archive", given.archive.is_some(), TAKES_ARCHIVE),
        ("repoint", given.repoint, TAKES_ARCHIVE),
        ("write", given.write, TAKES_BUNDLING),
        ("logs", given.logs.is_some(), TAKES_BUNDLING),
        (
            "filenames",
            matches!(given.filenames, Filenames::Shown),
            TAKES_BUNDLING,
        ),
        ("reveal", !given.reveal.is_empty(), TAKES_BUNDLING),
        ("only", given.only.is_some(), TAKES_NARROWING),
        ("check", given.check.is_some(), TAKES_CHECK),
        ("disruptive", given.disruptive.included(), TAKES_DISRUPTION),
        ("offer", given.offer.is_some(), TAKES_CONSENT),
        ("agreed", !given.agreed.is_empty(), TAKES_CONSENT),
        ("confirm", given.confirm, TAKES_AGREEMENT),
        ("item", given.item.is_some(), TAKES_ITEM),
    ];
    if !offered.contains(&action) {
        return None;
    }
    carried
        .into_iter()
        .find(|(_, was_given, takers)| *was_given && !takers.contains(&action))
        .map(|(argument, _, _)| Refused::Unwanted {
            action: action.to_owned(),
            argument: argument.to_owned(),
        })
}
