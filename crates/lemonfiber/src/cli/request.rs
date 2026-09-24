//! The subcommands, and nothing about what any of them means.
//!
//! Apart from the parser that carries it for one reason: this enum is the whole of
//! what the binary can be asked to do, so it grows every time the binary learns
//! anything, and a file holding it as well as the global flags and the help would
//! outgrow one sitting on the next command rather than eventually.
//!
//! What the flags of a single subcommand look like stays in the small modules
//! beside this one — a subcommand with enough of its own to say has always had a
//! file for it, and this file is the index of them rather than a second home.

use std::path::PathBuf;

use clap::Subcommand;

use super::{
    AlertCommand, Asked, ConfigAction, HostingCommand, HouseholdCommand, MigrateCommand,
    PluginCommand, QualityCommand, RawAllowance, RawBandwidth, RawCredentials, RawDoctor,
    RawRemoving, RawSetup, RawUi, UpdateCommand,
};

/// What the operator asked for.
#[derive(Debug, Subcommand)]
pub enum Request {
    /// Set up the stack by answering a few questions.
    ///
    /// Interactive by default. Given the flags below, it runs unattended: each
    /// answers a question the wizard would otherwise ask, and `--yes` stands in for
    /// the confirmation. A non-interactive run missing a flag it needs is told
    /// which, rather than left waiting on input that will not come.
    Setup {
        /// The answers, as the command line gives them.
        #[command(flatten)]
        flags: RawSetup,
    },
    /// Report the versions in play.
    Version,
    /// List the forms this stack has, and what each one is for.
    ///
    /// A form says which part of the stack to run. They come from the stack rather
    /// than from lemonfiber, so a stack of your own names its own.
    ///
    /// Naming one says what starting it would come to — the services it holds, and
    /// anything your configuration leaves out — without starting anything.
    Forms {
        /// The forms to describe; none lists them all.
        forms: Vec<String>,
    },
    /// See what is already on this machine, and take it over if you choose to.
    ///
    /// With nothing named it surveys: the stacks already standing here, the ports they
    /// hold that lemonfiber would want, and anything it could not take over as it
    /// stands. Nothing is started, stopped, moved, or written.
    Migrate {
        /// What to do about what was found; nothing surveys and changes none of it.
        #[command(subcommand)]
        action: Option<MigrateCommand>,
    },
    /// Start a form, or the union of several.
    Up {
        /// The forms to start; none starts everything the stack declares.
        forms: Vec<String>,
        /// Start only these services, leaving the rest of the form alone.
        #[arg(long = "service", value_name = "NAME")]
        services: Vec<String>,
        /// Start what a restart of this machine should start, and nothing otherwise.
        ///
        /// What a login runs. It brings back whichever form was last running unless
        /// you pinned one, and it declines — saying why — where you stopped the stack
        /// on purpose, where you never asked for it to start on its own, or where this
        /// machine is on its battery and you have not said to start anyway. It waits
        /// for the container engine to finish starting and tries again while the
        /// network is still arriving, and if the stack still does not come back it
        /// records that, so the next thing you type tells you once rather than not at
        /// all. Naming a form or a service alongside it is refused: which forms come
        /// back is the record's answer, not this command line's.
        #[arg(long, conflicts_with_all = ["forms", "services"])]
        at_boot: bool,
    },
    /// Stop and remove what a form started.
    Down {
        /// The forms to stop; none stops everything the stack declares.
        forms: Vec<String>,
        /// Stop only these services, leaving the rest of the form running.
        #[arg(long = "service", value_name = "NAME")]
        services: Vec<String>,
        /// Let anything still downloading finish before stopping.
        ///
        /// Not for a stop of named services: what is in flight is a question about
        /// the download clients a form holds, so naming two services that are not
        /// download clients would wait on downloads stopping them cannot interrupt.
        #[arg(long, conflicts_with = "services")]
        wait: bool,
        /// Stop without asking about anything still downloading.
        #[arg(long, conflicts_with = "wait")]
        yes: bool,
    },
    /// Make these forms the active set, leaving shared services running.
    ///
    /// Only what falls outside the new shape is stopped. A service the old shape
    /// and the new one both hold keeps running rather than being restarted, so a
    /// download in flight is not interrupted to change the stack around it.
    Switch {
        /// The forms to switch to.
        #[arg(required = true)]
        forms: Vec<String>,
    },
    /// Restart services without touching the rest.
    Restart {
        /// The form holding them.
        form: String,
        /// The services to restart; none restarts the whole form.
        services: Vec<String>,
    },
    /// Fetch newer images without applying them.
    Pull {
        /// The forms whose images to fetch.
        #[arg(required = true)]
        forms: Vec<String>,
    },
    /// Report what each service is actually doing.
    Ps {
        /// The forms to report on; none reports the whole stack.
        forms: Vec<String>,
    },
    /// Show what services are saying.
    Logs {
        /// The services to read; none reads them all.
        services: Vec<String>,
        /// Read only the services a form declares.
        #[arg(long, value_name = "FORM")]
        form: Vec<String>,
        /// Keep reading as new lines arrive.
        #[arg(long, short)]
        follow: bool,
        /// Read them on a screen that can be scrolled back and filtered.
        #[arg(long, conflicts_with = "follow")]
        watch: bool,
        /// How many existing lines to begin with.
        #[arg(long, default_value_t = 50)]
        tail: u32,
    },
    /// Read or change one setting.
    Config {
        /// Which of the three things to do with a setting.
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Choose how much lemonfiber tells you about, in plain language.
    Alerts {
        /// Show what you are told about, or change it.
        #[command(subcommand)]
        action: AlertCommand,
    },
    /// Choose how good your media should look, in plain language.
    Quality {
        /// Which of the four things to do with the quality choice.
        #[command(subcommand)]
        action: QualityCommand,
    },
    /// Run the checks that prove the stack is doing what it should.
    Doctor(RawDoctor),
    /// Install, update and remove plugins, and read what one may declare.
    ///
    /// Five of the words under this one are documents for somebody writing a plugin,
    /// answered with no network, no catalogue and no stack running, each saying which
    /// generation it reports — so an author who has to know whether a difference is
    /// their build or their manifest can tell. The other four are about this machine:
    /// `install` writes down what installing a plugin decides, `installed` reads that
    /// back, `update` replaces one version with another as one operation, and `remove`
    /// takes one off. Each of the three that acts can be rehearsed with `--dry-run`.
    Plugin {
        /// Which of them to ask for.
        #[command(subcommand)]
        read: PluginCommand,
    },
    /// Guard the data location while forms run, stopping them if it disappears.
    Watch {
        /// The forms to stop if the data location is lost.
        #[arg(required = true)]
        forms: Vec<String>,
    },
    /// Say what this machine keeps running when no terminal is open.
    ///
    /// Two of this program's commands have to keep running to be worth anything — the
    /// guard on the data location, and the clock that closes requests nobody rules on —
    /// and both stop when the window they were started in closes. This is what hands
    /// them to the machine instead.
    ///
    /// Asked nothing it reports what stands between each of them and this machine:
    /// whether one is installed, whether the system is actually running it, where its
    /// words are written, and — on a platform this program cannot configure — what to
    /// do instead of it. Installed is not running, and the two are never reported as
    /// one thing.
    ///
    /// Name one of the two words underneath to install one or take it back.
    Hosting {
        /// What to do about one of them, or nothing to read what stands.
        #[command(subcommand)]
        action: Option<HostingCommand>,
    },
    /// Follow one show or film across the services — "where is my show?".
    ///
    /// Searched for the way you would name it, not by an internal id. Reports how far
    /// it got and, where it plainly stopped, why. A show is reported season by season:
    /// how many episodes are here, and what each one that is not is waiting on.
    ///
    /// Something monitored that has never been grabbed stopped for one of two reasons,
    /// and nothing on this machine can tell them apart: the indexers carry nothing for
    /// it, or they carry releases the quality you chose rejects. `--search` asks them.
    Trace {
        /// The show or film to follow, named as you would say it.
        #[arg(required = true)]
        term: Vec<String>,
        /// Narrow to one season, instead of every season of the show.
        #[arg(long)]
        season: Option<u32>,
        /// Ask the indexers what they carry, to tell "nothing at your quality" from
        /// "nothing at all". Spends one real search against their daily allowance.
        #[arg(long)]
        search: bool,
    },
    /// Show who is in the household, what each may watch and ask for, and what each
    /// asked for.
    ///
    /// Everybody the media server holds an account for — including those who have
    /// asked for nothing, and the invitations nobody has taken up yet. Each person
    /// carries what they may watch and when they were last seen; their requests read
    /// in the words they would use rather than the services' own, and each named one
    /// links to its full trace.
    ///
    /// Each person also carries what they may ask for: how much a period allows them,
    /// how much of it is gone, and when there is room again. A request nobody has ruled
    /// on shows how long it has been waiting and about how much room it would want.
    ///
    /// Name one of the four things underneath to change any of that, to answer one
    /// request that is waiting, or to arrange what becomes of the ones nobody answers.
    Household {
        /// Narrow to one member, named the way you would say it.
        #[arg(long)]
        member: Option<String>,
        /// Decide what the household may ask for, or answer one waiting request.
        #[command(subcommand)]
        action: Option<HouseholdCommand>,
    },
    /// Show what one member can actually watch.
    ///
    /// The household word says what has been *asked for*. This says what is already
    /// here — read from the media server as that member, so it is their age limit,
    /// their blocked kinds and the libraries their account reaches that decide what
    /// comes back. Nothing in lemonfiber narrows it afterwards, which is why this is
    /// the only place a restriction can be seen as the list it comes to rather than
    /// as the setting it was typed in as.
    ///
    /// One person at a time, because no two accounts need have the same shelf.
    Held {
        /// Whose shelf, named the way you would say it.
        #[arg(long)]
        member: String,
        /// How many to show, newest first.
        #[arg(long)]
        most: Option<u32>,
    },
    /// Add one thing, end to end, and watch every step of it happen.
    ///
    /// The walk a first run is offered: search the indexers, grab a release, download
    /// it, import it, and see it appear in the library — narrated as it goes, so that
    /// afterwards you know what your stack does because you watched it do it. If any
    /// link is broken this is where it shows, with the step named and a way out.
    ///
    /// Name something, or name nothing and be suggested something likely to work.
    Walkthrough {
        /// What to add, named as you would say it.
        item: Vec<String>,
    },
    /// Say what one of this product's words means.
    ///
    /// A report explains the words it used underneath itself, in a sentence. This is
    /// the longer form, for somebody who wants it — nothing needs it in order to act,
    /// which is the difference between an explanation offered and one imposed.
    ///
    /// Name a word, or name nothing and be told which words there are.
    Explain {
        /// The word, as you would say it.
        word: Vec<String>,
    },
    /// Show everything lemonfiber changed, newest first, and how far each could be put
    /// back.
    ///
    /// The record only. Putting one back is asked for by name, because it acts on a
    /// running stack, and this says beforehand which of them could be.
    History,
    /// Put back one run of changes, named by the stamp `lemonfiber history` shows.
    ///
    /// The whole run and never half of one: a seed or a reconfigure is the unit an
    /// operator agreed to, and the history says beside each entry how many changes
    /// would go with it. Nothing is put back unless all of it can be.
    Undo {
        /// The stamp of the run to put back, copied from `lemonfiber history`.
        at: String,
    },
    /// List the items whose downloads are stuck — the landing point for "N stuck", each
    /// named so `lemonfiber trace` follows it on its own.
    Stuck,
    /// Name the one address to send somebody who lives here.
    ///
    /// The stack publishes several things to your network and only one of them is
    /// somewhere to begin. This says which, why the others are not, and — where this
    /// stack runs nothing anybody could begin at — that there is no address to send
    /// rather than naming the nearest thing that would open.
    FrontDoor,
    /// Say what each service in this stack is for, and what became of any it dropped.
    ///
    /// Twenty names convey nothing on their own. This gives each of them a sentence
    /// in plain language — what it does for you, what you lose while it is down, and
    /// how much that loss matters — so a stack you can list becomes a stack you can
    /// judge. Anything the stack used to carry and no longer does is listed after
    /// them, with why it went and what took its place.
    ///
    /// It reads the stack description and nothing else, so it answers with the
    /// machine off and the containers down.
    Catalogue,
    /// Say what this stack wires to what, and how each link was settled.
    ///
    /// A link asks for a capability — an identity source, a torrent client — and
    /// whatever provides it is what the link reaches, so putting something else in
    /// its place is one setting rather than a hunt for everything that named it. One
    /// kept to a named service is shown as one, with its reason, and where two claim
    /// the same thing `fill` is how you choose. Non-zero where nothing provides it.
    Wiring {
        /// The one verb: choose which service fills a capability.
        #[command(subcommand)]
        fill: Option<super::WiringCommand>,
    },
    /// List everything that leaves this machine, and what refusing each of it costs.
    ///
    /// lemonfiber's own requests first — where each goes, why, exactly what travels,
    /// whether it is on, the setting that switches it off and what stops working
    /// when it is — then the requests the stack's own services make, which are
    /// theirs rather than lemonfiber's.
    Outbound,
    /// Say where each service comes from: its licence, its project, and the exact
    /// version this stack pins it at.
    ///
    /// Everything bundled here is open source, and this is how you check that rather
    /// than take it on faith — the licence each service is published under, the
    /// project to go and read it at, and the image and tag actually being run.
    ///
    /// It reads the stack description and nothing else, so it answers with the
    /// machine off and the containers down.
    Provenance,
    /// Say which credentials this stack holds, or act on one of them.
    ///
    /// Every secret in the stack in one list, whoever produced it: what each is, what
    /// authenticates with it, where the value lives and where it stands — never the
    /// value itself — and what keeping them in files does and does not protect
    /// against. `--rotate` replaces one, proving the replacement before the existing
    /// value stops being in force; `--reveal` prints one, and asks first.
    Credentials(RawCredentials),
    /// List what lemonfiber keeps on this machine, where it is, and why.
    ///
    /// Everything it writes sits under two directories. This names each thing under
    /// them, says what it is for, and marks the ones holding a credential — and it
    /// names what is *not* lemonfiber's, because your library being absent from the
    /// list is the part worth being sure about.
    Stored,
    /// Say which app to watch on, for each kind of device somebody in the house has.
    ///
    /// The client landscape is uneven and it matters which app is used: some devices
    /// have an official one that works, and a smart television may have nothing worth
    /// using. This says which is which, names a browser as the answer that always
    /// works and needs no installation, and where a device is badly served says what
    /// to do instead rather than leaving somebody to find out by failing.
    Clients,
    /// Offer somebody in the house an account they can claim.
    ///
    /// Makes them an account on the media server with no password on it, and prints
    /// the one address to send them. Whoever sets the first password claims it; an
    /// invitation nobody takes up is withdrawn.
    Invite {
        /// What they will sign in as.
        name: String,
        /// What the account is to let them watch.
        #[command(flatten)]
        allowance: RawAllowance,
    },
    /// Let somebody set a new password, without you choosing or seeing it.
    ///
    /// Their account goes back to having no password on it — the state a fresh
    /// invitation leaves it in — so they claim it again by setting the first one
    /// themselves. Their old password stops working immediately. What this prints is
    /// the invitation to send them: the same address, the same code.
    Reissue {
        /// Whose account to make claimable again.
        name: String,
    },
    /// Take somebody out of the household, in both places they have an account.
    ///
    /// Revokes access to the media server and to the request service. Their watch
    /// history goes with the account — the media server offers no way to keep it —
    /// and the request service destroys what they asked for. Because none of that
    /// can be got back, it says what would go and does nothing until `--confirm`.
    Remove {
        /// Whose account to take away, as `lemonfiber household` shows them.
        name: String,
        /// Go ahead and remove them, having seen what goes.
        #[arg(long)]
        confirm: bool,
    },
    /// Remove everything lemonfiber keeps on this machine.
    ///
    /// The two directories and everything under them. Your library, your downloads
    /// and the containers are not lemonfiber's and are never touched. Because it
    /// throws work away it lists what would go and does nothing until `--confirm`.
    Forget {
        /// Go ahead and remove it, having seen what would go.
        #[arg(long)]
        confirm: bool,
    },
    /// Take lemonfiber off this machine, at one of four removals.
    ///
    /// `stop` removes nothing and stops the services. `services` removes the
    /// containers, the network they were on and the images pulled for them.
    /// `configuration` removes each service's own settings and everything lemonfiber
    /// keeps, credentials and all. `media` removes your library and your downloads,
    /// and is never bundled with any of the others.
    ///
    /// Naming a removal lists exactly what it would take — every container, image and
    /// path, with what each occupies — and does nothing. `--confirm` carries it out.
    /// The one that reaches your library takes no bare yes: the listing prints a name
    /// for itself and `--agreed` answers that name, so an answer given against one
    /// reading of your disk cannot be spent on another.
    Uninstall(RawRemoving),
    /// Account for the disk: where the room went, when it runs out, what can go.
    ///
    /// Says when the disk will be full rather than that it already is, counting what
    /// is queued against what is left. Breaks the rest down by where it went, and
    /// marks what could be got back — the downloads nothing ever imported and the
    /// archives already unpacked cost nothing at all. Those two are what `--confirm`
    /// takes, and nothing else ever is: a torrent still seeding is named with what
    /// removing it does to your standing with the tracker and left with you, and
    /// nothing you asked to be left alone is on offer at any level of fullness.
    Space {
        /// Go ahead and remove what was listed as costing nothing.
        #[arg(long)]
        confirm: bool,
    },
    /// Stop seeding one completed download, and let its files go with it.
    ///
    /// Everything the disk account names and deliberately leaves alone is asked for
    /// here instead, one download at a time. Named on its own it says what that
    /// download is, what it occupies, where it stands and, while it is still seeding,
    /// what removing it does to a private tracker's opinion of you. It removes
    /// nothing, and prints a name for that offer; answering with that name is the
    /// yes. There is no other way to say it, because a blanket confirmation would be
    /// agreement from somebody who had not read the cost.
    StopSeeding {
        /// The completed download, as the account and the client both name it.
        download: String,
        /// The offer being answered, as the run that made it printed it.
        #[arg(long, value_name = "NAME")]
        offer: Option<String>,
    },
    /// Account for the line: what it carries, what the stack takes, what that costs.
    ///
    /// Asked nothing it reports — the limits in force, which side of your day the
    /// clients say they are on, and whether they are actually keeping to what they
    /// were given. Asked for a limit it declares one and hands it to every download
    /// client, then reads back what each says, because a client that accepts a
    /// setting and does not apply it looks exactly like one that did.
    ///
    /// Nothing here touches anybody watching from your own library, and nothing
    /// here shapes this machine's traffic. It sets limits inside lemonfiber's own
    /// download clients and nowhere else.
    Bandwidth(RawBandwidth),
    /// Wire the stack's services to each other, idempotently.
    Seed,
    /// Adopt your current edits as lemonfiber's expected state.
    ///
    /// A value you changed by hand reports as drift until you adopt it; once
    /// adopted it is kept across future seeds and restores. Wires what is missing
    /// as a seed does, and promotes every drifted value to yours.
    Adopt,
    /// Put the stack back to lemonfiber's own state, reverting every edit you made.
    ///
    /// The opposite of adopt: it discards your hand-edits to the stack files and
    /// restores lemonfiber's own. Because it throws work away, it names exactly what
    /// will be lost and does nothing until `--confirm` — run it once to see the diffs,
    /// again with `--confirm` to reset.
    Reset {
        /// Go ahead and revert, having seen what will be lost.
        #[arg(long)]
        confirm: bool,
    },
    /// Move something onto a newer version, naming which.
    ///
    /// Two things here can be moved forward and the object is the whole of what tells
    /// them apart: `stack` moves the services somebody watches things on, and `self`
    /// moves this program. Neither is the smaller case of the other, so the object is
    /// required rather than defaulted — being handed the wrong one of these is being
    /// answered a question you did not ask.
    Update {
        /// Which of the two to move forward.
        #[command(subcommand)]
        object: UpdateCommand,
    },
    /// Back up your configuration to an archive, so it stops being precious.
    Backup {
        /// Back up one service's configuration instead of the whole stack.
        #[arg(long, value_name = "SERVICE")]
        service: Option<String>,
    },
    /// Gather everything a person helping you would ask for, with every value not
    /// named safe replaced by a stand-in.
    ///
    /// A bare run writes nothing. It collects, redacts, and reads the result back
    /// looking for anything that still resembles a credential, then says what the
    /// bundle would hold and how large it is — so the decision to make a file worth
    /// attaching to a public thread is taken after seeing what goes in it. Run it
    /// again with `--write` to produce it.
    ///
    /// Nothing is ever sent anywhere. The bundle is written here and stays here.
    Support(Asked),
    /// Serve the web interface, for as long as you leave it running.
    ///
    /// Started when you ask for it and not before: nothing is installed, nothing
    /// keeps running afterwards, and stopping it leaves nothing behind. It listens
    /// on this machine only.
    ///
    /// The connection is not encrypted, which it says when it starts, along with the
    /// whole address it was given and the token every request to it must carry. The
    /// token is minted for this run, printed once here, and kept nowhere else.
    Ui(RawUi),
    /// Restore your configuration from a backup archive.
    ///
    /// Verifies the archive and lists what it holds before anything is
    /// overwritten. A restore onto a different data root is refused until
    /// `--repoint` accepts moving it to this machine's.
    ///
    /// Name an archive, or name nothing and be told which backups this machine has
    /// kept.
    Restore {
        /// The archive to restore from.
        archive: Option<PathBuf>,
        /// Accept re-pointing to this machine's data root where it differs.
        #[arg(long)]
        repoint: bool,
    },
}
