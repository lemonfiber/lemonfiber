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
pub(super) async fn at_boot(ctx: &Ctx) -> Result<Outcome, Box<Problem>> {
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
        raised(ctx, ENGINE_NEVER_CAME);
        return Ok(Outcome::Lifecycle(nothing_started(ENGINE_NEVER_CAME)));
    }

    let outcome = tried(ctx, &forms, tries, between).await;
    acted(ctx).await;
    match outcome {
        Err(problem) => {
            raised(ctx, &problem.summary);
            Err(problem)
        }
        Ok(outcome) if !came_up(&outcome) => {
            raised(ctx, COMPOSE_REFUSED);
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
    if ctx.engine.list(&ctx.settings.project).await.is_ok() {
        return true;
    }
    ctx.narrator
        .say("waiting for the container engine to start")
        .await;
    for _ in 0..looks {
        tokio::time::sleep(again).await;
        if ctx.engine.list(&ctx.settings.project).await.is_ok() {
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
        Some(said) => raised(ctx, &said),
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

/// Record that this boot did not come back, with the reason.
fn raised(ctx: &Ctx, said: &str) {
    let fault = Fault::new(
        KIND,
        Severity::Warning,
        said,
        "Run `lemonfiber up` to bring it back, then `lemonfiber doctor` to see what stopped it",
    );
    observed(ctx, Some(&fault));
}

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
        },
        command: Vec::new(),
        rehearsed: false,
        status: None,
        services: Vec::new(),
        condition: None,
        stack_edits: Vec::new(),
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
pub(super) async fn reported(ctx: &Ctx) {
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
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use async_trait::async_trait;
    use tokio::sync::mpsc::Receiver;

    use super::{on_battery_said, reported, waited, ALREADY_DONE, CHECK};
    use crate::app::{Ctx, Outcome};
    use crate::autostart::Returning;
    use crate::condition::Fault;
    use crate::config::Settings;
    use crate::error::{Problem, Severity};
    use crate::ports::docker::{Container, Engine, ExecOutput, Failure, LogLine, LogQuery, Stats};
    use crate::ports::machine::{Power, Started, Supply};
    use crate::ports::Narrator;
    use crate::test_support::a_context;

    /// A machine that says when it started and where its power comes from, or will
    /// not say — both of which are facts no test could otherwise arrange.
    struct Machine {
        at: Option<u64>,
        power: Option<Power>,
    }

    #[async_trait]
    impl Started for Machine {
        async fn at(&self) -> Option<u64> {
            self.at
        }
    }

    #[async_trait]
    impl Supply for Machine {
        async fn source(&self) -> Option<Power> {
            self.power
        }
    }

    /// An engine that refuses the first `refusals` listings and answers after that.
    ///
    /// The shape of a desktop engine that has only just been asked to start itself,
    /// which is the case the wait exists for and the one the existing engine fixture
    /// cannot produce: that one is reachable or it is not, and never changes its mind.
    struct Waking {
        refusals: usize,
        asked: AtomicUsize,
    }

    impl Waking {
        /// An engine that answers straight away.
        fn awake() -> Arc<Self> {
            Self::after(0)
        }

        /// One that refuses this many listings first.
        fn after(refusals: usize) -> Arc<Self> {
            Arc::new(Self {
                refusals,
                asked: AtomicUsize::new(0),
            })
        }

        /// What every other method answers: this fixture is about one question.
        fn elsewhere() -> Failure {
            Failure::Unreachable {
                reason: "this fixture answers listings and nothing else".to_owned(),
            }
        }
    }

    #[async_trait]
    impl Engine for Waking {
        async fn list(&self, _project: &str) -> Result<Vec<Container>, Failure> {
            if self.asked.fetch_add(1, Ordering::SeqCst) < self.refusals {
                return Err(Failure::Unreachable {
                    reason: "the daemon is still starting".to_owned(),
                });
            }
            Ok(Vec::new())
        }

        async fn exec(&self, _container: &str, _argv: &[String]) -> Result<ExecOutput, Failure> {
            Err(Self::elsewhere())
        }

        async fn stats(&self, _project: &str) -> Result<Receiver<(String, Stats)>, Failure> {
            Err(Self::elsewhere())
        }

        async fn logs(
            &self,
            _project: &str,
            _services: &[String],
            _query: LogQuery,
        ) -> Result<Receiver<LogLine>, Failure> {
            Err(Self::elsewhere())
        }
    }

    /// A narrator that keeps what it was told, so a test can read it back.
    #[derive(Default)]
    struct Heard {
        lines: Mutex<Vec<String>>,
    }

    impl Heard {
        /// Everything said through it, in order.
        fn lines(&self) -> Vec<String> {
            self.lines
                .lock()
                .map(|held| held.clone())
                .unwrap_or_default()
        }
    }

    #[async_trait]
    impl Narrator for Heard {
        async fn say(&self, said: &str) {
            if let Ok(mut held) = self.lines.lock() {
                held.push(said.to_owned());
            }
        }
    }

    /// Where a test's scratch records live. Naming it does not touch it.
    fn scratch(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("lemonfiber-boot-{}-{name}", std::process::id()))
    }

    /// Settings keeping their records in an emptied scratch directory.
    fn settings_at(name: &str) -> Settings {
        let dir = scratch(name);
        let _ = std::fs::remove_dir_all(&dir);
        Settings {
            env_file: Some(dir.join(".env")),
            ..Settings::default()
        }
    }

    /// A context with the machine and the engine a test scripted.
    ///
    /// Compose is scripted to refuse, and patience is nothing at all. Every start
    /// here therefore fails, which is deliberate: what these tests are about is the
    /// four questions in front of the start and what is written down behind it, and a
    /// start that actually settled would need a stack of containers to settle.
    fn ctx_with(settings: Settings, machine: Machine, engine: Arc<Waking>) -> Ctx {
        let machine = Arc::new(machine);
        a_context()
            .runner(Arc::new(crate::test_support::Scripted(Ok(
                crate::test_support::refused("the daemon said no"),
            ))))
            .engine(engine)
            .settings(settings)
            .build()
            .waiting(Duration::ZERO)
            .with_started(Arc::clone(&machine) as Arc<dyn Started>)
            .with_power(machine as Arc<dyn Supply>)
    }

    /// A context over an emptied scratch directory, with the machine a test scripted.
    fn ctx_at(name: &str, machine: Machine, engine: Arc<Waking>) -> Ctx {
        ctx_with(settings_at(name), machine, engine)
    }

    /// Whether this run got as far as trying to start something.
    ///
    /// A start here is scripted to fail, and a run that tried and failed writes the
    /// condition the next interaction reads while one that declined writes nothing at
    /// all — which is the difference worth asserting, and a stronger claim than the
    /// absence of a reason.
    fn tried_it(ctx: &Ctx) -> bool {
        crate::app::conditions::load(ctx).get(CHECK).is_some()
    }

    /// A machine plugged in, that started at a fixed moment.
    const fn plugged_in(at: u64) -> Machine {
        Machine {
            at: Some(at),
            power: Some(Power::Mains),
        }
    }

    /// The run, driven with no waiting at all.
    async fn run(ctx: &Ctx) -> Result<Outcome, Box<Problem>> {
        waited(ctx, 2, Duration::ZERO, 2, Duration::ZERO).await
    }

    /// Why a run started nothing, where it started nothing.
    fn held(outcome: &Result<Outcome, Box<Problem>>) -> Option<String> {
        match outcome {
            Ok(Outcome::Lifecycle(report)) => report.held.clone(),
            _ => None,
        }
    }

    /// Record what the operator asked for about starting on boot.
    fn asked(ctx: &Ctx, returning: &Returning) {
        crate::app::autostart::save(ctx, returning);
    }

    /// The same context, with somewhere for its words to go.
    fn listening(ctx: Ctx) -> (Ctx, Arc<Heard>) {
        let heard = Arc::new(Heard::default());
        let ctx = ctx.narrating(Arc::clone(&heard) as Arc<dyn Narrator>);
        (ctx, heard)
    }

    #[tokio::test]
    async fn a_machine_nobody_asked_to_start_on_boot_starts_nothing_and_says_so() {
        let ctx = ctx_at("not-asked", plugged_in(1_000), Waking::awake());

        let said = held(&run(&ctx).await);

        assert!(
            said.is_some_and(|said| said.contains("not asked to start")),
            "declining is an answer rather than a failure, and it says which"
        );
        assert!(
            !tried_it(&ctx),
            "and it is not a fault, so there is nothing for the next run to report"
        );
    }

    #[tokio::test]
    async fn a_stack_stopped_on_purpose_is_left_where_the_operator_left_it() {
        // The edge case the requirement is written about, reached through the whole
        // run rather than through the record alone.
        let ctx = ctx_at("on-purpose", plugged_in(1_000), Waking::awake());
        let mut returning = Returning::default().answering(true);
        returning.started(&["tv".to_owned()]);
        returning.stopped();
        asked(&ctx, &returning);

        let said = held(&run(&ctx).await);

        assert!(
            said.as_deref()
                .is_some_and(|said| said.contains("on purpose")),
            "{said:?}"
        );
    }

    #[tokio::test]
    async fn a_laptop_on_its_battery_is_left_alone_unless_it_was_opted_in() {
        // A media stack started on a battery empties one in an afternoon, so the
        // default is to decline and say what it takes to change that.
        let ctx = ctx_at(
            "battery",
            Machine {
                at: Some(1_000),
                power: Some(Power::Battery),
            },
            Waking::awake(),
        );
        asked(&ctx, &Returning::default().answering(true));

        assert_eq!(held(&run(&ctx).await), Some(on_battery_said()));
    }

    #[tokio::test]
    async fn a_laptop_whose_operator_opted_in_is_started_on_its_battery() {
        // The other half of the requirement, and the reason the default is not a
        // rule: an operator who wants it has a reason for it.
        let ctx = ctx_with(
            Settings {
                autostart_on_battery: true,
                ..settings_at("opted-in")
            },
            Machine {
                at: Some(1_000),
                power: Some(Power::Battery),
            },
            Waking::awake(),
        );
        asked(&ctx, &Returning::default().answering(true));

        assert_eq!(held(&run(&ctx).await), None);
        assert!(
            tried_it(&ctx),
            "it got past the battery and on to the start"
        );
    }

    #[tokio::test]
    async fn a_machine_that_will_not_say_where_its_power_comes_from_is_not_on_battery() {
        // A desktop has no power report and no mains adapter to read. Holding a media
        // server's stack back on every restart because a file was missing would be the
        // expensive way round of the two.
        let ctx = ctx_at(
            "desktop",
            Machine {
                at: Some(1_000),
                power: None,
            },
            Waking::awake(),
        );
        asked(&ctx, &Returning::default().answering(true));

        assert_eq!(held(&run(&ctx).await), None);
        assert!(tried_it(&ctx), "it got as far as trying to start");
    }

    #[tokio::test]
    async fn the_same_restart_is_not_acted_on_twice() {
        // A login that fires twice, a service manager that restarts its agent, and an
        // operator running the command by hand must not each re-verify and re-report
        // the same boot.
        let ctx = ctx_at("twice", plugged_in(1_000), Waking::awake());
        asked(&ctx, &Returning::default().answering(true));

        let first = run(&ctx).await;
        let second = run(&ctx).await;

        assert_eq!(held(&first), None, "the first run tried to start");
        assert!(tried_it(&ctx));
        assert_eq!(held(&second).as_deref(), Some(ALREADY_DONE));
    }

    #[tokio::test]
    async fn a_restart_since_the_last_one_is_acted_on_again() {
        let ctx = ctx_at("again", plugged_in(1_000), Waking::awake());
        asked(&ctx, &Returning::default().answering(true));
        let _ = run(&ctx).await;

        let later = ctx.with_started(Arc::new(plugged_in(2_000)));

        assert_eq!(held(&run(&later).await), None);
        assert!(
            tried_it(&later),
            "a different moment is a different restart"
        );
    }

    #[tokio::test]
    async fn an_engine_that_never_answers_is_waited_for_and_then_reported() {
        // Docker Desktop takes the better part of a minute after a login, so a run
        // that asked once would report every Mac in the world as having failed.
        let ctx = ctx_at("no-engine", plugged_in(1_000), Waking::after(usize::MAX));
        asked(&ctx, &Returning::default().answering(true));

        let said = held(&run(&ctx).await);

        assert!(
            said.is_some_and(|said| said.contains("never answered")),
            "the wait ran out and the reason was written down"
        );
        assert!(
            crate::app::conditions::load(&ctx)
                .get(CHECK)
                .is_some_and(crate::condition::Condition::is_raised),
            "and the next interaction has something to tell them"
        );
    }

    #[tokio::test]
    async fn an_engine_that_arrives_late_is_waited_for_rather_than_given_up_on() {
        // The whole of the retry budget's first half: a wired host can be ready to run
        // things before the engine behind them is.
        let ctx = ctx_at("late-engine", plugged_in(1_000), Waking::after(1));
        asked(&ctx, &Returning::default().answering(true));

        assert_eq!(held(&run(&ctx).await), None);
        assert!(tried_it(&ctx), "it got past the engine and on to the start");
    }

    /// A context carrying one standing boot failure, raised at a fixed moment.
    fn failing(name: &str) -> Ctx {
        let ctx = ctx_at(name, plugged_in(1_000), Waking::awake());
        let mut conditions = crate::app::conditions::load(&ctx);
        conditions.observe(
            CHECK,
            Some(&Fault::new(
                "boot.failed",
                Severity::Warning,
                "the stack would not start",
                "Run `lemonfiber up`",
            )),
            "1000",
        );
        crate::app::conditions::save(&ctx, &conditions);
        ctx
    }

    #[tokio::test]
    async fn what_the_last_restart_left_is_said_once_and_not_again() {
        // The whole of the requirement's second half: a boot that failed at four in
        // the morning has nobody to tell, and an operator told about it at every
        // command for a fortnight stops reading anything this product says.
        let (ctx, heard) = listening(failing("owed"));

        reported(&ctx).await;
        reported(&ctx).await;

        let said = heard.lines();
        assert_eq!(said.len(), 1, "told once: {said:?}");
        assert!(
            said.first()
                .is_some_and(|line| line.contains("would not start")),
            "with the reason: {said:?}"
        );
    }

    #[tokio::test]
    async fn a_machine_that_came_back_has_nothing_to_report() {
        let (ctx, heard) = listening(ctx_at("quiet", plugged_in(1_000), Waking::awake()));

        reported(&ctx).await;

        assert!(heard.lines().is_empty());
    }

    #[tokio::test]
    async fn a_rehearsal_is_told_nothing_because_being_told_is_a_change() {
        let (ctx, heard) = listening(failing("rehearsed").rehearsing());

        reported(&ctx).await;

        assert!(heard.lines().is_empty(), "marking one delivered is a write");
    }
}
