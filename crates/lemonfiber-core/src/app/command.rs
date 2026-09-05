//! What a surface is asking for, as a value.
//!
//! Apart from the dispatcher that routes it because the two are read for different
//! reasons: this is the whole of what any surface may ask, and a reader checking
//! that a command line, a browser and a screen offer the same things reads it
//! without the routing in the way. The dispatcher lives beside it and matches on
//! every variant here, so nothing can be added without somewhere to send it.

use crate::audio::Format;
use crate::doctor::Narrowing;
use crate::quality::Preset;

use super::{bundle, repair, restore, setup::SetupAction, support, Waiting};

/// What an invitation lets the person it is for watch.
///
/// One value over two answers because they are one decision taken at one moment: which
/// libraries somebody may open and how far up the ratings they may go are the two
/// halves of what an account is *for*, and every surface asks both while it is asking
/// who the account is for.
///
/// Libraries are named the way the media server's own screens name them and are turned
/// into that server's own identifiers once, in the core, so no surface carries a table
/// of its own. The age limit needs no turning: the media server keeps it as a number
/// and the number is an age — see [`crate::age_limit`], which holds the words said for
/// one.
///
/// The default is the ordinary case and the one nobody has to think about: every
/// library, and no limit at all.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Allowance {
    /// The libraries they may open, by name. Empty is every one.
    pub libraries: Vec<String>,
    /// The age above which the media server holds things back. `None` is no limit.
    pub age_limit: Option<u32>,
    /// What is to happen to content the media server has no rating for. `None` leaves
    /// it to the default a restriction carries — see [`crate::app::invite`].
    pub unrated: Option<crate::ports::service::Unrated>,
}

/// What a household is to be allowed to ask for, and who the choice is about.
///
/// One value over three answers because they are one decision: a policy without a limit
/// is half of "within a limit", and a limit without somebody to hold to it is a number
/// nobody is held to. Every surface asks all three at once for that reason.
///
/// **Nothing said is nothing changed.** A run that named only a limit is not a run that
/// chose to trust everybody; it is a run that said nothing about the policy, which leaves
/// whatever the household already had. A value written here for something nobody typed
/// would be a surface deciding on the household's behalf.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Chosen {
    /// Whose it is about, matched the way a name is typed — or the whole household
    /// where absent.
    pub member: Option<String>,
    /// What is to happen to what they ask for. `None` leaves the policy in force.
    pub policy: Option<crate::asking::Policy>,
    /// How much a period allows. `None` leaves whatever limit is in force.
    pub quota: Option<crate::ports::service::Quota>,
}

/// What is being done about one request that is waiting on somebody.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    /// The request, by the number the request service files it under.
    ///
    /// A number rather than a title: a household asks for the same film twice under two
    /// spellings, and a decision that matched on words could rule on the wrong one.
    pub request: i64,
    /// Which way it goes.
    pub answer: Answer,
}

/// The two ways a waiting request can go.
///
/// The reason sits inside the variant that needs one rather than beside both, so a
/// refusal cannot be constructed without one — which is what a decline owes the person
/// who asked, and is stronger here than a check somebody could forget to make.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    /// Let it through.
    LetThrough,
    /// Turn it down, with what the person who asked is owed.
    TurnedDown {
        /// Why, in the operator's own words.
        reason: String,
    },
}

/// What a surface is asking for.
///
/// Deliberately exhaustive. The surfaces ship in the same binary, so a new
/// command should stop the build until every surface has decided what to do
/// with it — silently rendering nothing is the failure this prevents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Report the binary's version, and the engine's where it can be reached.
    Version,
    /// List the forms this stack declares.
    Forms,
    /// Say what naming these forms would come to, without running anything.
    Preview {
        /// The forms to resolve, as they were named.
        forms: Vec<String>,
    },
    /// Start one or more forms.
    Up {
        /// The forms to start, resolved to the union of their closures.
        forms: Vec<String>,
    },
    /// Start named services, leaving the rest of the form where it is.
    ///
    /// Apart from [`Command::Up`] for the reason [`Command::Halt`] is apart from
    /// [`Command::Down`]: bringing a form up creates everything its closure holds,
    /// and this starts the ones named. Compose spells them differently too.
    Start {
        /// The forms the services belong to; none means the whole stack.
        forms: Vec<String>,
        /// The services to start.
        services: Vec<String>,
    },
    /// Stop and remove what a form started.
    Down {
        /// The forms to stop.
        forms: Vec<String>,
        /// Whether anything still downloading is let finish before the stop.
        ///
        /// The wait is inside the command rather than in front of it, so a surface
        /// that cannot sit in a loop asks for it by saying so. Whether to offer the
        /// choice at all is the surface's — a terminal asks, a machine-readable run
        /// is not asked — but the waiting itself is one implementation.
        wait: Waiting,
    },
    /// Stop named services, leaving the rest of what is running alone.
    ///
    /// Apart from [`Command::Down`] because they are different requests, not one
    /// request with an argument: a teardown removes what a form started, and this
    /// stops services that stay where they are. Compose spells them differently too.
    Halt {
        /// The forms the services belong to; none means the whole stack.
        forms: Vec<String>,
        /// The services to stop.
        services: Vec<String>,
    },
    /// Make these forms the active set, stopping only what falls outside them.
    Switch {
        /// The forms to switch to, resolved to the union of their closures.
        forms: Vec<String>,
    },
    /// Restart services without touching the rest.
    Restart {
        /// The forms holding those services.
        forms: Vec<String>,
        /// The services to restart; empty restarts the whole form.
        services: Vec<String>,
    },
    /// Fetch newer images without applying them.
    Pull {
        /// The forms whose images to fetch.
        forms: Vec<String>,
    },
    /// Read one setting.
    ConfigGet {
        /// The setting to read.
        key: String,
    },
    /// Change one setting.
    ConfigSet {
        /// The setting to change.
        key: String,
        /// What to change it to.
        value: String,
    },
    /// Show every setting, with credentials withheld.
    ConfigShow,
    /// Report what each service is actually doing.
    Ps {
        /// The forms to report on; empty reports on the whole stack.
        forms: Vec<String>,
    },
    /// Run the diagnostic checks: the whole suite, one category, or one check.
    Doctor {
        /// What the run is narrowed to. A single check is named by the identifier
        /// its finding carries, so a report can be read and asked for again.
        narrowing: Narrowing,
        /// Whether the operator opted into the checks that disturb the system.
        disruptive: bool,
        /// A check whose warning the operator is answering: they have weighed the
        /// cost and chosen it, so it stops leading from now on.
        accept: Option<String>,
    },
    /// Offer what the diagnosis found that lemonfiber can put right, and carry out
    /// whatever this run was given consent for.
    ///
    /// Apart from [`Command::Doctor`] because looking and changing are two errands:
    /// a diagnosis is a read every surface serves without asking anybody anything,
    /// and this states what each repair would do and what else changes if it does,
    /// and then acts only on what was agreed to.
    Repair {
        /// How much of the putting-right this run was given consent for.
        consent: repair::Consent,
        /// Whether the checks that disturb the running system are included, which
        /// is a decision apart from consenting to any repair they turn up.
        disruptive: bool,
    },
    /// Put back what the last repair changed, and nothing else.
    ///
    /// Its own command rather than an argument to [`Command::Repair`]: which repair
    /// was last, what reversing it takes and which of those need a service to reach
    /// are the core's to decide, so this carries no subject at all.
    Undo,
    /// Show or change the quality preset — how good media should look, and how
    /// much disk it should cost — in plain language.
    Quality(QualityAction),
    /// Upgrade existing content to the chosen preset — a separate, explicit action
    /// whose bandwidth cost is stated, and which does nothing until confirmed. Its
    /// own command rather than a quality action because it reaches the services
    /// asynchronously, where the others only read and write the recorded choice.
    QualityUpgrade {
        /// Whether the operator confirmed the cost; without it, only the cost is
        /// stated and nothing is triggered.
        confirm: bool,
    },
    /// Choose the audio format for music — media with no resolution — and apply it to
    /// the music service. Its own command, like the upgrade, because it reaches the
    /// service asynchronously rather than only recording a choice.
    QualityMusic {
        /// The audio format to record and apply.
        format: Format,
    },
    /// Follow one item across the services and report where it is — "where is my
    /// show?" — searched for by a human term rather than an internal id.
    Trace {
        /// The show, film, or request to follow.
        term: String,
        /// The season to narrow the per-part coverage to, or every season where absent.
        season: Option<u32>,
        /// Whether the indexers may be asked what they carry for it.
        ///
        /// The one read in a trace that costs something outside this machine: it
        /// spends a live search against the daily allowance the indexers hold the
        /// operator to. Without it a trace that finds an item wanted and never
        /// grabbed cannot say whether the indexers carry nothing or the quality in
        /// force wants none of what they carry, and it says so rather than picking
        /// one. Carried like [`Command::Doctor`]'s widening, and asked for at the
        /// door changes are asked for.
        searching: bool,
    },
    /// Report what the household has asked for and where each request stands, in the
    /// words the member who asked would use rather than the services' own.
    Household {
        /// The member to narrow to, or every member where absent.
        member: Option<String>,
    },
    /// Choose what the household may ask for — the policy, the limit, or both, for
    /// everybody or for one person.
    ///
    /// Answers with the household as it now stands rather than with a report of its
    /// own, the way a forget answers with what is left: what an operator wants to see
    /// after changing a limit is the limit, on the people it applies to.
    Allowing(Chosen),
    /// Rule on one request that is waiting, letting it through or turning it down.
    ///
    /// Apart from [`Command::Allowing`] because they are different errands: one settles
    /// what a household may ask for from now on, and this settles one thing somebody
    /// already asked for. Answers with the household for the same reason that one does.
    Deciding(Decision),
    /// List the items whose downloads are stuck, each named so it links to its own
    /// trace — the landing point for "N items stuck".
    Stuck,
    /// Name the one address to hand somebody who lives here, and say where it
    /// stands — including that there is none, where this stack runs nothing they
    /// could begin at.
    FrontDoor,
    /// Say what one of this product's words means, at length.
    ///
    /// Answered from a table compiled into the binary, so it needs neither a stack
    /// nor a daemon.
    Explain {
        /// The word, as it would be said.
        word: String,
    },
    /// List every word this product explains.
    ///
    /// Apart from [`Command::Explain`] the way listing forms is apart from
    /// resolving them: a surface that has to name a word cannot know the names in
    /// advance, and asking is what keeps it from carrying its own copy of the table.
    Glossary,
    /// Say which app to use on which device, and where the honest answer is to use
    /// something else.
    ///
    /// A read with no arguments. The table it answers with is the same for every
    /// machine — the client landscape belongs to the platforms rather than to this
    /// stack — and nothing is asked of the engine to build it.
    ///
    /// One caution above it is this machine's own: the quality preset on record and
    /// the platform decide together whether playback here will be transcoded on the
    /// processor, which is the likeliest cause of trouble on any of the devices
    /// below. Both are read best-effort, so a machine with nothing set up is
    /// answered in full and simply warned about nothing.
    Clients,
    /// Offer somebody in the house an account they can claim.
    ///
    /// Makes an account on the media server with no password on it, which is the
    /// whole of what an invitation is: whoever sets the first password claims it.
    /// Takes back any nobody claimed in time on the way past, because nothing runs
    /// between commands to do it on a clock.
    Invite {
        /// What they will sign in as.
        name: String,
        /// What the account is to let them watch.
        allowance: Allowance,
    },
    /// Put somebody's account back to having no password, so they can claim it again.
    ///
    /// The operator never chooses or reads a password: the account returns to the
    /// unclaimed state an invitation leaves it in, and whoever holds it sets the next
    /// first password themselves. What is handed back is the invitation to send them.
    Reissue {
        /// Whose account to make claimable again.
        name: String,
    },
    /// Take somebody out of the household, revoking their access to both the media
    /// server and the request service.
    ///
    /// Because it throws away what cannot be got back — their watch history goes with
    /// the account, and the request service destroys what they asked for — it says
    /// what would go and does nothing until `confirm`.
    Remove {
        /// Whose account to take away.
        name: String,
        /// Go ahead and remove them, having seen what goes.
        confirm: bool,
    },
    /// List everything that leaves this machine: what lemonfiber asks of the world
    /// on its own account, and what the stack's own services ask of it.
    ///
    /// A read with no arguments. What it answers depends on this machine's settings
    /// and on the stack the manifest declares, so there is nothing for a caller to
    /// narrow it by — and an enumeration a surface could narrow would be one an
    /// operator could be shown half of.
    Outbound,
    /// List what lemonfiber keeps on this machine: what each thing is, where it is,
    /// and why it is kept.
    ///
    /// A read with no arguments. What it answers is the layout this build carries
    /// against the directories this machine resolved, and there is nothing to narrow
    /// it by — a disclosure a surface could ask for half of is one an operator could
    /// be shown half of.
    Stored,
    /// Remove everything lemonfiber keeps on this machine.
    ///
    /// The whole of it: every location the layout names sits under one of two
    /// directories, and both go. What is not lemonfiber's — the library, the
    /// containers — is named in the answer and never touched.
    Forget {
        /// Whether the operator confirmed the loss; without it, what would go is
        /// listed and nothing is removed.
        confirm: bool,
    },
    /// Account for the disk: where the room went, when it runs out, and what could
    /// be got back.
    ///
    /// One argument, and it is the same shape a forget takes: unconfirmed it
    /// reports and offers, confirmed it takes what the report already named as
    /// costing nothing. There is no argument choosing *what* to reclaim, because
    /// the answer to that is never a caller's — a seeding torrent's removal has a
    /// consequence outside this machine and is named and left, and something the
    /// operator asked to be left alone is not on offer at any level of fullness.
    Space {
        /// Whether the operator agreed to what the unconfirmed run listed; without
        /// it, nothing is removed.
        confirm: bool,
    },
    /// Ask the download client to let one completed download go, files and all.
    ///
    /// The one thing the account above names and leaves alone, taken on its own. It
    /// is its own command rather than an argument to [`Command::Space`] because the
    /// spec forbids bundling a torrent's removal with generic cleanup, and because
    /// somebody agreeing to reclaim what costs nothing has not thereby agreed to lose
    /// a ratio a private tracker keeps their account on.
    ///
    /// The agreement is the offer's own name and there is no blanket form of it, so
    /// the only path to the removal runs through a run that stated the consequence.
    StopSeeding {
        /// Which completed download, by the name the client and the account both use.
        download: String,
        /// The offer being answered, as the run that made it named itself; without
        /// one, the cost is stated and nothing is removed.
        agreement: Option<String>,
    },
    /// Account for the line: what it carries, what the stack may take of it, and
    /// whether the clients are keeping to that.
    ///
    /// The same two halves the disk accounting has. Asked nothing it reads and
    /// reports; asked for a limit it declares one and hands it to every download
    /// client, then reads back what each says — because a client that accepts a
    /// setting and does not apply it looks exactly like one that did.
    Bandwidth(BandwidthAsked),
    /// Guard the data location while the given forms run, stopping them the moment
    /// it disappears.
    ///
    /// The one command with no ending of its own: everything else here answers and
    /// is done, and this holds until the location is lost or whoever asked for it
    /// stops asking. A surface that cannot be interrupted has to be able to say so.
    Watch {
        /// The forms to stop if the data location is lost.
        forms: Vec<String>,
    },
    /// Add one thing end to end, saying each step as it happens.
    ///
    /// Naming nothing asks for something safe to be suggested, because a first
    /// attempt that fails on an obscure choice teaches the wrong lesson entirely.
    Walkthrough {
        /// What to add, as it would be said, or nothing to be suggested something.
        item: Option<String>,
    },
    /// Wire the stack's services to each other, idempotently.
    Seed,
    /// Adopt the operator's current edits as lemonfiber's expected state, so they
    /// stop reporting as drift and are kept across future seeds and restores. Wires
    /// what is missing as a seed does, and promotes every drifted value to adopted.
    Adopt,
    /// Put the stack back to lemonfiber's own state, reverting every operator edit — the
    /// opposite of adopt. Because it discards their work, it names what will be lost and
    /// does nothing until confirmed: unconfirmed it only previews the reverts.
    Reset {
        /// Whether the operator confirmed the loss; without it, only the reverts are
        /// shown and nothing is written.
        confirm: bool,
    },
    /// Capture the configuration to a backup archive, so it stops being precious.
    Backup {
        /// The one service to capture instead of the whole stack, or every one of
        /// them where absent.
        service: Option<String>,
    },
    /// Gather everything somebody helping would ask for, with every value not named
    /// safe replaced by a stand-in.
    Support {
        /// Whether to produce the file, rather than say what one would hold.
        write: bool,
        /// What goes in it, and what was agreed to going in it.
        wanted: bundle::Wanted,
        /// Where it is written, for a run that produces one.
        dest: support::Destination,
    },
    /// List the backup archives this machine has kept, by the names they were
    /// written under.
    ///
    /// The half of a restore that comes before naming one, apart from it the way
    /// listing forms is apart from resolving them: a surface that has to name an
    /// archive cannot know the names in advance, and the one with no filesystem in
    /// front of it cannot look.
    Archives,
    /// Put a configuration back from a backup archive.
    Restore {
        /// The archive to restore from, named the way the surface can name one.
        archive: restore::Kept,
        /// Whether re-pointing to this machine's data root was accepted.
        repoint: bool,
        /// How much of the restore this run was given consent for, and for which
        /// listing. Without a yes the archive is verified and its contents listed,
        /// and nothing is touched.
        consent: restore::Consent,
    },
    /// Walk first-run setup: read where it stands, answer one question, move
    /// between them, or apply what has been answered.
    ///
    /// One step per command rather than the whole conversation, because a surface
    /// that cannot hold a conversation must still be able to have one — and the
    /// answers gathered so far live in the resumable progress file between them,
    /// which is where a terminal run keeps them too.
    Setup(SetupAction),
}

/// What a bandwidth request is asking for.
///
/// Every field is what the operator wrote rather than what it means, and is read
/// in the core: a surface that parsed a limit would be a second place the rule
/// about what `50%` means could change, and the three surfaces would then disagree
/// about a household's evening.
///
/// All of it absent is a reading, which is the request every surface makes first
/// and the one nothing is written by.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BandwidthAsked {
    /// The download limit, as a share or a figure.
    pub down: Option<String>,
    /// The upload limit, declared apart from the download's and defaulting lower.
    pub up: Option<String>,
    /// The household's hours, as `HH:MM-HH:MM` on the wall clock.
    pub active: Option<String>,
    /// What the line carries, as `<down>/<up>`, where the operator knows.
    pub line: Option<String>,
    /// A monthly cap on what the stack itself moves.
    pub cap: Option<String>,
    /// What is to happen when that cap is reached, decided in advance.
    pub exceeded: Option<String>,
    /// Lift the limits for this many minutes, and no longer.
    pub unrestricted_for: Option<u64>,
}

impl BandwidthAsked {
    /// Whether this request changes anything, or only asks.
    #[must_use]
    pub const fn anything(&self) -> bool {
        self.down.is_some()
            || self.up.is_some()
            || self.active.is_some()
            || self.line.is_some()
            || self.cap.is_some()
            || self.exceeded.is_some()
            || self.unrestricted_for.is_some()
    }
}

/// What a quality command asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QualityAction {
    /// Show the choice in force and what each preset means and costs.
    Show,
    /// Choose a preset — for everything, or for one media type — and record it.
    Set {
        /// The preset to choose.
        preset: Preset,
        /// The media type it applies to, or the whole library where absent.
        media_type: Option<String>,
        /// Whether the operator confirmed a choice this host would have to
        /// transcode in software, which is otherwise held rather than recorded.
        confirm: bool,
    },
    /// Re-assert the recorded preset over a hand-edited Recyclarr config — the
    /// explicit consent to let the preset win where a run would preserve the edit.
    Reapply,
}
