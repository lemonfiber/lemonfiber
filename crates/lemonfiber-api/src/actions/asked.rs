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
//! agreement that turns out to guard nothing, a service named to a start that then
//! starts the whole form, or a bundle asked to show a setting by an action that
//! writes no bundle.
//!
//! Which action takes what is a list per argument rather than a match arm, so that
//! the answer can be read, counted and held against the flags the command line
//! declares.

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
    /// Whether a cost the action would incur was agreed to in advance.
    pub confirm: bool,
    /// The one thing to add end to end, as it would be said.
    pub item: Option<String>,
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
/// Five, and no others. Each names something that cannot be taken back once it is
/// done: quality this host would have to transcode in software, bandwidth spent
/// re-fetching a library that is already here, hand-edits to the stack files
/// discarded, a configuration overwritten by an archive, and a credential printed
/// into a file people post in public. Unconfirmed, each of the five reports what
/// it would cost and changes nothing, so the agreement is a fork inside the
/// command rather than a gate in front of it.
///
/// Four of them the command line declares a flag for. A restore is the one it
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
    "reset",
    "restore",
    "support",
];

/// The actions whose command carries the services it was given.
///
/// Stopping named services is `Command::Halt`, a different request from a teardown
/// rather than a teardown with an argument, and a restart carries the services it
/// restarts. No other command has a field to put them in.
///
/// `up` is the one whose absence costs something. The command line starts named
/// services through a streamed path of its own that never reaches a `Command`, so
/// there is nothing here to hand them to — and dropping them starts every service
/// the form holds, which is the answer to a request nobody made. Whether starting
/// named services is its own request, the way `Halt` is its own request, is a
/// question for the core rather than for this table.
pub const TAKES_SERVICES: &[&str] = &["down", "restart"];

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
    let carried: [(&str, bool, &[&str]); 15] = [
        ("forms", !given.forms.is_empty(), TAKES_FORMS),
        ("services", !given.services.is_empty(), TAKES_SERVICES),
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
