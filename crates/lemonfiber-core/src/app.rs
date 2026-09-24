//! The one way in.
//!
//! A surface turns input into a [`Command`], hands it to [`dispatch`], and
//! renders the [`Outcome`]. A keypress, a subcommand and an HTTP route all
//! become the same value, so the three surfaces cannot grow behaviour that only
//! one of them has — which is the drift that otherwise happens quietly, one
//! convenience flag at a time.
//!
//! Whether this is a rehearsal is a property of the [`Ctx`], not a second code
//! path, so there is no parallel implementation to fall out of step.

use crate::asking::run as asking;
use crate::autostart::run as autostart;
use crate::backup::run as backup;
use crate::bandwidth::run as bandwidth;
use crate::bundle::run as bundle;
use crate::doctor::Narrowing;
use crate::door::run as door;
use crate::error::{Code, Diagnose, Problem};
use crate::household::run as household;
use crate::migration::run as migration;
use crate::notify::run as notify;
use crate::quality::run as quality;
use crate::repair::run as repair;
use crate::seed::run as seed;
use crate::self_update::run as self_update;
use crate::space::run as space;
use crate::stack::compose::Action;
use crate::stored::run as stored;
use crate::uninstall::run as uninstall;
use crate::update::run as update;
use crate::walkthrough::run as walkthrough;
use crate::wiring::run as wiring;
// The one function named rather than reached through its module below. The three
// settings arms are the longest in the dispatcher, and the module prefix on each of
// them is what pushed it past the length a function may be.

pub mod accepted;
pub(crate) mod adopt;
pub mod appetite;
pub mod apply;
pub mod archives;
pub(crate) mod arrangement;
pub(crate) mod beside;
pub(crate) mod boot;
pub(crate) mod bounded;
pub(crate) mod command;
pub mod conditions;
pub(crate) mod configuring;
pub(crate) mod credentials;
pub(crate) mod ctx;
pub mod disturbance;
pub(crate) mod engine;
pub(crate) mod expiring;
#[cfg(test)]
pub(crate) mod fixtures;
pub mod forwarding;
pub(crate) mod held;
pub(crate) mod history;
pub(crate) mod hosting;
pub(crate) mod import;
pub(crate) mod invite;
pub(crate) mod letting;
pub(crate) mod materialise;
pub(crate) mod music;
pub(crate) mod outbox;
pub mod plugins;
pub(crate) mod preflight;
pub mod putting_back;
pub(crate) mod quiesced;
pub(crate) mod reconfiguring;
pub(crate) mod record;
pub mod recover;
pub(crate) mod refusals;
pub mod rehearsal;
pub(crate) mod remove;
pub(crate) mod replace;
pub(crate) mod reset;
pub mod restore;
pub(crate) mod screen;
pub mod setup;
pub mod support;
pub(crate) mod targets;
pub(crate) mod trace;
pub mod unforwarded;
pub(crate) mod upgrade;
pub mod watch;

pub use command::{
    AlertAction, Allowance, Answer, Arranged, Asking, BandwidthAsked, Chosen, Command, Decision,
    Filling, Hostable, Keeping, Linking, MigrateAction, QualityAction, Removing, Setting, HOSTABLE,
};
pub(crate) mod outcome;
pub use ctx::{Ctx, PATIENCE};
pub use outcome::Outcome;
pub use rehearsal::{asked, carried, permitted, Asked, Rehearsal};
pub use setup::SetupAction;

// The log-following reads a surface streams from live outside dispatch, so they are the
// engine module's functions re-exported for the binary and the log commands to reach.
pub use engine::{
    claimed, diagnose, in_flight, logs, pull_progress, released, start_progress, started, Claim,
    Interrupted, Waiting,
};
pub use notify::{notify, Notified, CHANNEL_CHECK};
pub use walkthrough::{walkthrough, worth_offering};
// Named at the import rather than at the arm: every other command in the dispatch
// below is one line, and the module and the variant behind this one are together long
// enough that spelling it out there is three.
use self_update::standing as stands;

// The data-location watch is a self-contained feature in its own module; these
// are the names the rest of the crate and the binary reach it by.
pub use watch::{supervise, ALREADY_GONE, NOTHING_TO_WATCH, WATCH};

pub use crate::error::codes::life::NEVER_SETTLED;

pub use crate::error::codes::life::STILL_NEEDED;

pub use crate::error::codes::life::ALREADY_WORKING;

pub use crate::error::codes::life::REGISTRY_REFUSED;

pub use crate::error::codes::life::ABSENT_THERE;

/// Ask the engine to act on a set of services, which three commands do identically.
///
/// Named apart because they differ only in the action, and three arms that said the same
/// thing three times is what left `dispatch` with no room for a new command.
async fn acting(ctx: &Ctx, forms: &[String], action: Action) -> Result<Outcome, Box<Problem>> {
    engine::lifecycle(ctx, forms, &action).await
}

/// Offer somebody an account, which is the one request this table takes apart.
///
/// Named apart for the reason [`acting`] is: every other row here passes what it was
/// given straight along in one line, and an invitation carries three things. Spelling
/// them out in the table would make the request this file does least with the longest
/// arm in it.
async fn invited(ctx: &Ctx, name: String, allowance: Allowance) -> Result<Outcome, Box<Problem>> {
    invite::offer(ctx, name, allowance)
        .await
        .map(Outcome::Invited)
}

/// What the services themselves reach out for, which needs the stack read first.
///
/// Named apart for the reason [`acting`] is: every other row here passes what it was
/// given straight along, and this one has a step before it. The stack has to be
/// readable, because half the answer is about it: a manifest that could not be read
/// would leave the services' own requests reading as none at all, which is a claim
/// rather than a gap.
fn outbound(ctx: &Ctx) -> Result<Outcome, Box<Problem>> {
    let manifest = ctx
        .stack
        .manifest()
        .map_err(|err| Box::new(err.problem()))?;
    // What is installed is part of the answer, and a record that is there and will not
    // read refuses it: an account of what leaves this machine that quietly left a
    // stranger's plugin out would be believed.
    let installed = plugins::read(ctx)?;
    Ok(Outcome::Outbound(crate::outbound::leaving(
        &ctx.settings,
        &manifest.services,
        installed.installed(),
    )))
}

/// A diagnosis, and the warning it was told to consider answered.
///
/// Named apart for the reason [`acting`] is: the run happens first and what it found
/// is then read against what was accepted, which is two steps rather than a
/// pass-along.
async fn diagnosed(
    ctx: &Ctx,
    narrowing: Narrowing,
    disruptive: bool,
    accept: Option<String>,
) -> Result<Outcome, Box<Problem>> {
    let report = engine::diagnose(ctx, &narrowing, disruptive).await?;
    accepted::acknowledge(ctx, accept.as_deref(), report).map(Outcome::Doctor)
}

/// What letting one completed download go costs, and what became of letting it.
async fn letting(
    ctx: &Ctx,
    download: String,
    agreement: Option<String>,
) -> Result<Outcome, Box<Problem>> {
    letting::stop_seeding(ctx, download, agreement)
        .await
        .map(Outcome::Letting)
}

/// A walk through the stack, said onto whatever the surface is listening with.
async fn walked(ctx: &Ctx, item: Option<String>) -> Result<Outcome, Box<Problem>> {
    walkthrough::walkthrough(ctx, item.as_deref(), ctx.steps.as_ref())
        .await
        .map(Outcome::Walkthrough)
}

/// What a trace follows, and whether it reaches past this machine to find it.
async fn traced(
    ctx: &Ctx,
    term: String,
    season: Option<u32>,
    searching: bool,
) -> Result<Outcome, Box<Problem>> {
    trace::trace(ctx, &term, season, searching)
        .await
        .map(Outcome::Trace)
}

/// The bundle a support request asks for, gathered and put where it was asked for.
async fn bundled(
    ctx: &Ctx,
    wanted: crate::bundle::run::Wanted,
    write_it: bool,
    dest: crate::app::support::Destination,
) -> Result<Outcome, Box<Problem>> {
    support::run(ctx, &wanted, write_it, &dest)
        .await
        .map(Outcome::Support)
}

/// What an archive would put back, or the putting back of it.
async fn restored(
    ctx: &Ctx,
    archive: crate::app::restore::Kept,
    repoint: bool,
    consent: crate::app::restore::Consent,
) -> Result<Outcome, Box<Problem>> {
    restore::run(ctx, &archive, repoint, &consent)
        .await
        .map(Outcome::Restore)
}

/// What became of one thing the household asked for.
async fn decided(
    ctx: &Ctx,
    decision: crate::app::command::Decision,
) -> Result<Outcome, Box<Problem>> {
    asking::deciding(ctx, &decision)
        .await
        .map(Outcome::Household)
}

/// Putting right what the diagnosis found, at the operator's word.
async fn mended(
    ctx: &Ctx,
    consent: &repair::Consent,
    disruptive: bool,
) -> Result<Outcome, Box<Problem>> {
    repair::putting_right(ctx, consent, disruptive)
        .await
        .map(Outcome::Repair)
}

/// Carry out a command.
///
/// What it takes away is said before anything is taken, and here rather than in
/// the arm that does it: an operation stating its own cost is an operation that
/// can be added without one, and the length is only any use to somebody who has
/// not yet decided.
///
/// # Errors
///
/// Returns the [`Problem`] a surface should render when the command could not
/// be carried out, and the [`Problem`] that says so where a rehearsal was asked
/// for and this command cannot give one.
pub async fn dispatch(command: Command, ctx: &Ctx) -> Result<Outcome, Box<Problem>> {
    rehearsal::permitted(&command, ctx)?;
    // What a failed restart left, said before the answer to whatever was actually
    // asked for — and once, however many commands follow it. A boot that failed at
    // four in the morning has nobody to tell, so the only moment it can reach the
    // operator is the next one they are present at, and that is any command at all
    // rather than a particular one. The run a login starts is excluded: it is the
    // thing being reported on, not somebody arriving to be told.
    if !matches!(command, Command::AtBoot) {
        boot::reported(ctx).await;
    }
    // Last, so that the sentence nearest the acting is the one about what acting
    // costs. The line above is news from a run nobody saw; this one is about the
    // run the operator is in the middle of asking for.
    disturbance::said(&command, ctx).await;
    routed(rehearsal::carried(command, ctx), ctx).await
}

/// What this product's own vocabulary answers: one word, or all of them.
///
/// Two arms of the table folded into one call, the way asking for one setting and
/// asking for every setting already are. They are one question read two ways — a
/// reader who does not know a word and a reader who does not know which words there
/// are — and the answer to both comes out of the same table.
fn worded(word: Option<&str>) -> Result<Outcome, Box<Problem>> {
    let Some(word) = word else {
        return Ok(Outcome::Glossary(crate::glossary::vocabulary()));
    };
    crate::glossary::explain(word)
        .map(|term| Outcome::Word(*term))
        .ok_or_else(|| Box::new(crate::glossary::unrecognised(word)))
}

/// Held open until the location is lost, which is what a guard is.
///
/// Beside the table rather than in it because this is the one command whose arm reached
/// into the context for something the caller never named. The interval and the volume
/// are this command's own: a surface that could choose either could choose one that
/// misses the moment the command exists for.
async fn watching(ctx: &Ctx, forms: &[String]) -> Result<Outcome, Box<Problem>> {
    watch::supervise(ctx, ctx.volume.as_ref(), forms, WATCH)
        .await
        .map(Outcome::Watch)
}

/// Held on until the arrangement changes under it, sweeping as it goes.
///
/// Beside the table for the reason [`watching`] is: this arm reached for something
/// the caller never named. How often it wakes is this command's own, because a
/// period in whole days has no moment to miss — and a surface that could choose it
/// could choose one that misses the day.
///
/// It is also the one command here that acts while nobody is watching, which is why
/// its period is named rather than defaulted: naming one records it and stops.
async fn sweeping(ctx: &Ctx, arranged: Arranged) -> Result<Outcome, Box<Problem>> {
    expiring::expiring(ctx, arranged, expiring::SWEEPING)
        .await
        .map(Outcome::Household)
}

/// The table itself: every command, and where it goes.
///
/// Split from [`dispatch`] so that the three things asked of every command are asked
/// once, above the table, rather than in an arm somebody can add without adding:
/// whether a rehearsal means anything here, whether news is waiting from a run
/// nobody saw, and what this is about to take away. Private, so this is reachable
/// only through all three.
async fn routed(command: Command, ctx: &Ctx) -> Result<Outcome, Box<Problem>> {
    match command {
        Command::Version => engine::version(ctx).await.map(Outcome::Version),
        Command::Forms => engine::forms(ctx).map(Outcome::Forms),
        Command::Preview { forms } => engine::preview(ctx, &forms).map(Outcome::Preview),
        Command::Up { forms } => engine::lifecycle(ctx, &forms, &Action::Up).await,
        Command::AtBoot => boot::at_boot(ctx).await,
        Command::Start { forms, services } => acting(ctx, &forms, Action::Start(services)).await,
        Command::Down { forms, wait } => engine::teardown(ctx, &forms, wait).await,
        Command::Halt { forms, services } => acting(ctx, &forms, Action::Stop(services)).await,
        Command::Switch { forms } => engine::switch(ctx, &forms).await,
        Command::Restart { forms, services } => {
            acting(ctx, &forms, Action::Restart(services)).await
        }
        Command::Pull { forms } => engine::lifecycle(ctx, &forms, &Action::Pull).await,
        Command::ConfigGet { key } => configuring::reading(ctx, Some(&key)).await,
        Command::ConfigSet(change) => configuring::configuration(ctx, change).await,
        Command::ConfigShow => configuring::reading(ctx, None).await,
        Command::Quality(action) => quality::quality(ctx, action).map(Outcome::Quality),
        Command::Alerts(action) => appetite::hearing(ctx, action),
        Command::History => Ok(Outcome::History(history::history(ctx))),
        Command::Migrate(action) => migration::migrating(ctx, action).await,
        Command::QualityMusic { format } => music::music(ctx, format).await.map(Outcome::Music),
        Command::Trace {
            term,
            season,
            searching,
        } => traced(ctx, term, season, searching).await,
        Command::Held { member, most } => held::held(ctx, &member, most).await.map(Outcome::Held),
        Command::Household { member } => household::household(ctx, member.as_deref())
            .await
            .map(Outcome::Household),
        // Both answer with the household as it now stands rather than with a report of
        // their own, the way a forget answers with what is left: what an operator wants
        // to see after changing a limit is the limit, on the people it applies to.
        Command::Allowing(chosen) => asking::allowing(ctx, &chosen).await.map(Outcome::Household),
        Command::Deciding(decision) => decided(ctx, decision).await,
        Command::Expiring(arranged) => sweeping(ctx, arranged).await,
        // The short command that decides what becomes of the long ones. It reads
        // after it writes rather than reporting what a write claimed, because a written
        // definition is not a running command and this exists to tell the two apart.
        Command::Hosting(asked) => hosting::hosting(ctx, asked).await.map(Outcome::Hosting),
        Command::FrontDoor => door::front_door(ctx).await.map(Outcome::FrontDoor),
        Command::Stuck => trace::stuck(ctx).await.map(Outcome::Stuck),
        Command::Explain { word } => worded(Some(&word)),
        Command::Glossary => worded(None),
        Command::Clients => Ok(Outcome::Clients(crate::clients::guidance(
            quality::straining(ctx),
        ))),
        Command::Invite { name, allowance } => invited(ctx, name, allowance).await,
        Command::Reissue { name } => invite::reissued(ctx, name).await.map(Outcome::Invited),
        Command::Remove { name, confirm } => remove::dispatched(ctx, name, confirm).await,
        Command::Catalogue => engine::catalogue(ctx).map(Outcome::Catalogue),
        Command::Wiring(asked) => wiring::dispatched(ctx, &asked),
        Command::Outbound => outbound(ctx),
        Command::Provenance => engine::provenance(ctx).map(Outcome::Provenance),
        Command::QualityUpgrade { confirm } => {
            upgrade::upgrade(ctx, confirm).await.map(Outcome::Upgrade)
        }
        Command::Ps { forms } => engine::status(ctx, &forms).await.map(Outcome::Status),
        Command::Doctor {
            narrowing,
            disruptive,
            accept,
        } => diagnosed(ctx, narrowing, disruptive, accept).await,
        Command::Repair {
            consent,
            disruptive,
        } => mended(ctx, &consent, disruptive).await,
        Command::Undo { run } => putting_back::undo(ctx, run).await,
        Command::Credentials(asked) => credentials::answer(ctx, asked).await,
        Command::Stored => stored::listing(ctx).map(Outcome::Stored),
        Command::Plugins(action) => plugins::asked(ctx, &action).await,
        // The one read here that cannot fail, and the requirement is that it cannot:
        // an availability check another command could be blocked by would be one this
        // product had made a precondition of itself.
        Command::SelfUpdate { to } => Ok(Outcome::SelfUpdate(stands(ctx, to.as_deref()).await)),
        // The one write here, and it is the same answer twice: unconfirmed it lists
        // what would go, confirmed it goes.
        Command::Forget { confirm } => stored::forgetting(ctx, confirm).await.map(Outcome::Stored),
        // The same shape, over the operator's own disk rather than over lemonfiber's
        // files: unconfirmed it accounts and offers, confirmed it takes what the
        // account named as costing nothing.
        Command::Space { confirm } => space::space(ctx, confirm).await.map(Outcome::Space),
        // The one download the account leaves with the operator, asked for by name and
        // answered by the offer's own name. Apart from the account rather than an
        // argument to it, because a yes to reclaiming what costs nothing is not a yes
        // to losing a ratio a tracker keeps somebody's account on.
        Command::StopSeeding {
            download,
            agreement,
        } => letting(ctx, download, agreement).await,
        // And the same shape over the line rather than the disk: asked nothing it
        // reads, asked for a limit it declares one and tells every client.
        Command::Bandwidth(asked) => bandwidth::bandwidth(ctx, &asked)
            .await
            .map(Outcome::Bandwidth),
        Command::Watch { forms } => watching(ctx, &forms).await,
        // Said onto whatever the surface is listening with, which is how a walk is
        // watched rather than read afterwards.
        Command::Walkthrough { item } => walked(ctx, item).await,
        Command::Seed => seed::seed(ctx, false).await.map(Outcome::Seed),
        Command::Adopt => seed::seed(ctx, true).await.map(Outcome::Seed),
        Command::Uninstall(asked) => uninstall::uninstalled(ctx, asked).await,
        Command::Reset { confirm } => reset::reset(ctx, confirm).await.map(Outcome::Reset),
        Command::Setup(action) => setup::setting_up(ctx, action).await.map(Outcome::Wizard),
        // Unconfirmed it says what moving onto this build's pins would change, and
        // touches nothing; confirmed it takes those steps behind a backup.
        Command::Update(asked) => update::update(ctx, asked).await.map(Outcome::Update),
        Command::Backup { service } => backup::run(ctx, service).await.map(Outcome::Backup),
        Command::Support {
            write,
            wanted,
            dest,
        } => bundled(ctx, wanted, write, dest).await,
        Command::Archives => archives::run(ctx).await.map(Outcome::Archives),
        Command::Restore {
            archive,
            repoint,
            consent,
        } => restored(ctx, archive, repoint, consent).await,
    }
}

#[cfg(test)]
mod tests;
