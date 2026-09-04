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
/// only asked what was wrong with it.
///
/// What the six do **not** share is a shape. This is the list of actions whose
/// command has a field for the agreement, and nothing more: "unconfirmed, it says
/// what it would do and changes nothing" is true of four of them and false of the
/// other two. Said here rather than left to be found, because a surface that read
/// the list as the rule would put an account of what is about to happen in front of
/// a run that has already happened.
///
/// Three of them are the fork the reading suggests. Unconfirmed, `reset`,
/// `restore` and `quality-upgrade` each report what it would cost and change
/// nothing, so the agreement is inside the command rather than a gate in front of
/// it.
///
/// A repair is the fourth, and the one where it is the whole of the design rather
/// than a guard in front of it. Unconfirmed it *is* the offer — each repair with
/// what it would do and what else changes if it does — and the yes that follows
/// names the offer it was read in through [`TAKES_CONSENT`]. Given alone it is the
/// standing consent the command line spells `--yes`: a decision taken before there
/// was an offer to read, which is a different thing from skipping being told. What
/// the offer half is *not* allowed to do is disturb the stack to build itself, which
/// is why the widening beside it belongs to the half that acts — see
/// [`TAKES_DISRUPTION`].
///
/// `quality-set` is not a fork at all. Unconfirmed it records the choice; the
/// agreement answers one particular cost inside a run that writes either way — a
/// preset this host would have to transcode in software, which is held rather than
/// recorded until it is agreed to, and which is not a cost at all on a machine that
/// transcodes in hardware. So an unconfirmed run there is the choice being made,
/// and a surface that put an account in front of it would be describing a decision
/// already taken.
///
/// A bundle's agreement is the revealing and never the file. `confirm` answers
/// `reveal`, which prints a setting as it is into a thing people post in public;
/// whether there is a file at all is `write`'s, and that fork is already the second
/// deliberate run — a bare run collects, redacts, scans and says what a bundle would
/// hold. So a bundle written without an agreement has revealed nothing, which is
/// what the agreement is for, and the two are separate fields because they are
/// separate decisions.
///
/// Most of them the command line declares a `--confirm` for, and a repair declares
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
    "forget",
    "remove",
    "quality-set",
    "quality-upgrade",
    "repair",
    "reset",
    "restore",
    "space",
    "support",
];

/// The actions whose command carries what the operator read before answering.
///
/// The three that show the operator something and then act on what they answered. A
/// repair is offered and then carried out; a restore is listed and then overwrites;
/// letting a download go states what that costs a tracker's opinion of somebody and
/// then asks the client to take it. All three are two requests with a decision in the
/// gap, and in that gap the thing that was read can move — a repair's effects
/// rewritten by a fresh diagnosis, a restore's re-point derived again from a data root
/// that has changed, a download's ratio earned while somebody was deciding. So the
/// answer names what it was given for, and the run that acts builds that name again
/// and compares.
///
/// On two of them it sits beside a `confirm` that can stand in for it. On the third
/// there is no `confirm` at all — see [`TAKES_AGREEMENT`], which deliberately leaves
/// it out — so the name of the offer is the only way to say yes to it. That is the
/// point of it there: a blanket yes would be a removal agreed to by somebody who had
/// not read what it costs.
///
/// No other action needs one, because no other action shows the operator something
/// and then acts on what they answered. Everywhere else the reply is the answer.
pub const TAKES_CONSENT: &[&str] = &["repair", "restore", "stop-seeding"];

/// The action whose command carries which completed download it is about.
///
/// Letting one go, and nothing else. It is required rather than optional: this is the
/// one request in the whole surface that destroys something outside this machine's
/// filesystem, and one with nothing named would be a request that has lost the only
/// subject that makes it safe.
///
/// Apart from every other list here because it is a different kind of subject. A form
/// is part of the stack, a service is part of a form, a name is a person, and this is
/// a torrent — matched by the name the download client and the disk account both use.
pub const TAKES_DOWNLOAD: &[&str] = &["stop-seeding"];

/// The action whose command carries *which* of what it read was agreed to.
///
/// A repair and nothing else, and it is apart from [`TAKES_CONSENT`] because it is
/// a different half of the same answer. An offer holds several repairs and the
/// operator picks from them, so an offer with none of it agreed to is consent that
/// has lost its subject. A restore has one archive and nothing to pick out of it,
/// so a list of what was agreed to would name nothing there.
pub const TAKES_AGREED: &[&str] = &["repair"];

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
/// On a repair it belongs to the half that acts, and the core refuses it to the half
/// that does not. The offer is what an operator reads before deciding anything, and
/// these checks prove themselves by disturbing — the tunnel goes away, a live search
/// is spent — so a run that did either to say what it *would* do has already done it.
/// Refused rather than quietly narrowed, because neither disturbing check turns up a
/// repair to offer: what a caller widening an offer wants is what those checks found,
/// and that is `diagnose`, which reports them.
///
/// The two reads this surface serves take no such argument. A check that disturbs a
/// running system changes something, and so does a trace that spends one of the
/// indexers' daily allowance to answer — and changes are asked for here.
///
/// A searching trace is the fourth, and it is the diagnosis's arrangement again for the
/// same reason. The read at `/api/trace` follows an item across the services and touches
/// nothing; widened, it asks the indexers what they carry, which is the one thing a
/// trace can do that reaches past this machine. Required there too: a trace that asks
/// the indexers nothing is the read already served.
pub const TAKES_DISRUPTION: &[&str] = &["repair", "accept", "diagnose", "search"];

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

/// The actions that are about a person rather than a form, a file or a service.
///
/// Four. Three of them are one errand — offering somebody an account, letting its
/// password be set again, and taking it away — and each of those *requires* a name,
/// because a request addressed to nobody has lost its subject.
///
/// The fourth is not that errand and does not require one. Choosing what may be asked
/// for is a decision about the household, and naming somebody narrows it to them: a
/// choice with nobody named is the household's own, which is a request rather than an
/// omission. So the name is optional there and refused nowhere else, which is exactly
/// what this list says and [`crate::actions::named`] enforces separately.
pub const TAKES_NAME: &[&str] = &["invite", "reissue", "remove", "household-allow"];

/// The action whose command carries what the household may ask for.
///
/// One, and all three arguments belong to it because they are one decision: a policy
/// without a limit is half of "within a limit", and a limit without a policy is a count
/// nothing acts on. Named to anything else they would be a decision about a household
/// that is not being made.
///
/// The two numbers are one group rather than two because neither means anything alone —
/// "five" is not a limit and "a week" is not a limit, and the command line refuses
/// either without the other before this is reached.
pub const TAKES_POLICY: &[&str] = &["household-allow"];

/// The actions whose command carries the request being ruled on.
///
/// Two, and the number is required of both: a decision with no request has lost its
/// subject, and answering it with every waiting request would rule on things nobody
/// mentioned.
///
/// Apart from [`TAKES_POLICY`] although both are about what a household may ask for,
/// because they are different errands. One settles what happens to everything from now
/// on; these settle one thing somebody already asked for. An action that took one for
/// the other would have an operator setting a limit and finding they had approved
/// something.
pub const TAKES_REQUEST: &[&str] = &["household-approve", "household-decline"];

/// The action whose command carries why a request was turned down.
///
/// **One, and it is not the pair.** An approval owes the person who asked the thing
/// they asked for; a refusal owes them a sentence. So a reason named to an approval is
/// refused by name here rather than accepted and dropped — and the core carries it
/// inside the variant that needs it, so a refusal cannot be built without one at all.
pub const TAKES_REASON: &[&str] = &["household-decline"];

/// The action whose command carries what the person being invited may watch.
///
/// An invitation and nothing else. It is apart from [`TAKES_NAME`] because it is a
/// different half of the same errand: all three of those are addressed to a person,
/// and only one of them decides anything about the account. A reissue takes a password
/// off an account whose access somebody already chose, and a removal takes the account
/// away, so a library, an age limit or a choice about unrated content named to either
/// would be a choice about an account that is not being made.
///
/// The three are one list because they are one decision, taken at one moment. Which
/// libraries somebody may open, how far up the ratings they may go, and what happens to
/// content the media server has no rating for are together what an account is *for*,
/// and the reason all are asked here rather than left for later is the same reason: an
/// account made open and narrowed afterwards is open for as long as it takes anybody to
/// remember.
pub const TAKES_ALLOWANCE: &[&str] = &["invite"];

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

/// The action whose command carries what it was told to follow.
///
/// A searching trace and nothing else, and it is required rather than optional: a
/// trace with nothing to follow is a request that has lost its subject, refused here
/// in the sentence `/api/trace` refuses the same omission with.
///
/// Apart from [`TAKES_ITEM`] although both name one piece of content, because the two
/// commands do different things with it. A walk is told what to add and goes and adds
/// it; this is told what to look for and reports where it already is. An action that
/// took one for the other would have an operator asking where something was and
/// finding it had been fetched.
///
/// The season it may be narrowed by belongs to the same group. A season means nothing
/// without the show it is a season of, and there is no other request here that a
/// season narrows.
pub const TAKES_TERM: &[&str] = &["search"];

/// The argument this action's command has nowhere to put, where one was given.
///
/// Only for a name this surface offers — a name it does not offer is absent before
/// it is anything else, and saying what its arguments should have been would be
/// answering about an action that does not exist.
pub fn unwanted(action: &str, given: &Arguments, offered: &[&str]) -> Option<Refused> {
    let carried: [(&str, bool, &[&str]); 33] = [
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
        ("name", given.name.is_some(), TAKES_NAME),
        ("libraries", !given.libraries.is_empty(), TAKES_ALLOWANCE),
        ("age_limit", given.age_limit.is_some(), TAKES_ALLOWANCE),
        ("unrated", given.unrated.is_some(), TAKES_ALLOWANCE),
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
        ("agreed", !given.agreed.is_empty(), TAKES_AGREED),
        ("confirm", given.confirm, TAKES_AGREEMENT),
        ("item", given.item.is_some(), TAKES_ITEM),
        ("term", given.term.is_some(), TAKES_TERM),
        ("season", given.season.is_some(), TAKES_TERM),
        ("download", given.download.is_some(), TAKES_DOWNLOAD),
        ("policy", given.policy.is_some(), TAKES_POLICY),
        ("requests", given.requests.is_some(), TAKES_POLICY),
        ("days", given.days.is_some(), TAKES_POLICY),
        ("request", given.request.is_some(), TAKES_REQUEST),
        ("reason", given.reason.is_some(), TAKES_REASON),
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
