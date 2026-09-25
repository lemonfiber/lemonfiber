//! What lemonfiber does when this machine has just started, and what it says about
//! it afterwards.
//!
//! Docker's restart policies bring *containers* back, and only once the engine
//! behind them is running. Everything else about a restart was nobody's: which form
//! should come back, whether the operator had deliberately stopped it, whether the
//! network had arrived yet, whether the tunnel re-established on the same forwarded
//! port — and, when any of that went wrong, whether anybody would ever find out.
//! This is the run that answers all of it, and the sentence that reaches the
//! operator afterwards.
//!
//! **It is a start with four questions in front of it and two behind it.** In front:
//! has this machine actually restarted since the stack was last brought back, was
//! autostart asked for, was the stack stopped on purpose, and is this a laptop on its
//! battery. Behind: did the stack settle, and did the tunnel and its forwarded port
//! come back with it. Only the middle step runs Compose, and it is the ordinary start
//! every surface runs — a second way of starting the stack would be a second thing to
//! keep true, and the one nobody watches is the one that stops being true.
//!
//! **Nothing here is a new way of failing.** A boot that could not start the stack
//! raises one condition under one check, which is what makes ten failed boots one
//! ongoing problem rather than ten notifications; and the reason is said at the next
//! thing the operator types rather than into a log file they will not read, because
//! the whole failure mode this feature exists for is *not noticing*.

use std::collections::BTreeSet;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::alert::{Alert, Moment};
use crate::condition::{Condition, Fault};
use crate::doctor::{Category, Narrowing, Verdict};
use crate::error::{Problem, Severity};
use crate::model::LifecycleReport;
use crate::ports::machine::Power;
use crate::stack::closure::Plan;
use crate::stack::compose::Action;

use super::{Ctx, Outcome};

/// The check a boot that did not bring the stack back is filed under.
///
/// One name for every way it can go wrong, and that is the whole point: a condition
/// is keyed by check, so a machine that has failed to come back after nine restarts
/// holds one condition raised nine days ago rather than nine of them — which is the
/// difference between telling the operator something and training them to ignore it.
pub const CHECK: &str = "boot.start";

/// The kind of event it is, shared by every instance of it.
const KIND: &str = "boot.failed";

/// What a boot run is called in the report it produces.
///
/// Not a Compose verb, and the field it lands in has carried one before: what an
/// operator wants at the top of this report is which run it was, and "up" would make
/// a login indistinguishable from something they typed.
const BOOT: &str = "boot";

/// How many times a start at a login is tried before it is reported as failed.
///
/// A wired host can be ready to run things before its address has arrived, which is
/// the failure this budget exists for — and the honest answer to it is to try again
/// rather than to probe for a network, because what "the network is up" means to the
/// stack is whether its own gateway can reach its provider, and nothing here can
/// establish that without starting it.
const TRIES: u32 = 5;

/// How long it leaves between tries.
///
/// Half a minute rather than the hundreds of milliseconds the retrying transport
/// uses. That one is budgeted for a request that met a busy server; this is budgeted
/// for an address that has not been handed out yet, and the two are minutes apart.
const BETWEEN: Duration = Duration::from_secs(30);

/// How many times it asks the container engine whether it is there yet.
///
/// Docker Desktop takes the better part of a minute to come up after a login, so a
/// boot run that asked once would report every Mac in the world as having failed.
const LOOKS: u32 = 60;

/// How long it leaves between those.
const AGAIN: Duration = Duration::from_secs(2);

/// Which restart of this machine has already been acted on.
///
/// So a login that fires twice, a service manager that restarts its agent, and an
/// operator running the command by hand do not each re-verify and re-report the same
/// boot. Written after the run rather than before it, so a run that was killed
/// part-way is tried again rather than counted as done.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Acted {
    /// The moment this machine started, as the run that acted on it read it.
    #[serde(default)]
    at: Option<u64>,
}

/// The record's file name, named once so the layout and the readers agree.
const RECORD: &str = "boot.json";

/// Bring back what this machine was running, if anything should come back at all.
///
/// # Errors
///
/// Returns the [`Problem`] a surface should render where the start itself could not
/// be carried out — a stack that cannot be read, a data location that never appeared,
/// services that never settled. A run that declines to start anything is not an
/// error: declining is the correct answer to three of the four questions in front of
/// it, and the report says which.
pub(crate) async fn at_boot(ctx: &Ctx) -> Result<Outcome, Box<Problem>> {
    waited(ctx, LOOKS, AGAIN, TRIES, BETWEEN).await
}

/// The same run with its waiting spelled out, so a test can drive it without sitting
/// through the minutes a real login takes.
async fn waited(
    ctx: &Ctx,
    looks: u32,
    again: Duration,
    tries: u32,
    between: Duration,
) -> Result<Outcome, Box<Problem>> {
    let forms = match asked_for(ctx).await {
        Ok(forms) => forms,
        Err(held) => return Ok(Outcome::Lifecycle(nothing_started(&held))),
    };

    // The engine first, because on two of the four platforms it is a desktop
    // application that has only just been asked to start itself. Nothing is written
    // down as acted on here: an engine that never came up is a boot worth trying
    // again, and the operator running anything at all is when it gets tried.
    if !reachable(ctx, looks, again).await {
        raised(ctx, ENGINE_NEVER_CAME, NOTHING_CAME_BACK);
        return Ok(Outcome::Lifecycle(nothing_started(ENGINE_NEVER_CAME)));
    }

    let outcome = tried(ctx, &forms, tries, between).await;
    acted(ctx).await;
    match outcome {
        Err(problem) => {
            raised(ctx, &problem.summary, NOTHING_CAME_BACK);
            Err(problem)
        }
        Ok(outcome) if !came_up(&outcome) => {
            raised(ctx, COMPOSE_REFUSED, NOTHING_CAME_BACK);
            Ok(outcome)
        }
        Ok(outcome) => {
            confirmed(ctx).await;
            Ok(outcome)
        }
    }
}

/// What to say where the engine never turned up.
const ENGINE_NEVER_CAME: &str = "the container engine never answered, so nothing could be started";

/// What to say where Compose ran and would not bring the stack up.
const COMPOSE_REFUSED: &str = "the stack would not start";

/// What to say where this restart has already been dealt with.
const ALREADY_DONE: &str = "this machine has not restarted since the stack was last brought back";

/// What an operator on a battery is told, and what it takes to change it.
///
/// The setting is interpolated rather than written out, the way every other sentence
/// that names one does it: a name spelled in two places is a name that stops matching
/// when one of them is renamed, and the operator is the one who finds out.
fn on_battery_said() -> String {
    format!(
        "this machine is running on its battery, and a media stack started on one empties it \
         in an afternoon — set {} to on if you would rather it started anyway",
        crate::config::AUTOSTART_ON_BATTERY_KEY
    )
}

/// Which forms should come back, or why none should.
///
/// Four questions, cheapest first rather than most important first: reading a record
/// costs nothing and asking about the battery costs a process, so a machine that was
/// never asked to start anything never asks its battery about it.
async fn asked_for(ctx: &Ctx) -> Result<Vec<String>, String> {
    if already(ctx).await {
        return Err(ALREADY_DONE.to_owned());
    }
    let returning = super::autostart::load(ctx);
    let forms = match returning.at_boot() {
        Ok(forms) => forms.to_vec(),
        Err(held) => return Err(held.said().to_owned()),
    };
    if on_battery(ctx).await {
        return Err(on_battery_said());
    }
    Ok(forms)
}

/// Whether this run has already acted on this restart.
///
/// A machine that will not say when it started reads as not yet acted on: a second
/// start is a Compose command that changes nothing, and the alternative — declining
/// because the moment could not be read — would be a machine that never comes back.
async fn already(ctx: &Ctx) -> bool {
    let Some(at) = ctx.started.at().await else {
        return false;
    };
    super::record::beside::<Acted>(ctx, RECORD).at == Some(at)
}

/// Write down which restart this run acted on.
///
/// A rehearsal writes nothing, here and in the two below it. What it is being asked
/// is what a login would come to, and a rehearsal that recorded the boot as acted on
/// would make the real run that followed it decline — which is the one way a
/// rehearsal of this could change the thing it was rehearsing.
async fn acted(ctx: &Ctx) {
    if ctx.dry_run {
        return;
    }
    let at = ctx.started.at().await;
    super::record::keep_beside(ctx, RECORD, &Acted { at });
}

/// Whether this machine is on its battery and nobody said to start anyway.
///
/// A machine that will not say where its power comes from is not a machine on a
/// battery. A desktop has no power report and no mains adapter to read, and holding
/// a media server's stack back on every restart because a file was missing would be
/// the expensive way round of the two.
async fn on_battery(ctx: &Ctx) -> bool {
    !ctx.settings.autostart_on_battery && matches!(ctx.power.source().await, Some(Power::Battery))
}

/// Wait until the container engine answers, or give up.
///
/// Asked through the ordinary listing rather than through a probe of its own: what
/// this needs to know is whether the thing every later step talks to is talking, and
/// a second way of asking would be a second answer to disagree with.
async fn reachable(ctx: &Ctx, looks: u32, again: Duration) -> bool {
    if ctx.seams.engine.list(&ctx.settings.project).await.is_ok() {
        return true;
    }
    ctx.narrator
        .say("waiting for the container engine to start")
        .await;
    for _ in 0..looks {
        tokio::time::sleep(again).await;
        if ctx.seams.engine.list(&ctx.settings.project).await.is_ok() {
            return true;
        }
    }
    false
}

/// Start the forms, trying again while there is budget left.
///
/// The ordinary start, run again rather than a start with a retry built into it: what
/// fails at a login fails for reasons that clear on their own — an address that has
/// not arrived, a mount still coming up — and what does not clear on its own fails
/// the same way five times and is reported.
async fn tried(
    ctx: &Ctx,
    forms: &[String],
    tries: u32,
    between: Duration,
) -> Result<Outcome, Box<Problem>> {
    let mut outcome = super::engine::lifecycle(ctx, forms, &Action::Up).await;
    for attempt in 1..tries {
        if outcome.as_ref().is_ok_and(came_up) {
            return outcome;
        }
        ctx.narrator
            .say(&format!(
                "the stack did not start; trying again ({attempt} of {})",
                tries.saturating_sub(1)
            ))
            .await;
        tokio::time::sleep(between).await;
        outcome = super::engine::lifecycle(ctx, forms, &Action::Up).await;
    }
    outcome
}

/// Whether an attempt brought the stack up.
fn came_up(outcome: &Outcome) -> bool {
    matches!(outcome, Outcome::Lifecycle(report) if report.status == Some(0))
}

/// Confirm the stack actually came back, tunnel and forwarded port included.
///
/// The start has already waited for the services to be usable and reconciled the
/// forwarded port, which is the half a running stack can prove for itself. This is
/// the other half: a tunnel that came back on a different address, a killswitch that
/// is not holding, a client that could not be moved onto the port the provider grants
/// now. Everything it asks is a check that already existed and that nothing asked
/// after a restart.
async fn confirmed(ctx: &Ctx) {
    match wrong(ctx).await {
        None => observed(ctx, None),
        Some(said) => raised(ctx, &said, TUNNEL_UNPROVEN),
    }
}

/// What the trust checks found wrong after the stack came back, if anything.
///
/// Narrowed to the tunnel, because that is the part of a restart that does not look
/// after itself: containers come back under their own restart policies, and a tunnel
/// comes back with a new address and, commonly, a different forwarded port. An
/// unreadable stack leaves this unanswered rather than wrong — a diagnosis that could
/// not run has established nothing, and reporting that as a failed boot would be the
/// comfortable falsehood.
async fn wrong(ctx: &Ctx) -> Option<String> {
    let report = super::engine::diagnose(ctx, &Narrowing::Category(Category::Vpn), false)
        .await
        .ok()?;
    let named: Vec<&str> = report
        .findings
        .iter()
        .filter(|finding| matches!(finding.verdict, Verdict::Fail(_) | Verdict::Warn(_)))
        .map(|finding| finding.check.as_str())
        .collect();
    if named.is_empty() {
        return None;
    }
    Some(format!(
        "the stack came back and its tunnel did not: {}",
        named.join(", ")
    ))
}

/// Record that this boot did not come back, with the reason and what it costs.
///
/// The cost is passed in rather than fixed here: a stack that never started and one
/// that started without a proven tunnel are the same check and not the same loss,
/// and one sentence covering both would be true of neither.
fn raised(ctx: &Ctx, said: &str, means: &str) {
    let fault = Fault::new(
        KIND,
        Severity::Warning,
        said,
        means,
        "Run `lemonfiber up` to bring it back, then `lemonfiber doctor` to see what stopped it",
    );
    observed(ctx, Some(&fault));
}

/// What a machine that restarted without its stack costs the operator.
const NOTHING_CAME_BACK: &str =
    "nothing has been running since this machine started — no downloads, no imports, and \
     nothing to watch";

/// What a stack that came back without a proven tunnel costs.
const TUNNEL_UNPROVEN: &str =
    "the stack is back but its tunnel is not established, so the download client's traffic \
     cannot be assumed to be protected";

/// Put what this boot came to into the store the next interaction reads.
///
/// The store keeps `since` untouched while a fault stays raised and counts a
/// recurrence only when one returns, so a machine that has failed to come back nine
/// times running holds one condition rather than nine — which is the whole of what
/// "report a recurring condition once" needs, and the reason this writes a condition
/// rather than a line somewhere.
fn observed(ctx: &Ctx, fault: Option<&Fault>) {
    if ctx.dry_run {
        return;
    }
    let mut conditions = super::conditions::load(ctx);
    conditions.observe(CHECK, fault, &ctx.stamp());
    super::conditions::save(ctx, &conditions);
}

/// A report for a run that started nothing, and the reason it did not.
fn nothing_started(reason: &str) -> LifecycleReport {
    LifecycleReport {
        action: BOOT.to_owned(),
        plan: Plan {
            forms: Vec::new(),
            profiles: BTreeSet::new(),
            services: Vec::new(),
            dropped: Vec::new(),
            filtered: Vec::new(),
            footprint: crate::stack::closure::Footprint::default(),
        },
        command: Vec::new(),
        rehearsed: false,
        status: None,
        services: Vec::new(),
        condition: None,
        stack_edits: Vec::new(),
        // Nothing was about to bind, so nothing asked the machine who holds a port.
        // Empty here means the question was never put, which is the same shape a
        // teardown reports and reads the same way: no clash was found.
        port_conflicts: Vec::new(),
        forwarding: None,
        switched: None,
        held: Some(reason.to_owned()),
    }
}

/// Say what the last boot left, once, at the next thing the operator does.
///
/// The other half of the requirement, and the half a log file cannot do: a boot that
/// failed at four in the morning is a thing nobody was there for, so the only moment
/// it can reach them is the next one they are present at. Said through the narrator
/// rather than folded into the answer of whatever they typed, because it is not about
/// what they asked for — and every surface has one, so all three say it.
///
/// Once. The outbox records which spell of this fault the operator has been told
/// about, so a machine that has been failing to come back for a fortnight says so at
/// the next interaction and then stops, rather than prefixing every command they type
/// for a fortnight.
pub(crate) async fn reported(ctx: &Ctx) {
    // A rehearsal changes nothing, and marking an alert delivered is a change.
    if ctx.dry_run {
        return;
    }
    let conditions = super::conditions::load(ctx);
    let Some(condition) = conditions.get(CHECK) else {
        return;
    };
    let mut outbox = super::outbox::load(ctx);
    if !condition.is_worth_saying(outbox.told(CHECK)) {
        return;
    }
    outbox.owe(vec![owed(condition)]);
    ctx.narrator.say(&said(condition)).await;
    // Delivered because the narrator cannot fail: whatever else happens, the operator
    // has the sentence and the outbox has the history of its having been given.
    outbox.delivered(&|_| condition.recurrences);
    super::outbox::save(ctx, &outbox);
}

/// The alert a standing boot failure amounts to.
fn owed(condition: &Condition) -> Alert {
    Alert {
        check: CHECK.to_owned(),
        kind: condition.kind.clone(),
        moment: Moment::Onset,
        severity: condition.severity,
        summary: condition.summary.clone(),
        meaning: condition.meaning.clone(),
        remedies: condition.remedies.clone(),
        affected: vec![CHECK.to_owned()],
    }
}

/// The sentence itself: what happened, and when it started happening.
///
/// The date is the point. "The stack did not come back" is a thing to look into this
/// morning; "the stack has not come back since the fourteenth" is a fortnight of
/// downloads that never happened, and the two lead to different afternoons.
fn said(condition: &Condition) -> String {
    format!(
        "at the last restart of this machine, {} (first seen {})",
        condition.summary, condition.since
    )
}

#[cfg(test)]
mod tests;
