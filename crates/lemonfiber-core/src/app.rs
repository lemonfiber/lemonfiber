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
use crate::door::run as door;
use crate::error::codes::life::{
    ABSENT_THERE, ALREADY_WORKING, NEVER_SETTLED, REGISTRY_REFUSED, STILL_NEEDED,
};
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

pub mod accepted;
pub(crate) mod adopt;
pub mod answering;
pub mod appetite;
pub mod apply;
pub mod archives;
pub(crate) mod arrangement;
pub mod authoring;
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
pub(crate) mod gating;
pub(crate) mod handoff;
pub(crate) mod healing;
pub(crate) mod held;
pub(crate) mod history;
pub(crate) mod hosting;
mod household_commands;
pub(crate) mod import;
pub(crate) mod invite;
pub(crate) mod letting;
pub(crate) mod materialise;
pub mod members;
pub(crate) mod music;
pub(crate) mod outbox;
pub(crate) mod playing;
pub mod plugins;
pub(crate) mod preflight;
pub(crate) mod project;
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
pub(crate) mod showing;
pub mod support;
pub mod switched;
pub(crate) mod targets;
pub(crate) mod trace;
mod underneath;
pub mod unforwarded;
pub(crate) mod upgrade;
pub mod watch;
pub(crate) mod watcher;

pub use command::{
    AlertAction, Allowance, Answer, Arranged, Asking, BandwidthAsked, Chosen, Command, Decision,
    Diagnosing, Filling, Gathering, Hostable, Inviting, Keeping, LettingGo, Linking, MigrateAction,
    QualityAction, Queueing, Reclaiming, Removing, Restarting, Restoring, Setting, Settling,
    Teardown, Tracing, Viewing, Whom, Wishing, HOSTABLE,
};
pub(crate) mod outcome;
pub use answering::answered_under;
pub use ctx::{Ctx, PATIENCE};
pub use outcome::Outcome;
pub use rehearsal::{asked, carried, permitted, Asked, Rehearsal};
pub use setup::SetupAction;

// The log-following reads a surface streams from live outside dispatch, so they are the
// engine module's functions re-exported for the binary and the log commands to reach.
pub use engine::{
    claimed, diagnose, in_flight, logs, offline, pull_progress, released, start_progress, started,
    unstarted, Claim, InstallOffered, Interrupted, Offline, Waiting, INSTALL_REFUSALS,
};
pub use notify::{notify, Notified, CHANNEL_CHECK};
pub use walkthrough::{walkthrough, worth_offering};
// Named at the import rather than at the arm: the module and the variant behind this
// one are together long enough that spelling it out there takes the arm past a line.
use self_update::standing as stands;

// The data-location watch is a self-contained feature in its own module; these
// are the names the rest of the crate and the binary reach it by.
pub use watch::{supervise, WATCH};

/// Every refusal a run on the stack answers with before it acts, named by its code: another
/// run holding it, a registry refusing it, a project the operator edited, a data location
/// the project cannot be written from, and a mount source reached through a link.
pub const REFUSALS: [Code; 5] = [
    ALREADY_WORKING,
    REGISTRY_REFUSED,
    crate::error::codes::stack::PROJECT_EDITED,
    crate::error::codes::storage::ROOT_REFUSED,
    crate::error::codes::life::MOUNT_LINKED,
];

/// Ask the engine to act on a set of services, which five commands do identically.
///
/// Named apart because they differ only in the action, and five arms that each spelled
/// out the call and the variant would leave `dispatch` with no room for a new command.
async fn lifecycle(ctx: &Ctx, forms: &[String], action: Action) -> Result<Outcome, Box<Problem>> {
    engine::lifecycle(ctx, forms, &action)
        .await
        .map(Outcome::from)
}

/// Upgrade existing content to the chosen preset, or state what that would cost.
///
/// Beside the table for the reason [`engine::restarted`] is.
async fn upgraded(ctx: &Ctx, confirm: bool) -> Result<Outcome, Box<Problem>> {
    upgrade::upgrade(ctx, confirm).await.map(Outcome::Upgrade)
}

/// Take the stack down, having waited for its downloads first where that was asked.
///
/// Beside the table because its row, spelled out, is longer than one line.
async fn down(ctx: &Ctx, forms: &[String], wait: Waiting) -> Result<Outcome, Box<Problem>> {
    engine::teardown(ctx, forms, wait).await.map(Outcome::from)
}

/// Which app to use on which device, with what the quality in force does to playback.
///
/// Beside the table because it has a step before the answer. One caution above the
/// devices is this machine's own: the quality preset on record and the platform decide
/// together whether playback here will be transcoded on the processor, which is the
/// likeliest cause of trouble on any of them. Both are read best-effort, so a machine with
/// nothing set up is answered in full and simply warned about nothing.
fn guided(ctx: &Ctx) -> Outcome {
    Outcome::Clients(crate::clients::guidance(quality::straining(ctx)))
}

/// Put back the last repair, or the run a stamp names.
///
/// Beside the table for the reason [`down`] is.
async fn undone(ctx: &Ctx, run: Option<&str>) -> Result<Outcome, Box<Problem>> {
    putting_back::undo(ctx, run).await.map(Outcome::Undo)
}

/// Read, reveal or rotate what the stack holds as credentials.
///
/// Beside the table for the reason [`down`] is.
async fn inventoried(ctx: &Ctx, asked: Asking) -> Result<Outcome, Box<Problem>> {
    credentials::credentials(ctx, asked)
        .await
        .map(Outcome::Credentials)
}

/// What the services themselves reach out for, which needs the stack read first.
///
/// Named apart for the reason [`lifecycle`] is: every other row here passes what it was
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
        &crate::outbound::Noted {
            covers: crate::outbound::covers::recorded(ctx),
            backends: crate::notify::backends::destinations(ctx),
        },
    )))
}

/// A diagnosis, and the warning it was told to consider answered.
///
/// Named apart for the reason [`lifecycle`] is: the run happens first and what it found
/// is then read against what was accepted, which is two steps rather than a
/// pass-along.
async fn diagnosed(ctx: &Ctx, asked: &Diagnosing) -> Result<Outcome, Box<Problem>> {
    let report = engine::diagnose(ctx, &asked.narrowing, asked.disruptive).await?;
    let mut report = accepted::acknowledge(ctx, asked.accept.as_deref(), report)?;
    // Dated after an acceptance is applied, so a warning the operator has just
    // accepted is not remembered as wrong.
    report.findings = conditions::dated(ctx, report.findings);
    Ok(Outcome::Doctor(report))
}

/// Where the disk stands, and what taking the offer came to where `asked` answers it.
async fn reclaimed(ctx: &Ctx, asked: Reclaiming) -> Result<Outcome, Box<Problem>> {
    space::space(ctx, asked.agreement, &asked.only)
        .await
        .map(Outcome::Space)
}

/// What letting one completed download go costs, and what became of letting it.
async fn letting(ctx: &Ctx, asked: LettingGo) -> Result<Outcome, Box<Problem>> {
    letting::stop_seeding(ctx, asked.download, asked.agreement)
        .await
        .map(Outcome::StopSeeding)
}

/// A walk through the stack, said onto whatever the surface is listening with.
async fn walked(ctx: &Ctx, item: Option<String>) -> Result<Outcome, Box<Problem>> {
    walkthrough::walkthrough(ctx, item.as_deref(), ctx.steps.as_ref())
        .await
        .map(Outcome::Walkthrough)
}

/// What a trace follows, and whether it reaches past this machine to find it.
async fn traced(ctx: &Ctx, tracing: Tracing) -> Result<Outcome, Box<Problem>> {
    let Tracing {
        term,
        season,
        searching,
    } = tracing;
    trace::trace(ctx, &term, season, searching)
        .await
        .map(Outcome::Trace)
}

/// The bundle a support request asks for, gathered and put where it was asked for.
async fn bundled(ctx: &Ctx, asked: Gathering) -> Result<Outcome, Box<Problem>> {
    support::run(ctx, &asked.wanted, asked.write, &asked.dest)
        .await
        .map(Outcome::Bundle)
}

/// What an archive would put back, or the putting back of it.
async fn restored(ctx: &Ctx, restoring: Restoring) -> Result<Outcome, Box<Problem>> {
    let Restoring {
        archive,
        repoint,
        consent,
    } = restoring;
    restore::run(ctx, &archive, repoint, &consent)
        .await
        .map(Outcome::Restore)
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
    if !matches!(command, Command::Authoring(_)) {
        stack_first(&command, ctx).await;
        // Last, so that the sentence nearest the acting is the one about what acting
        // costs: what a failed restart left is news from a run nobody saw; this is
        // about the run the operator is in the middle of asking for.
        disturbance::said(&command, ctx).await;
    }
    Box::pin(routed(rehearsal::carried(command, ctx), ctx))
        .await
        .map(|outcome| {
            if ctx.dry_run {
                outcome.said_as_a_rehearsal()
            } else {
                outcome
            }
        })
}

/// What every command that reaches a stack settles before it is routed.
async fn stack_first(command: &Command, ctx: &Ctx) {
    // What a failed restart left, said before the answer to whatever was actually
    // asked for — and once, however many commands follow it. A boot that failed at
    // four in the morning has nobody to tell, so the only moment it can reach the
    // operator is the next one they are present at, and that is any command at all
    // rather than a particular one. The run a login starts is excluded: it is the
    // thing being reported on, not somebody arriving to be told.
    if !matches!(command, Command::AtBoot) {
        boot::reported(ctx).await;
    }
    crate::settings::filing::everything(ctx).await;
    // Before anything signs in with a credential lemonfiber minted: a rotation that
    // stopped part-way left its replacement beside it, and which of the two the
    // service takes is settled here so every command after it signs in with that one.
    credentials::pending::settled(ctx).await;
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
    watch::supervise(ctx, ctx.seams.volume.as_ref(), forms, WATCH)
        .await
        .map(Outcome::Watch)
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
        Command::Preview { forms } => engine::preview(ctx, &forms).await.map(Outcome::Preview),
        Command::Up { forms, offer } => engine::up(ctx, &forms, offer).await,
        Command::AtBoot => boot::at_boot(ctx).await.map(Outcome::from),
        Command::Start { forms, services } => lifecycle(ctx, &forms, Action::Start(services)).await,
        Command::Down(Teardown { forms, wait }) => down(ctx, &forms, wait).await,
        Command::Halt { forms, services } => lifecycle(ctx, &forms, Action::Stop(services)).await,
        Command::Switch { forms, offer } => engine::switched(ctx, &forms, offer).await,
        Command::Restart(restart) => engine::restarted(ctx, restart).await,
        Command::Pull { forms } => lifecycle(ctx, &forms, Action::Pull(Vec::new())).await,
        Command::ConfigGet { key } => configuring::get(ctx, Some(&key)).await.map(Outcome::Config),
        Command::ConfigSet(change) => configuring::set(ctx, change).await.map(Outcome::Config),
        Command::ConfigShow => configuring::get(ctx, None).await.map(Outcome::Config),
        Command::Quality(action) => quality::quality(ctx, action).await.map(Outcome::Quality),
        Command::Alerts(action) => appetite::alerts(ctx, action).map(Outcome::Alerts),
        Command::History => history::history(ctx).map(Outcome::History),
        Command::Migrate(action) => migration::migrate(ctx, action).await,
        Command::QualityMusic { format } => music::music(ctx, format).await.map(Outcome::Music),
        Command::Trace(tracing) => traced(ctx, tracing).await,
        Command::Held { member, most } => held::held(ctx, &member, most).await.map(Outcome::Held),
        Command::Playing { member } => playing::asked(ctx, member.as_deref()).await,
        Command::Viewing(viewing) => crate::screening::viewed(ctx, viewing).await,
        Command::Wish(wishing) => crate::wishing::wished(ctx, wishing).await,
        Command::Household { member } => household_commands::household(ctx, member.as_ref()).await,
        // Both answer with the household as it now stands rather than with a report of
        // their own, the way a forget answers with what is left: what an operator wants
        // to see after changing a limit is the limit, on the people it applies to.
        Command::Allowing(chosen) => asking::allowing(ctx, &chosen).await.map(Outcome::Household),
        Command::Deciding(decision) => household_commands::decided(ctx, decision).await,
        Command::Expiring(arranged) => household_commands::sweeping(ctx, arranged).await,
        // The short command that decides what becomes of the long ones. It reads
        // after it writes rather than reporting what a write claimed, because a written
        // definition is not a running command and this exists to tell the two apart.
        Command::Hosting(asked) => hosting::hosting(ctx, asked).await.map(Outcome::Hosting),
        Command::FrontDoor => door::front_door(ctx).await.map(Outcome::FrontDoor),
        Command::Stack => underneath::stack(ctx).await.map(Outcome::Stack),
        // A read that cannot fail: a kind that could not be read is named in the
        // answer rather than failing the rest.
        Command::News => Ok(Outcome::News(crate::news::run::news(ctx).await)),
        Command::Companion(asked) => crate::companion::asked(ctx, asked).await,
        Command::Stuck => trace::stuck(ctx).await.map(Outcome::Stuck),
        Command::Boot => boot::booted(ctx).await.map(Outcome::Boot),
        Command::Watcher => watcher::watcher(ctx).await.map(Outcome::Watcher),
        Command::Queue(Queueing::Read) => healing::queue(ctx).map(Outcome::Queue),
        Command::Queue(Queueing::Heal) => healing::heal(ctx).await.map(Outcome::Queue),
        Command::Explain { word } => worded(Some(&word)),
        Command::Glossary => worded(None),
        Command::Clients => Ok(guided(ctx)),
        Command::Invite(inviting) => household_commands::invited(ctx, inviting).await,
        Command::Reissue { name } => invite::reissue(ctx, name).await.map(Outcome::Invitation),
        Command::Handoff { name } => handoff::handoff(ctx, name).await.map(Outcome::Handoff),
        Command::Remove { name, agreement } => {
            household_commands::removed(ctx, name, agreement).await
        }
        Command::Catalogue => engine::catalogue(ctx).map(Outcome::Catalogue),
        Command::Wiring(asked) => wiring::wiring(ctx, &asked).await,
        Command::Settings(asked) => crate::settings::run::settings(ctx, &asked).await,
        Command::Outbound => outbound(ctx),
        Command::Provenance => engine::provenance(ctx).map(Outcome::Provenance),
        Command::QualityUpgrade { confirm } => upgraded(ctx, confirm).await,
        Command::Status { forms } => engine::status(ctx, &forms).await.map(Outcome::Status),
        Command::Doctor(asked) => diagnosed(ctx, &asked).await,
        Command::Repair {
            consent,
            disruptive,
        } => mended(ctx, &consent, disruptive).await,
        Command::Undo { run } => undone(ctx, run.as_deref()).await,
        Command::Credentials(asked) => inventoried(ctx, asked).await,
        Command::Stored => stored::listing(ctx).map(Outcome::Stored),
        Command::Plugins(action) => plugins::plugins(ctx, &action).await.map(Outcome::Plugins),
        Command::Authoring(asked) => authoring::answered(ctx, asked).await,
        // The one read here that cannot fail, and the requirement is that it cannot:
        // an availability check another command could be blocked by would be one this
        // product had made a precondition of itself.
        Command::SelfUpdate { to } => Ok(Outcome::SelfUpdate(stands(ctx, to.as_deref()).await)),
        // The one write here, and it is the same answer twice: unconfirmed it lists
        // what would go, confirmed it goes.
        Command::Forget { confirm } => stored::forget(ctx, confirm).await.map(Outcome::Stored),
        // The same shape, over the operator's own disk: unconfirmed it accounts and offers,
        // confirmed it takes what the account named as costing nothing.
        Command::Space(asked) => reclaimed(ctx, asked).await,
        // The one download the account leaves with the operator, asked for by name and
        // answered by the offer's own name. Apart from the account rather than an
        // argument to it, because a yes to reclaiming what costs nothing is not a yes
        // to losing a ratio a tracker keeps somebody's account on.
        Command::StopSeeding(asked) => letting(ctx, asked).await,
        // And the same shape over the line rather than the disk: asked nothing it
        // reads, asked for a limit it declares one and tells every client.
        Command::Bandwidth(asked) => bandwidth::shared(ctx, &asked).await,
        Command::Downloads { asked, offer } => bandwidth::pausing::paused(ctx, asked, offer).await,
        Command::Keys(asked) => crate::keys::run::asked(ctx, asked).await,
        Command::DeviceKeys(asked) => crate::device_keys::run::asked(ctx, asked).await,
        Command::PushEndpoints(asked) => crate::push::run::asked(ctx, asked),
        Command::Notify(asked) => crate::notify::configuring::asked(ctx, asked).await,
        Command::Watch { forms } => watching(ctx, &forms).await,
        // Said onto whatever the surface is listening with, which is how a walk is
        // watched rather than read afterwards.
        Command::Walkthrough { item } => walked(ctx, item).await,
        Command::Seed => seed::seed(ctx, false).await.map(Outcome::Seed),
        Command::Adopt => seed::seed(ctx, true).await.map(Outcome::Seed),
        Command::Uninstall(how) => uninstall::uninstall(ctx, how).await.map(Outcome::Uninstall),
        Command::Reset { agreement } => reset::reset(ctx, agreement).await.map(Outcome::Reset),
        Command::Setup(action) => setup::setup(ctx, action).await.map(Outcome::Wizard),
        // Unconfirmed it says what moving onto this build's pins would change, and
        // touches nothing; confirmed it takes those steps behind a backup.
        Command::Update(asked) => update::update(ctx, asked).await.map(Outcome::Update),
        Command::Backup { service } => backup::backup(ctx, service).await.map(Outcome::Backup),
        Command::Support(asked) => bundled(ctx, asked).await,
        Command::Archives => archives::archives(ctx).await.map(Outcome::Archives),
        Command::Restore(restoring) => restored(ctx, restoring).await,
    }
}

#[cfg(test)]
mod tests;
