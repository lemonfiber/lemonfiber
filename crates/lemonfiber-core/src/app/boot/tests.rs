use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;

use super::{confirmed, on_battery_said, reported, waited, ALREADY_DONE, CHECK};
use crate::app::{Ctx, Outcome};
use crate::autostart::Returning;
use crate::condition::Fault;
use crate::config::{Protocols, Settings};
use crate::error::{Problem, Severity};
use crate::ports::docker::{Health, Lifecycle};
use crate::ports::machine::{Power, Started, Supply};
use crate::ports::process::Output;
use crate::ports::Narrator;
use crate::test_support::{a_context, nowhere, refused, spoke, Reporting, Scripted};
use lemonfiber_fixtures::http::Fake;

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

/// Everything the `library` form declares, which is the form these tests bring
/// back.
///
/// A form the stack this repository carries actually declares, rather than an
/// invented one: the two questions *behind* the start are about a stack that
/// genuinely settled, and a stack settles when the engine reports the services the
/// manifest says the named form holds.
const LIBRARY: [&str; 5] = [
    "jellyfin",
    "seerr",
    "calibre-web-automated",
    "audiobookshelf",
    "navidrome",
];

/// An engine that answers listings straight away and holds nothing.
fn awake() -> Arc<Reporting> {
    Arc::new(Reporting::holding(&[], Lifecycle::Running, Health::Healthy))
}

/// One that is not there at all for its first few listings.
///
/// A desktop engine that has only just been asked to start itself, which is the
/// case the wait at a login exists for: neither reachable nor absent, but one and
/// then the other.
fn waking_after(refusals: usize) -> Arc<Reporting> {
    let engine = Reporting::holding(&[], Lifecycle::Running, Health::Healthy);
    Arc::new(engine.waking_after(refusals))
}

/// One that never turns up, however long it is waited for.
fn never_there() -> Arc<Reporting> {
    Arc::new(Reporting::absent())
}

/// One holding the library form's services, every one of them healthy.
fn holding_the_library() -> Arc<Reporting> {
    Arc::new(Reporting::holding(
        &LIBRARY,
        Lifecycle::Running,
        Health::Healthy,
    ))
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

/// A context with the machine, the engine and the Compose answer a test scripted.
///
/// Patience is nothing at all, so a start that is going to fail fails now rather
/// than after the minutes a real one is allowed. The transport answers nothing, so
/// the trust checks run after a start that came back resolve to unreachable rather
/// than reaching the real network from a test.
fn ctx_answering(
    settings: Settings,
    machine: Machine,
    engine: Arc<Reporting>,
    compose: Output,
) -> Ctx {
    let machine = Arc::new(machine);
    a_context()
        .runner(Arc::new(Scripted(Ok(compose))))
        .engine(engine)
        .settings(settings)
        .build()
        .waiting(Duration::ZERO)
        .with_http(Fake::scripted(Vec::new()))
        .with_started(Arc::clone(&machine) as Arc<dyn Started>)
        .with_power(machine as Arc<dyn Supply>)
}

/// The same, over a Compose that refuses.
///
/// Every start built this way therefore fails, which is deliberate: what most of
/// these tests are about is the four questions in front of the start and what is
/// written down behind it, and a start that actually settled would need a stack of
/// containers to settle.
fn ctx_with(settings: Settings, machine: Machine, engine: Arc<Reporting>) -> Ctx {
    ctx_answering(settings, machine, engine, refused("the daemon said no"))
}

/// A context over an emptied scratch directory, with the machine a test scripted.
fn ctx_at(name: &str, machine: Machine, engine: Arc<Reporting>) -> Ctx {
    ctx_with(settings_at(name), machine, engine)
}

/// A context whose Compose works and whose engine holds the library form, healthy.
///
/// The other side of every context above, and the one arrangement in which a boot
/// start actually brings a stack back — which is what the two questions behind the
/// start are asked about. The settings are the caller's, because the protocols in
/// them are what decide whether there is a tunnel to ask after at all.
fn ctx_coming_back(settings: Settings) -> Ctx {
    ctx_answering(
        settings,
        plugged_in(1_000),
        holding_the_library(),
        spoke(""),
    )
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
    crate::autostart::run::save(ctx, returning);
}

/// A machine asked to bring the library form back after a restart.
///
/// Named rather than left to the empty list, which means every form: bringing back
/// nineteen services would need nineteen of them reported healthy to settle, and
/// what these tests are about is not how many.
fn asked_for_the_library(ctx: &Ctx) {
    let mut returning = Returning::default().answering(true);
    returning.started(&["library".to_owned()]);
    asked(ctx, &returning);
}

/// What the next thing the operator types would be told about the last boot.
///
/// Read out of the condition store rather than off the report, because the store
/// is where it actually has to be: a boot happens at four in the morning with
/// nobody there, so a reason that only ever existed in the run's own answer has
/// reached nobody.
fn standing(ctx: &Ctx) -> Option<String> {
    crate::app::conditions::load(ctx)
        .get(CHECK)
        .filter(|condition| condition.is_raised())
        .map(|condition| condition.summary.clone())
}

/// The same context, with somewhere for its words to go.
fn listening(ctx: Ctx) -> (Ctx, Arc<Heard>) {
    let heard = Arc::new(Heard::default());
    let ctx = ctx.narrating(Arc::clone(&heard) as Arc<dyn Narrator>);
    (ctx, heard)
}

#[tokio::test]
async fn a_machine_nobody_asked_to_start_on_boot_starts_nothing_and_says_so() {
    let ctx = ctx_at("not-asked", plugged_in(1_000), awake());

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
    let ctx = ctx_at("on-purpose", plugged_in(1_000), awake());
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
        awake(),
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
        awake(),
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
        awake(),
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
    let ctx = ctx_at("twice", plugged_in(1_000), awake());
    asked(&ctx, &Returning::default().answering(true));

    let first = run(&ctx).await;
    let second = run(&ctx).await;

    assert_eq!(held(&first), None, "the first run tried to start");
    assert!(tried_it(&ctx));
    assert_eq!(held(&second).as_deref(), Some(ALREADY_DONE));
}

#[tokio::test]
async fn a_restart_since_the_last_one_is_acted_on_again() {
    let ctx = ctx_at("again", plugged_in(1_000), awake());
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
    let ctx = ctx_at("no-engine", plugged_in(1_000), never_there());
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
    let ctx = ctx_at("late-engine", plugged_in(1_000), waking_after(1));
    asked(&ctx, &Returning::default().answering(true));

    assert_eq!(held(&run(&ctx).await), None);
    assert!(tried_it(&ctx), "it got past the engine and on to the start");
}

/// A stack that came back is started once and leaves the operator nothing to read.
///
/// The plain case a login is supposed to end in, and the one nothing here could
/// assert until a start could actually succeed: the stack settles, the checks
/// behind it find nothing wrong, and the next thing the operator types says
/// nothing at all. That last part is the claim worth making — a boot that reported
/// itself on every subsequent command would train them to stop reading the
/// sentences that matter, and the whole point of filing this under a condition is
/// that there is nothing to file when it worked.
///
/// Nothing on this machine is configured to torrent, so there is no tunnel for the
/// trust checks to ask after and they say so rather than failing — which leaves the
/// start itself as the only thing under test here.
#[tokio::test]
async fn a_stack_that_came_back_is_not_started_twice_and_leaves_nothing_to_report() {
    let ctx = ctx_coming_back(settings_at("came-back"));
    asked_for_the_library(&ctx);
    let (ctx, heard) = listening(ctx);

    let outcome = run(&ctx).await;

    assert!(outcome.is_ok(), "the start was carried out: {outcome:?}");
    assert_eq!(held(&outcome), None, "and nothing in front of it declined");
    assert_eq!(
        standing(&ctx),
        None,
        "a stack that came back whole has nothing to file against it"
    );
    let said = heard.lines().join("\n");
    assert!(
        !said.contains("trying again"),
        "and a start that worked is not tried a second time: {said}"
    );
}

/// A stack that cannot be read leaves the question unanswered rather than answered.
///
/// The confirmation behind a boot start asks the trust checks what is wrong with
/// the tunnel, and those need the stack read before any of them can run. Where it
/// cannot be, the honest answer is that nothing was established — so nothing is
/// filed. Filing it as a failed boot would be the comfortable falsehood: the
/// operator would be told at their next interaction that the stack did not come
/// back, on the strength of a check that never ran, and the thing actually wrong
/// with their machine is that lemonfiber cannot find the stack at all.
#[tokio::test]
async fn a_confirmation_that_could_not_run_files_nothing_against_the_boot() {
    let ctx = a_context().over(nowhere()).build();

    confirmed(&ctx).await;

    assert_eq!(
        standing(&ctx),
        None,
        "a check that could not run has established nothing to report"
    );
}

/// A stack that came back without its tunnel is a failed boot, not a successful one.
///
/// The half of a restart that does not look after itself. Containers come back
/// under their own restart policies; a tunnel comes back on a new address and,
/// commonly, a different forwarded port — so a machine can restart into a stack
/// that is entirely up and torrenting in the open. That is worse than one that
/// never came back at all, because nothing about it looks wrong from the outside
/// and nobody goes looking.
///
/// Filed under the same one check as every other way this can fail, which is what
/// keeps a fortnight of failed restarts one thing to tell the operator about rather
/// than fourteen.
#[tokio::test]
async fn a_stack_that_came_back_without_its_tunnel_is_filed_as_a_failed_boot() {
    let ctx = ctx_coming_back(Settings {
        protocols: Protocols::both(),
        ..settings_at("no-tunnel")
    });
    asked_for_the_library(&ctx);

    let outcome = run(&ctx).await;

    assert_eq!(held(&outcome), None, "the stack itself came back");
    let filed = standing(&ctx);
    assert!(
        filed
            .as_deref()
            .is_some_and(|said| said.contains("its tunnel did not")),
        "{filed:?}"
    );
}

/// A start that could not be carried out reaches the surface as the refusal it is,
/// and is filed as well.
///
/// Compose ran, said it was done, and the services never turned up. That is the one
/// shape of failure with no report to hand back — there is nothing to say about a
/// start except why it could not be made — and it is therefore the one shape where
/// the reason could most easily be lost between the run and the operator.
///
/// Both halves have to hold at once, and they are different halves. The refusal is
/// for whoever is standing there; the condition is for whoever was asleep at four
/// in the morning, which at a login is everybody. A run that returned the problem
/// and filed nothing would be a failure nobody ever hears about.
#[tokio::test]
async fn a_start_that_could_not_be_carried_out_is_a_refusal_rather_than_a_report() {
    let ctx = ctx_answering(
        settings_at("never-settled"),
        plugged_in(1_000),
        awake(),
        spoke(""),
    );
    asked_for_the_library(&ctx);

    let outcome = run(&ctx).await;

    assert_eq!(
        outcome.as_ref().err().map(|problem| problem.code),
        Some(crate::app::NEVER_SETTLED),
        "the services never settled, and that is what the operator is told"
    );
    assert_eq!(
        held(&outcome),
        None,
        "a refusal carries no report, so there is no reason in one to read"
    );
    let filed = standing(&ctx);
    assert!(
        filed
            .as_deref()
            .is_some_and(|said| said.contains("did not finish starting")),
        "{filed:?}"
    );
}

/// A rehearsal writes down neither the restart it acted on nor what it came to.
///
/// The one way a rehearsal of this could change the thing it was rehearsing. A run
/// that recorded the restart as dealt with would make the real start behind it
/// decline, leaving a machine that comes back only on the logins nobody rehearsed —
/// which is the exact failure this whole run exists to remove, introduced by the
/// command that promised to touch nothing.
///
/// And a rehearsal that filed a condition would have the operator told, at their
/// next command, about a boot failure that never happened. Both are asserted here
/// because they are written by two different steps, and either one alone would
/// leave the other unread.
#[tokio::test]
async fn a_rehearsed_boot_records_neither_the_restart_nor_what_it_came_to() {
    let ctx = ctx_at("rehearsed-boot", plugged_in(1_000), awake()).rehearsing();
    asked(&ctx, &Returning::default().answering(true));

    let first = run(&ctx).await;
    let second = run(&ctx).await;

    assert_eq!(held(&first), None, "it got past the four questions");
    assert_eq!(
        standing(&ctx),
        None,
        "and filed nothing, because being told is itself a change"
    );
    assert_eq!(
        held(&second),
        None,
        "nor wrote the restart down, so the run that follows still acts on it"
    );
}

/// A context carrying one standing boot failure, raised at a fixed moment.
fn failing(name: &str) -> Ctx {
    let ctx = ctx_at(name, plugged_in(1_000), awake());
    let mut conditions = crate::app::conditions::load(&ctx);
    conditions.observe(
        CHECK,
        Some(&Fault::new(
            "boot.failed",
            Severity::Warning,
            "the stack would not start",
            "nothing has been running since",
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
    let (ctx, heard) = listening(ctx_at("quiet", plugged_in(1_000), awake()));

    reported(&ctx).await;

    assert!(heard.lines().is_empty());
}

#[tokio::test]
async fn a_rehearsal_is_told_nothing_because_being_told_is_a_change() {
    let (ctx, heard) = listening(failing("rehearsed").rehearsing());

    reported(&ctx).await;

    assert!(heard.lines().is_empty(), "marking one delivered is a write");
}
