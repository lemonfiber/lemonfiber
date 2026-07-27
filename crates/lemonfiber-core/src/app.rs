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

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc::Receiver;

use crate::config::store;
use crate::config::Settings;
use crate::docker::{condition, survey, unsettled, Service};
use crate::doctor::environment::EnvironmentCheck;
use crate::doctor::storage::StorageCheck;
use crate::doctor::vpn::VpnCheck;
use crate::doctor::{examine, Category, Check};
use crate::error::{Code, Diagnose, Problem, Remedy, Severity, State};
use crate::model::{
    ConfigReport, DoctorReport, Envelope, LifecycleReport, SettingReport, StatusReport,
    SupervisionReport, VersionReport,
};
use crate::platform::Environment;
use crate::ports::docker::{Engine, LogLine, LogQuery};
use crate::ports::filesystem::{Presence, Volume};
use crate::ports::{Clock, FileSystem, Runner};
use crate::stack::closure::resolve;
use crate::stack::compose::{build, Action};
use crate::stack::Source;

/// What a surface is asking for.
///
/// Deliberately exhaustive. The surfaces ship in the same binary, so a new
/// command should stop the build until every surface has decided what to do
/// with it — silently rendering nothing is the failure this prevents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Report the binary's version, and the engine's where it can be reached.
    Version,
    /// Start one or more forms.
    Up {
        /// The forms to start, resolved to the union of their closures.
        forms: Vec<String>,
    },
    /// Stop and remove what a form started.
    Down {
        /// The forms to stop.
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
    /// Run the diagnostic checks, or one category of them.
    Doctor {
        /// The category to run; empty runs every check there is.
        only: Option<Category>,
        /// Whether the operator opted into the checks that disturb the system.
        disruptive: bool,
    },
}

/// What dispatching produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The answer to [`Command::Version`].
    Version(VersionReport),
    /// What a lifecycle command did, or would have done.
    Lifecycle(LifecycleReport),
    /// The answer to a configuration command.
    Config(ConfigReport),
    /// What each service is doing.
    Status(StatusReport),
    /// What the diagnostic checks found.
    Doctor(DoctorReport),
}

impl Outcome {
    /// Wrap this outcome for machine-readable output.
    #[must_use]
    pub fn envelope(self) -> Envelope<Self> {
        let kind = match self {
            Self::Version(_) => "version",
            Self::Lifecycle(_) => "lifecycle",
            Self::Config(_) => "config",
            Self::Status(_) => "status",
            Self::Doctor(_) => "doctor",
        };
        Envelope::new(kind, self)
    }
}

impl serde::Serialize for Outcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Version(report) => report.serialize(serializer),
            Self::Lifecycle(report) => report.serialize(serializer),
            Self::Config(report) => report.serialize(serializer),
            Self::Status(report) => report.serialize(serializer),
            Self::Doctor(report) => report.serialize(serializer),
        }
    }
}

/// Everything a command needs that is not part of the command itself.
pub struct Ctx {
    /// Whether to report what would happen and change nothing.
    pub dry_run: bool,
    /// How programs are run.
    pub runner: Arc<dyn Runner>,
    /// How the engine is observed.
    pub engine: Arc<dyn Engine>,
    /// What time it is, for the one rule that depends on it.
    pub clock: Arc<dyn Clock>,
    /// How the filesystem is reached, for the checks that prove what it can do.
    pub filesystem: Arc<dyn FileSystem>,
    /// How long starting waits for services to settle before giving up.
    ///
    /// A knob rather than a constant because it is a policy: an operator on a
    /// slow disk needs longer than the default, and a test needs none at all.
    pub patience: Duration,
    /// Which stack is being operated.
    pub stack: Source,
    /// What the operator chose.
    pub settings: Settings,
    /// Which of the four environments this is.
    ///
    /// Supplied rather than decided here. Telling Docker Engine from Docker
    /// Desktop means asking the daemon, and a core that guessed would be wrong
    /// silently — so the surface answers, and today it answers with what it can
    /// see until the engine adapter can tell it the rest.
    pub environment: Environment,
}

impl Ctx {
    /// A context that runs programs for real, against a given stack.
    #[must_use]
    pub fn new(
        runner: Arc<dyn Runner>,
        engine: Arc<dyn Engine>,
        clock: Arc<dyn Clock>,
        filesystem: Arc<dyn FileSystem>,
        stack: Source,
        settings: Settings,
        environment: Environment,
    ) -> Self {
        Self {
            dry_run: false,
            runner,
            engine,
            clock,
            filesystem,
            patience: PATIENCE,
            stack,
            settings,
            environment,
        }
    }

    /// The same context, willing to wait a different length of time.
    #[must_use]
    pub const fn waiting(mut self, patience: Duration) -> Self {
        self.patience = patience;
        self
    }

    /// Today, as the manifest's date rules mean it.
    ///
    /// A clock before the epoch, or one far enough ahead to overflow a calendar,
    /// falls back to the epoch: refusing to do anything because the machine's
    /// clock is absurd would be a worse answer than checking dates against a
    /// date that is merely wrong.
    fn today(&self) -> lemonfiber_manifest::Date {
        let seconds = self
            .clock
            .now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()
            .and_then(|elapsed| i64::try_from(elapsed.as_secs()).ok())
            .unwrap_or_default();
        lemonfiber_manifest::Date::from_unix_seconds(seconds).unwrap_or(EPOCH)
    }

    /// The same context, in rehearsal.
    #[must_use]
    pub fn rehearsing(mut self) -> Self {
        self.dry_run = true;
        self
    }
}

/// The first day the calendar rules can name, used when the clock cannot be
/// believed at all.
const EPOCH: lemonfiber_manifest::Date = lemonfiber_manifest::Date {
    year: 1970,
    month: 1,
    day: 1,
};

/// How long starting waits for every service to settle.
///
/// Long enough for the slowest first run on a spinning disk, and bounded
/// because a wait with no end is indistinguishable from a hang.
const PATIENCE: Duration = Duration::from_secs(180);

/// How often the engine is asked whether anything has changed.
const POLL: Duration = Duration::from_millis(500);

/// How many recent lines a service that would not start is asked for.
const LAST_WORDS: u32 = 20;

/// Raised when a service never reached a state that starting could accept.
pub const NEVER_SETTLED: Code = Code::new("LIFE-1");

/// Wait until every service has settled, or until patience runs out.
///
/// Polls rather than subscribes to engine events, because the question is about
/// the whole set rather than about any one container, and re-reading nineteen
/// summaries twice a second costs less than the machinery for correlating an
/// event stream back into the same answer.
/// The manifest is handed in rather than read again. Whoever is waiting has
/// already resolved and validated it to know what to start, and reading it a
/// second time would add a way for this to fail that cannot happen — a failure
/// no test can reach is a failure nobody has checked the wording of.
async fn settle(
    ctx: &Ctx,
    manifest: &lemonfiber_manifest::Manifest,
    profiles: &[String],
) -> Result<Vec<Service>, Problem> {
    let deadline = ctx.clock.now() + ctx.patience;

    loop {
        let containers = ctx
            .engine
            .list(&ctx.settings.project)
            .await
            .map_err(|err| err.problem())?;
        let services = survey(manifest, profiles, &containers);

        let waiting: Vec<String> = unsettled(&services)
            .into_iter()
            .map(|service| service.id.clone())
            .collect();
        if waiting.is_empty() {
            return Ok(services);
        }

        // Checked after the survey rather than before it, so a patience of zero
        // still reports what it saw rather than reporting nothing at all.
        if ctx.clock.now() >= deadline {
            return Err(never_settled(ctx, &waiting).await);
        }

        tokio::time::sleep(POLL).await;
    }
}

/// What to tell an operator whose stack did not finish starting.
///
/// The services' own recent output is attached, because the explanation is
/// almost always in it and an operator who has to go and find it has been given
/// a fault report rather than a diagnosis.
async fn never_settled(ctx: &Ctx, waiting: &[String]) -> Problem {
    let named = waiting.join(", ");
    let problem = Problem::new(
        NEVER_SETTLED,
        Severity::Error,
        format!("{named} did not finish starting"),
        "The containers were started and never reached a state that counts as running. \
         Whatever went wrong is usually in their own output, which is below.",
        Remedy::new("Look at what the service said, then start it again")
            .with_detail("lemonfiber logs <service>"),
    )
    .in_state(State::Guided);

    // An engine that will not open the stream falls back to one that is already
    // finished, so the reading below has no second shape. There is nothing
    // useful to say about a service whose output cannot be read that the
    // problem does not already say.
    let (closed, silent) = tokio::sync::mpsc::channel(1);
    drop(closed);

    let query = LogQuery::recent(LAST_WORDS);
    let mut lines = ctx
        .engine
        .logs(&ctx.settings.project, waiting, query)
        .await
        .unwrap_or(silent);

    let mut said = String::new();
    while let Some(line) = lines.recv().await {
        said.push_str(&line.service);
        said.push_str(": ");
        said.push_str(&line.line);
        said.push('\n');
    }

    if said.is_empty() {
        return problem;
    }
    problem.with_detail(said)
}

/// Stream a project's log lines, tagged by the service that wrote them.
///
/// Streaming has its own entry point rather than an [`Outcome`], because a log
/// stream is not a value that arrives once. Forcing it into one would mean
/// either buffering output that has no end or giving each surface its own way
/// of reading it, and the second is the drift [`dispatch`] exists to prevent —
/// so there is still exactly one implementation, and all three surfaces call it.
///
/// # Errors
///
/// Returns the [`Problem`] a surface should render when the stack cannot be
/// resolved or the engine cannot be reached.
pub async fn logs(
    ctx: &Ctx,
    forms: &[String],
    services: &[String],
    query: LogQuery,
) -> Result<Receiver<LogLine>, Problem> {
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| err.problem())?;

    // Naming forms and naming services are two ways of saying the same thing,
    // and both narrow: a form is the services its profiles declare.
    let mut wanted: Vec<String> = services.to_vec();
    if !forms.is_empty() {
        let plan =
            resolve(&manifest, forms, ctx.settings.protocols).map_err(|err| err.problem())?;
        let profiles: Vec<String> = plan.profiles.into_iter().collect();
        wanted.extend(
            manifest
                .services
                .iter()
                .filter(|service| profiles.contains(&service.profile))
                .filter(|service| services.is_empty() || services.contains(&service.id))
                .map(|service| service.id.clone()),
        );
        wanted.retain(|id| manifest.services.iter().any(|service| &service.id == id));
    }

    ctx.engine
        .logs(&ctx.settings.project, &wanted, query)
        .await
        .map_err(|err| err.problem())
}

/// Carry out a command.
///
/// # Errors
///
/// Returns the [`Problem`] a surface should render when the command could not
/// be carried out.
pub async fn dispatch(command: Command, ctx: &Ctx) -> Result<Outcome, Problem> {
    match command {
        Command::Version => version(ctx).await.map(Outcome::Version),
        Command::Up { forms } => lifecycle(ctx, &forms, &Action::Up).await,
        Command::Down { forms } => lifecycle(ctx, &forms, &Action::Down).await,
        Command::Restart { forms, services } => {
            lifecycle(ctx, &forms, &Action::Restart(services)).await
        }
        Command::Pull { forms } => lifecycle(ctx, &forms, &Action::Pull).await,
        Command::ConfigGet { key } => configuration(ctx, Some(&key), None).map_err(|p| *p),
        Command::ConfigSet { key, value } => {
            configuration(ctx, Some(&key), Some(&value)).map_err(|p| *p)
        }
        Command::ConfigShow => configuration(ctx, None, None).map_err(|p| *p),
        Command::Ps { forms } => status(ctx, &forms).await.map(Outcome::Status),
        Command::Doctor { only, disruptive } => {
            diagnose(ctx, only, disruptive).await.map(Outcome::Doctor)
        }
    }
}

/// Run the diagnostic checks, or the one category asked for.
///
/// The checks are assembled here rather than held on the context because each
/// needs a slice of it — the VPN check needs the engine, the resolved pair and
/// the operator's echo choice — and building them at the point of use keeps the
/// context a bag of capabilities rather than a registry of features.
async fn diagnose(
    ctx: &Ctx,
    only: Option<Category>,
    disruptive: bool,
) -> Result<DoctorReport, Problem> {
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| err.problem())?;

    let environment = EnvironmentCheck::new(ctx.runner.clone());
    let storage = StorageCheck::new(
        ctx.filesystem.clone(),
        ctx.settings.data_root.clone(),
        ctx.settings.storage_state.clone(),
        ctx.environment,
        ctx.settings.service_user,
    );
    let vpn = VpnCheck::new(
        ctx.engine.clone(),
        ctx.settings.project.clone(),
        &manifest,
        ctx.settings.protocols,
        ctx.settings.ip_echo.clone(),
        ctx.settings.port_forward.clone(),
        disruptive,
    );
    let checks: Vec<Box<dyn Check>> = vec![Box::new(environment), Box::new(storage), Box::new(vpn)];

    Ok(examine(&checks, only).await)
}

/// What every service in the named forms is doing.
///
/// Naming no form reports the whole stack, because "what is running" is a
/// question about the machine rather than about a form — and an operator asking
/// it has usually forgotten which form they started.
async fn status(ctx: &Ctx, forms: &[String]) -> Result<StatusReport, Problem> {
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| err.problem())?;

    let profiles: Vec<String> = if forms.is_empty() {
        manifest
            .profiles
            .iter()
            .map(|profile| profile.id.clone())
            .collect()
    } else {
        resolve(&manifest, forms, ctx.settings.protocols)
            .map_err(|err| err.problem())?
            .profiles
            .into_iter()
            .collect()
    };

    let containers = ctx
        .engine
        .list(&ctx.settings.project)
        .await
        .map_err(|err| err.problem())?;
    let services = survey(&manifest, &profiles, &containers);

    Ok(StatusReport {
        forms: forms.to_vec(),
        condition: condition(&services),
        services,
    })
}

/// Read or change settings.
///
/// A rehearsal reads and reports what it would have written without writing it,
/// so `--dry-run` means the same thing here as everywhere else.
///
/// The failure is boxed. This is the only fallible path here that is not async,
/// so it is the only one where a large error variant sits in the returned value
/// rather than inside a future — and a problem is a rare, cold thing that is
/// cheaper to move behind a pointer.
fn configuration(
    ctx: &Ctx,
    key: Option<&str>,
    value: Option<&str>,
) -> Result<Outcome, Box<Problem>> {
    let Some(path) = ctx.settings.env_file.as_deref() else {
        return Err(Box::new(store::Failure::Nowhere.problem()));
    };

    let changed = match (key, value) {
        (Some(key), Some(value)) if !ctx.dry_run => {
            if let Err(err) = store::set(path, key, value) {
                return Err(Box::new(err.problem()));
            }
            true
        }
        (_, value) => value.is_some(),
    };

    let file = match store::read(path) {
        Ok(file) => file,
        Err(err) => return Err(Box::new(err.problem())),
    };
    let settings = store::shown(&file)
        .into_iter()
        .filter(|setting| key.is_none_or(|wanted| setting.key == wanted))
        .map(|setting| SettingReport {
            key: setting.key,
            value: setting.value,
            secret: setting.secret,
        })
        .collect();

    Ok(Outcome::Config(ConfigReport {
        settings,
        changed,
        rehearsed: ctx.dry_run,
    }))
}

/// Resolve forms, build the command, and run it unless this is a rehearsal.
///
/// Nothing here decides anything a surface could have decided differently, which
/// is the point: `up` from a keypress and `up` from a subcommand reach this same
/// function with the same arguments.
async fn lifecycle(ctx: &Ctx, forms: &[String], action: &Action) -> Result<Outcome, Problem> {
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| err.problem())?;
    let plan = resolve(&manifest, forms, ctx.settings.protocols).map_err(|err| err.problem())?;
    let stack = ctx
        .stack
        .materialise(ctx.settings.stack_dir.as_deref())
        .map_err(|err| err.problem())?;

    let command = build(&plan, &ctx.settings, &stack, action, ctx.environment);

    let mut report = LifecycleReport {
        action: action.name().to_owned(),
        profiles: plan.profiles.into_iter().collect(),
        dropped: plan.dropped.into_iter().collect(),
        command: command.clone(),
        rehearsed: ctx.dry_run,
        status: None,
        services: Vec::new(),
        condition: None,
    };

    // A rehearsal stops here deliberately: it has already done everything except
    // the one irreversible step, so what it reports is what would run rather
    // than an approximation of it.
    if ctx.dry_run {
        return Ok(Outcome::Lifecycle(report));
    }

    let output = ctx
        .runner
        .run(&command)
        .await
        .map_err(|err| err.problem())?;
    report.status = output.status;

    // Starting waits for the services to be usable, because "started" that
    // means "a process exists" is a claim the operator will disprove by opening
    // a browser. Nothing else waits: stopping is done when Compose says so.
    if action == &Action::Up && output.succeeded() {
        let settled = settle(ctx, &manifest, &report.profiles).await?;
        report.condition = Some(condition(&settled));
        report.services = settled;
    }

    Ok(Outcome::Lifecycle(report))
}

/// How often a watch re-checks that the data root is still there.
///
/// Frequent enough to stop the services before much is written into a vanished
/// mount, and no more, because the check is a stat and doing it in a tight loop
/// would spin a core to catch an event that arrives in seconds at worst.
pub const WATCH: Duration = Duration::from_secs(5);

/// Raised when a watch is asked for but no data location is configured to watch.
pub const NOTHING_TO_WATCH: Code = Code::new("WATCH-1");

/// Raised when the data location is already gone when the watch is asked to
/// start.
pub const ALREADY_GONE: Code = Code::new("WATCH-2");

/// How a watch ended.
enum Loss {
    /// The data root's path is no longer there at all.
    Vanished,
    /// The path is there, but on a different volume than it started on — the
    /// shape of a drive pulled out from under a surviving mount point.
    Moved,
}

/// Whether the data root is still the one the watch began guarding.
enum Availability {
    /// Present, and the same volume as before.
    Holding,
    /// Lost, and how.
    Lost(Loss),
}

/// Read a fresh presence against the one the watch started with.
///
/// A path on a different volume is a loss, not a presence: it is what a mount
/// point left behind by an unplugged drive looks like, and treating it as "still
/// there" is exactly the mistake that lets the services write a phantom library
/// onto the system disk.
fn assess(baseline: u64, current: Presence) -> Availability {
    match current {
        Presence::Gone => Availability::Lost(Loss::Vanished),
        Presence::On(volume) if volume == baseline => Availability::Holding,
        Presence::On(_) => Availability::Lost(Loss::Moved),
    }
}

/// Poll the data root until it is lost, and say how it was lost.
async fn watch_until_lost(
    volume: &dyn Volume,
    root: &Path,
    baseline: u64,
    interval: Duration,
) -> Loss {
    loop {
        tokio::time::sleep(interval).await;
        match assess(baseline, volume.presence(root).await) {
            Availability::Holding => {}
            Availability::Lost(loss) => return loss,
        }
    }
}

/// Watch the data root while the given forms run, and stop them the moment it is
/// lost — never restarting it, because whether the state that came back is
/// trustworthy is the operator's call, not this one's.
///
/// # Errors
///
/// Returns a [`Problem`] where there is no data location configured to watch, or
/// where it is already gone before the watch can begin.
pub async fn supervise(
    ctx: &Ctx,
    volume: &dyn Volume,
    forms: &[String],
    interval: Duration,
) -> Result<SupervisionReport, Problem> {
    let Some(root) = ctx.settings.data_root.as_deref() else {
        return Err(nothing_to_watch());
    };
    let baseline = match volume.presence(root).await {
        Presence::On(volume) => volume,
        Presence::Gone => return Err(already_gone(root)),
    };

    let loss = watch_until_lost(volume, root, baseline, interval).await;

    // The stop is attempted whatever its outcome: the data root is gone either
    // way, and reporting that the services could not be stopped is more use than
    // refusing to report the loss at all.
    let stopped = matches!(
        lifecycle(ctx, forms, &Action::Stop).await,
        Ok(Outcome::Lifecycle(report)) if report.status == Some(0)
    );

    Ok(SupervisionReport {
        forms: forms.to_vec(),
        reason: describe_loss(&loss),
        stopped,
    })
}

/// The one-line reason a watch ended, for the operator.
fn describe_loss(loss: &Loss) -> String {
    match loss {
        Loss::Vanished => "the data location is no longer present".to_owned(),
        Loss::Moved => "the data location is now a different volume — the drive holding it was \
                        most likely disconnected"
            .to_owned(),
    }
}

/// The problem for a watch with no data location to guard.
fn nothing_to_watch() -> Problem {
    Problem::new(
        NOTHING_TO_WATCH,
        Severity::Error,
        "There is no data location to watch",
        "A watch guards the directory your downloads and library live in, and none is configured \
         yet, so there is nothing for it to guard.",
        Remedy::new("Run setup to choose a data location, then start the watch again"),
    )
    .in_state(State::Guided)
}

/// The problem for a watch whose data location is gone before it starts.
fn already_gone(root: &Path) -> Problem {
    Problem::new(
        ALREADY_GONE,
        Severity::Error,
        format!("The data location {} is not available", root.display()),
        "A watch can only guard a location that is present when it begins; this one is already \
         gone, so there is nothing running over it to protect.",
        Remedy::new("Connect the drive or mount holding the data location, then start the watch"),
    )
    .in_state(State::Guided)
}

/// The binary's version, and the engine's where it answers.
///
/// An unreachable engine is reported as absent rather than as a failure: asking
/// what versions are in play is exactly what an operator does when something is
/// wrong, so it must still answer when the engine is down.
async fn version(ctx: &Ctx) -> Result<VersionReport, Problem> {
    let argv = ["docker", "compose", "version", "--short"].map(str::to_owned);
    let compose = match ctx.runner.run(&argv).await {
        Ok(output) if output.succeeded() => Some(output.stdout.trim().to_owned()),
        Ok(_) | Err(_) => None,
    };

    // The stack is the one thing here that can genuinely be wrong: an
    // unreadable directory is the operator's own `--stack-dir`, and they need
    // to hear about it rather than see a version report with a hole in it.
    let stack = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| err.problem())?;

    Ok(VersionReport {
        binary: env!("CARGO_PKG_VERSION").to_owned(),
        supported_schema: lemonfiber_manifest::SUPPORTED_SCHEMA_VERSIONS.to_vec(),
        stack: stack.stack_version,
        compose,
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;

    use super::{
        dispatch, Category, Command, Ctx, Environment, Outcome, Settings, Source, VersionReport,
    };
    use crate::docker::{Condition, State as ServiceState};
    use crate::ports::docker::{
        Engine, Failure as EngineFailure, Health, Lifecycle, LogLine, LogQuery,
    };
    use crate::ports::process::{Failure, Output, Runner};
    use std::time::Duration;
    use tokio::sync::mpsc::Receiver;

    /// A runner that answers with whatever the test scripted.
    struct Scripted(Result<Output, Failure>);

    /// An engine that reports whatever the test put in it.
    ///
    /// A trait fake is the right tool here and the wrong one for the adapter:
    /// the question above the port is what lemonfiber does with an answer, and
    /// the question below it is whether the wire is spoken correctly.
    #[derive(Default)]
    struct Reporting {
        containers: Vec<crate::ports::docker::Container>,
        said: Vec<crate::ports::docker::LogLine>,
        reachable: bool,
        /// How many listings to answer before every container reports healthy.
        ///
        /// Absent means the engine never changes its mind. A stack that is
        /// genuinely starting does not answer the same way twice, and an engine
        /// that always did would make the waiting itself untestable.
        settles_after: Option<usize>,
        asked: std::sync::atomic::AtomicUsize,
    }

    impl Reporting {
        /// An engine reporting the named services in one state.
        fn holding(services: &[&str], lifecycle: Lifecycle, health: Health) -> Self {
            Self {
                containers: services
                    .iter()
                    .map(|service| crate::ports::docker::Container {
                        id: format!("id-{service}"),
                        project: "lemonfiber".to_owned(),
                        service: (*service).to_owned(),
                        lifecycle,
                        health,
                        exit: None,
                    })
                    .collect(),
                said: Vec::new(),
                reachable: true,
                settles_after: None,
                asked: std::sync::atomic::AtomicUsize::new(0),
            }
        }

        /// The same engine, unsettled until it has been asked `listings` times.
        fn settling_after(mut self, listings: usize) -> Self {
            self.settles_after = Some(listings);
            self
        }

        /// The same engine, with something to say about a service.
        fn saying(mut self, service: &str, line: &str) -> Self {
            self.said.push(crate::ports::docker::LogLine {
                service: service.to_owned(),
                stream: crate::ports::docker::Stream::Stderr,
                at: None,
                line: line.to_owned(),
            });
            self
        }

        /// An engine that is not there.
        fn absent() -> Self {
            Self {
                containers: Vec::new(),
                said: Vec::new(),
                reachable: false,
                settles_after: None,
                asked: std::sync::atomic::AtomicUsize::new(0),
            }
        }
    }

    #[async_trait]
    impl Engine for Reporting {
        async fn list(
            &self,
            _project: &str,
        ) -> Result<Vec<crate::ports::docker::Container>, EngineFailure> {
            if !self.reachable {
                return Err(EngineFailure::Unreachable {
                    reason: "no daemon here".to_owned(),
                });
            }

            let asked = self
                .asked
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let settled = self.settles_after.is_some_and(|after| asked >= after);

            Ok(self
                .containers
                .iter()
                .map(|container| crate::ports::docker::Container {
                    health: if settled {
                        Health::Healthy
                    } else {
                        container.health
                    },
                    ..container.clone()
                })
                .collect())
        }

        async fn exec(
            &self,
            container: &str,
            _argv: &[String],
        ) -> Result<crate::ports::docker::ExecOutput, EngineFailure> {
            Err(EngineFailure::NoSuchContainer {
                name: container.to_owned(),
            })
        }

        async fn stats(
            &self,
            _project: &str,
        ) -> Result<Receiver<(String, crate::ports::docker::Stats)>, EngineFailure> {
            let (_sender, receiver) = tokio::sync::mpsc::channel(1);
            Ok(receiver)
        }

        async fn logs(
            &self,
            _project: &str,
            services: &[String],
            _query: LogQuery,
        ) -> Result<Receiver<LogLine>, EngineFailure> {
            // Opening a stream means finding the containers first, so an engine
            // that cannot be reached refuses this as surely as it refuses a
            // listing. A fake that answered anyway would be a fake that made
            // the health gate's own failure path untestable.
            if !self.reachable {
                return Err(EngineFailure::Unreachable {
                    reason: "no daemon here".to_owned(),
                });
            }

            let wanted: Vec<LogLine> = self
                .said
                .iter()
                .filter(|line| services.is_empty() || services.contains(&line.service))
                .cloned()
                .collect();

            let (sender, receiver) = tokio::sync::mpsc::channel(wanted.len().max(1));
            for line in wanted {
                let _ = sender.send(line).await;
            }
            Ok(receiver)
        }
    }

    #[async_trait]
    impl Runner for Scripted {
        async fn run(&self, _argv: &[String]) -> Result<Output, Failure> {
            match &self.0 {
                Ok(output) => Ok(output.clone()),
                Err(Failure::NotFound { program }) => Err(Failure::NotFound {
                    program: program.clone(),
                }),
                Err(Failure::Unusable { program, reason }) => Err(Failure::Unusable {
                    program: program.clone(),
                    reason: reason.clone(),
                }),
            }
        }
    }

    /// The stack this repository carries, read from disk.
    fn stack() -> Source {
        Source::External(std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/media-stack"
        )))
    }

    fn ctx(scripted: Result<Output, Failure>) -> Ctx {
        Ctx::new(
            Arc::new(Scripted(scripted)),
            Arc::new(Reporting::default()),
            Arc::new(crate::adapters::System),
            Arc::new(crate::adapters::Disk),
            stack(),
            Settings::default(),
            Environment::MacOs,
        )
    }

    fn spoke(stdout: &str) -> Output {
        Output {
            status: Some(0),
            stdout: stdout.to_owned(),
            stderr: String::new(),
        }
    }

    fn refused(stderr: &str) -> Output {
        Output {
            status: Some(1),
            stdout: String::new(),
            stderr: stderr.to_owned(),
        }
    }

    /// What a version report looks like when the engine said `compose`.
    ///
    /// Tests assert against the whole outcome rather than picking it apart. A
    /// destructuring assertion needs a branch for the case that cannot happen,
    /// and that branch is a line no test can ever cover.
    fn reported(compose: Option<&str>) -> Outcome {
        Outcome::Version(VersionReport {
            binary: env!("CARGO_PKG_VERSION").to_owned(),
            supported_schema: vec![1],
            stack: "0.1.0".to_owned(),
            compose: compose.map(str::to_owned),
        })
    }

    #[tokio::test]
    async fn reports_the_engine_version_when_the_engine_answers() {
        let ctx = ctx(Ok(spoke("v2.32.1\n")));
        assert_eq!(
            dispatch(Command::Version, &ctx).await,
            Ok(reported(Some("v2.32.1")))
        );
    }

    #[tokio::test]
    async fn still_answers_when_the_engine_is_missing() {
        let ctx = ctx(Err(Failure::NotFound {
            program: "docker".to_owned(),
        }));
        assert_eq!(dispatch(Command::Version, &ctx).await, Ok(reported(None)));
    }

    #[tokio::test]
    async fn treats_an_engine_that_fails_as_one_that_did_not_answer() {
        let ctx = ctx(Ok(refused("permission denied")));
        assert_eq!(dispatch(Command::Version, &ctx).await, Ok(reported(None)));
    }

    #[tokio::test]
    async fn an_unusable_engine_is_also_reported_as_absent() {
        let ctx = ctx(Err(Failure::Unusable {
            program: "docker".to_owned(),
            reason: "denied".to_owned(),
        }));
        assert_eq!(dispatch(Command::Version, &ctx).await, Ok(reported(None)));
    }

    #[tokio::test]
    async fn an_outcome_serialises_inside_the_versioned_envelope() {
        let ctx = ctx(Ok(spoke("v2.32.1")));
        let rendered = dispatch(Command::Version, &ctx)
            .await
            .ok()
            .and_then(|outcome| outcome.envelope().to_json());
        assert_eq!(
            rendered.as_deref(),
            Some(concat!(
                r#"{"api_version":1,"kind":"version","data":{"binary":""#,
                env!("CARGO_PKG_VERSION"),
                r#"","supported_schema":[1],"stack":"0.1.0","compose":"v2.32.1"}}"#
            ))
        );
    }

    #[tokio::test]
    async fn a_stack_that_cannot_be_read_is_reported_rather_than_left_out() {
        let nowhere = Source::External(std::path::Path::new("/lemonfiber/no/such/stack"));
        let ctx = Ctx::new(
            Arc::new(Scripted(Ok(spoke("v2.32.1")))),
            Arc::new(Reporting::default()),
            Arc::new(crate::adapters::System),
            Arc::new(crate::adapters::Disk),
            nowhere,
            Settings::default(),
            Environment::MacOs,
        );
        let refusal = dispatch(Command::Version, &ctx)
            .await
            .err()
            .map(|problem| problem.code);
        assert_eq!(
            refusal,
            Some(crate::stack::STACK_UNREADABLE),
            "an operator's own --stack-dir mistake reaches them"
        );
    }

    /// A context that runs against the checked-out stack, in rehearsal.
    fn rehearsing(protocols: crate::config::Protocols) -> Ctx {
        let settings = Settings {
            protocols,
            ..Settings::default()
        };
        Ctx::new(
            Arc::new(Scripted(Ok(spoke("")))),
            Arc::new(Reporting::default()),
            Arc::new(crate::adapters::System),
            Arc::new(crate::adapters::Disk),
            stack(),
            settings,
            Environment::MacOs,
        )
        .rehearsing()
    }

    fn report(outcome: Result<Outcome, super::Problem>) -> Option<crate::model::LifecycleReport> {
        match outcome {
            Ok(Outcome::Lifecycle(report)) => Some(report),
            Ok(
                Outcome::Version(_) | Outcome::Config(_) | Outcome::Status(_) | Outcome::Doctor(_),
            )
            | Err(_) => None,
        }
    }

    fn diagnosis(outcome: Result<Outcome, super::Problem>) -> Option<crate::model::DoctorReport> {
        match outcome {
            Ok(Outcome::Doctor(report)) => Some(report),
            Ok(
                Outcome::Version(_)
                | Outcome::Lifecycle(_)
                | Outcome::Config(_)
                | Outcome::Status(_),
            )
            | Err(_) => None,
        }
    }

    #[tokio::test]
    async fn doctor_runs_the_checks_and_reports_them_in_the_envelope() {
        // The engine here does not host the torrent pair, so the findings are
        // not green — but dispatch's job is only to run the checks and hand back
        // what they found, named in the machine-readable envelope.
        let ctx = watching(Reporting::holding(
            &LIBRARY,
            Lifecycle::Running,
            Health::Healthy,
        ));
        let command = Command::Doctor {
            only: Some(Category::Vpn),
            disruptive: false,
        };
        let outcome = dispatch(command, &ctx).await;

        let json = outcome
            .as_ref()
            .ok()
            .and_then(|outcome| outcome.clone().envelope().to_json());
        assert!(
            json.as_deref()
                .is_some_and(|json| json.contains(r#""kind":"doctor""#)
                    && json.contains(r#""category":"vpn""#)),
            "the doctor envelope should name itself and carry vpn findings: {json:?}"
        );

        let report = diagnosis(outcome);
        assert!(report.is_some_and(|report| !report.findings.is_empty()
            && report
                .findings
                .iter()
                .all(|finding| finding.category == Category::Vpn)));
    }

    #[tokio::test]
    async fn doctor_reports_an_unreadable_stack_rather_than_guessing() {
        let nowhere = Source::External(std::path::Path::new("/lemonfiber/no/such/stack"));
        let ctx = Ctx::new(
            Arc::new(Scripted(Ok(spoke("v2.32.1")))),
            Arc::new(Reporting::default()),
            Arc::new(crate::adapters::System),
            Arc::new(crate::adapters::Disk),
            nowhere,
            Settings::default(),
            Environment::MacOs,
        );
        let outcome = dispatch(
            Command::Doctor {
                only: None,
                disruptive: false,
            },
            &ctx,
        )
        .await;
        assert_eq!(
            outcome.as_ref().err().map(|problem| problem.code),
            Some(crate::stack::STACK_UNREADABLE)
        );
        assert!(diagnosis(outcome).is_none());
    }

    #[tokio::test]
    async fn starting_a_form_reports_what_it_would_run_and_runs_nothing() {
        let ctx = rehearsing(crate::config::Protocols::both());
        let command = Command::Up {
            forms: vec!["library".to_owned()],
        };
        let produced = report(dispatch(command, &ctx).await);
        assert_eq!(
            produced.map(|report| (
                report.action,
                report.profiles,
                report.rehearsed,
                report.status,
                report.command.last().cloned()
            )),
            Some((
                "up".to_owned(),
                vec!["media".to_owned()],
                true,
                None,
                Some("--detach".to_owned())
            )),
            "a rehearsal reports the command and never ran it"
        );
    }

    #[tokio::test]
    async fn what_the_configuration_left_out_is_reported_rather_than_dropped() {
        let ctx = rehearsing(crate::config::Protocols {
            usenet: true,
            torrent: false,
        });
        let command = Command::Up {
            forms: vec!["tv".to_owned()],
        };
        let produced = report(dispatch(command, &ctx).await);

        assert_eq!(
            produced.map(|report| report.dropped),
            Some(vec!["torrent".to_owned()]),
            "the operator hears which service is missing, and why"
        );
    }

    #[tokio::test]
    async fn a_real_run_reports_how_the_command_exited() {
        let settings = Settings {
            protocols: crate::config::Protocols::both(),
            ..Settings::default()
        };
        let ctx = Ctx::new(
            Arc::new(Scripted(Ok(spoke("")))),
            Arc::new(Reporting::default()),
            Arc::new(crate::adapters::System),
            Arc::new(crate::adapters::Disk),
            stack(),
            settings,
            Environment::MacOs,
        );
        let command = Command::Down {
            forms: vec!["library".to_owned()],
        };
        let produced = report(dispatch(command, &ctx).await);

        assert_eq!(
            produced.map(|report| (report.action, report.rehearsed, report.status)),
            Some(("down".to_owned(), false, Some(0)))
        );
    }

    #[tokio::test]
    async fn an_engine_that_will_not_start_is_reported_to_the_operator() {
        let settings = Settings {
            protocols: crate::config::Protocols::both(),
            ..Settings::default()
        };
        let ctx = Ctx::new(
            Arc::new(Scripted(Err(Failure::NotFound {
                program: "docker".to_owned(),
            }))),
            Arc::new(Reporting::default()),
            Arc::new(crate::adapters::System),
            Arc::new(crate::adapters::Disk),
            stack(),
            settings,
            Environment::MacOs,
        );
        let command = Command::Pull {
            forms: vec!["library".to_owned()],
        };
        let refusal = dispatch(command, &ctx)
            .await
            .err()
            .map(|problem| problem.code);
        assert_eq!(refusal, Some(crate::ports::process::MISSING_PROGRAM));
    }

    #[tokio::test]
    async fn restarting_names_the_services_and_nothing_else() {
        let ctx = rehearsing(crate::config::Protocols::both());
        let command = Command::Restart {
            forms: vec!["library".to_owned()],
            services: vec!["jellyfin".to_owned()],
        };
        let produced = report(dispatch(command, &ctx).await);

        assert_eq!(
            produced.map(|report| (report.action, report.command.last().cloned())),
            Some(("restart".to_owned(), Some("jellyfin".to_owned())))
        );
    }

    #[tokio::test]
    async fn a_form_this_stack_does_not_have_never_reaches_the_engine() {
        let ctx = rehearsing(crate::config::Protocols::both());
        let command = Command::Up {
            forms: vec!["telly".to_owned()],
        };
        let outcome = dispatch(command, &ctx).await;
        assert_eq!(
            outcome.as_ref().err().map(|problem| problem.code),
            Some(crate::stack::closure::NO_SUCH_FORM)
        );
        assert_eq!(report(outcome), None, "nothing ran, so there is no report");
    }

    #[tokio::test]
    async fn an_unreadable_stack_is_reported_before_anything_is_started() {
        let nowhere = Source::External(std::path::Path::new("/lemonfiber/no/such/stack"));
        let ctx = Ctx::new(
            Arc::new(Scripted(Ok(spoke("")))),
            Arc::new(Reporting::default()),
            Arc::new(crate::adapters::System),
            Arc::new(crate::adapters::Disk),
            nowhere,
            Settings::default(),
            Environment::MacOs,
        );
        let command = Command::Up {
            forms: vec!["library".to_owned()],
        };
        assert_eq!(
            dispatch(command, &ctx).await.err().map(|p| p.code),
            Some(crate::stack::STACK_UNREADABLE)
        );
    }

    #[tokio::test]
    async fn an_embedded_stack_with_nowhere_to_go_stops_before_starting_anything() {
        static EMBEDDED: include_dir::Dir<'_> =
            include_dir::include_dir!("$CARGO_MANIFEST_DIR/../../assets/media-stack");

        let settings = Settings {
            protocols: crate::config::Protocols::both(),
            stack_dir: None,
            ..Settings::default()
        };
        let ctx = Ctx::new(
            Arc::new(Scripted(Ok(spoke("")))),
            Arc::new(Reporting::default()),
            Arc::new(crate::adapters::System),
            Arc::new(crate::adapters::Disk),
            Source::Embedded(&EMBEDDED),
            settings,
            Environment::MacOs,
        );
        let command = Command::Up {
            forms: vec!["library".to_owned()],
        };
        assert_eq!(
            dispatch(command, &ctx).await.err().map(|p| p.code),
            Some(crate::stack::STACK_NOT_SET_UP),
            "an operator who has not run setup is told to, not shown a path error"
        );
    }

    #[tokio::test]
    async fn a_stack_that_contradicts_itself_is_refused_with_every_fault_at_once() {
        let invalid = Source::External(std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/invalid"
        )));
        let ctx = Ctx::new(
            Arc::new(Scripted(Ok(spoke("")))),
            Arc::new(Reporting::default()),
            Arc::new(crate::adapters::System),
            Arc::new(crate::adapters::Disk),
            invalid,
            Settings::default(),
            Environment::MacOs,
        );
        let command = Command::Up {
            forms: vec!["library".to_owned()],
        };

        let problem = dispatch(command, &ctx).await.err();
        assert_eq!(
            problem.as_ref().map(|problem| problem.code),
            Some(crate::stack::STACK_INVALID)
        );

        let detail = problem
            .and_then(|problem| problem.detail)
            .unwrap_or_default();
        for expected in [
            "names profile telly, which is not declared",
            "that is not a pin",
            "not a recognised OSI identifier",
        ] {
            assert!(
                detail.contains(expected),
                "missing {expected:?} in: {detail}"
            );
        }
    }

    /// A context whose settings live in a scratch file.
    fn with_config(path: &std::path::Path) -> Ctx {
        let settings = Settings {
            env_file: Some(path.to_path_buf()),
            ..Settings::default()
        };
        Ctx::new(
            Arc::new(Scripted(Ok(spoke("")))),
            Arc::new(Reporting::default()),
            Arc::new(crate::adapters::System),
            Arc::new(crate::adapters::Disk),
            stack(),
            settings,
            Environment::MacOs,
        )
    }

    fn config_scratch(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("lemonfiber-app-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join(".env")
    }

    fn settings_of(outcome: Result<Outcome, super::Problem>) -> Option<Vec<(String, String)>> {
        match outcome {
            Ok(Outcome::Config(report)) => Some(
                report
                    .settings
                    .into_iter()
                    .map(|setting| (setting.key, setting.value))
                    .collect(),
            ),
            Ok(
                Outcome::Version(_)
                | Outcome::Lifecycle(_)
                | Outcome::Status(_)
                | Outcome::Doctor(_),
            )
            | Err(_) => None,
        }
    }

    #[tokio::test]
    async fn a_setting_can_be_written_and_read_back() {
        let path = config_scratch("round-trip");
        let ctx = with_config(&path);

        let written = dispatch(
            Command::ConfigSet {
                key: "LEMONFIBER_USENET".to_owned(),
                value: "on".to_owned(),
            },
            &ctx,
        )
        .await;
        assert_eq!(
            settings_of(written),
            Some(vec![("LEMONFIBER_USENET".to_owned(), "on".to_owned())])
        );

        let read = dispatch(
            Command::ConfigGet {
                key: "LEMONFIBER_USENET".to_owned(),
            },
            &ctx,
        )
        .await;
        assert_eq!(
            settings_of(read),
            Some(vec![("LEMONFIBER_USENET".to_owned(), "on".to_owned())])
        );

        let _ = std::fs::remove_dir_all(path.parent().unwrap_or(std::path::Path::new("/")));
    }

    #[tokio::test]
    async fn a_rehearsed_change_reports_itself_and_writes_nothing() {
        let path = config_scratch("rehearsed");
        let ctx = with_config(&path).rehearsing();

        let outcome = dispatch(
            Command::ConfigSet {
                key: "LEMONFIBER_TORRENT".to_owned(),
                value: "on".to_owned(),
            },
            &ctx,
        )
        .await;
        assert!(
            matches!(&outcome, Ok(Outcome::Config(report)) if report.changed && report.rehearsed),
            "a rehearsal reports the change it would make, and that it was a rehearsal"
        );
        assert!(!path.exists(), "a rehearsal writes nothing");
    }

    #[tokio::test]
    async fn showing_settings_withholds_credentials() {
        let path = config_scratch("secrets");
        let ctx = with_config(&path);
        for (key, value) in [("DATA_ROOT", "/media"), ("WIREGUARD_PRIVATE_KEY", "abc123")] {
            let _ = dispatch(
                Command::ConfigSet {
                    key: key.to_owned(),
                    value: value.to_owned(),
                },
                &ctx,
            )
            .await;
        }

        let shown = settings_of(dispatch(Command::ConfigShow, &ctx).await).unwrap_or_default();
        assert_eq!(
            shown,
            vec![
                ("DATA_ROOT".to_owned(), "/media".to_owned()),
                (
                    "WIREGUARD_PRIVATE_KEY".to_owned(),
                    crate::config::store::REDACTED.to_owned()
                ),
            ]
        );

        let _ = std::fs::remove_dir_all(path.parent().unwrap_or(std::path::Path::new("/")));
    }

    #[tokio::test]
    async fn a_configuration_answer_serialises_under_its_own_kind() {
        let path = config_scratch("envelope");
        let ctx = with_config(&path);
        let _ = dispatch(
            Command::ConfigSet {
                key: "DATA_ROOT".to_owned(),
                value: "/media".to_owned(),
            },
            &ctx,
        )
        .await;

        let rendered = dispatch(Command::ConfigShow, &ctx)
            .await
            .ok()
            .map(Outcome::envelope)
            .and_then(|envelope| envelope.to_json().map(|json| (envelope.kind, json)));

        assert_eq!(
            rendered.map(|(kind, json)| (
                kind,
                json.starts_with(r#"{"api_version":1,"kind":"config","data":{"settings":["#),
                json.contains(r#""changed":false"#)
            )),
            Some(("config", true, true))
        );

        let _ = std::fs::remove_dir_all(path.parent().unwrap_or(std::path::Path::new("/")));
    }

    #[tokio::test]
    async fn asking_a_non_configuration_outcome_for_settings_gets_none() {
        let ctx = ctx(Ok(spoke("v2.32.1")));
        assert_eq!(settings_of(dispatch(Command::Version, &ctx).await), None);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn settings_that_cannot_be_saved_reach_the_operator() {
        use std::os::unix::fs::PermissionsExt as _;

        let dir = std::env::temp_dir().join(format!("lemonfiber-app-{}-ro", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o500));

        let ctx = with_config(&dir.join(".env"));
        let refusal = dispatch(
            Command::ConfigSet {
                key: "A".to_owned(),
                value: "1".to_owned(),
            },
            &ctx,
        )
        .await
        .err()
        .map(|problem| problem.code);
        assert_eq!(refusal, Some(crate::config::store::CONFIG_NOT_WRITTEN));

        let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn settings_that_cannot_be_read_reach_the_operator() {
        // A file where the directory holding settings should be.
        let blocker =
            std::env::temp_dir().join(format!("lemonfiber-app-{}-blocked", std::process::id()));
        let _ = std::fs::remove_dir_all(&blocker);
        let _ = std::fs::write(&blocker, "in the way");

        let ctx = with_config(&blocker.join(".env"));
        let refusal = dispatch(Command::ConfigShow, &ctx)
            .await
            .err()
            .map(|problem| problem.code);
        assert_eq!(refusal, Some(crate::config::store::CONFIG_UNREADABLE));

        let _ = std::fs::remove_file(&blocker);
    }

    #[tokio::test]
    async fn settings_with_nowhere_to_live_say_setup_has_not_run() {
        let ctx = Ctx::new(
            Arc::new(Scripted(Ok(spoke("")))),
            Arc::new(Reporting::default()),
            Arc::new(crate::adapters::System),
            Arc::new(crate::adapters::Disk),
            stack(),
            Settings::default(),
            Environment::MacOs,
        );
        assert_eq!(
            dispatch(Command::ConfigShow, &ctx)
                .await
                .err()
                .map(|p| p.code),
            Some(crate::config::store::CONFIG_NOWHERE)
        );
    }

    #[tokio::test]
    async fn a_lifecycle_outcome_serialises_under_its_own_kind() {
        let ctx = rehearsing(crate::config::Protocols::both());
        let command = Command::Up {
            forms: vec!["library".to_owned()],
        };
        let rendered = dispatch(command, &ctx)
            .await
            .ok()
            .map(Outcome::envelope)
            .and_then(|envelope| envelope.to_json().map(|json| (envelope.kind, json)));

        assert_eq!(
            rendered.map(|(kind, json)| (
                kind,
                json.starts_with(r#"{"api_version":1,"kind":"lifecycle","data":{"action":"up""#),
                json.contains(r#""rehearsed":true"#)
            )),
            Some(("lifecycle", true, true))
        );
    }

    /// A real run against an engine reporting whatever the test put in it.
    fn watching(engine: Reporting) -> Ctx {
        let settings = Settings {
            protocols: crate::config::Protocols::both(),
            ..Settings::default()
        };
        Ctx::new(
            Arc::new(Scripted(Ok(spoke("")))),
            Arc::new(engine),
            Arc::new(crate::adapters::System),
            Arc::new(crate::adapters::Disk),
            stack(),
            settings,
            Environment::MacOs,
        )
    }

    /// Everything the `library` form declares.
    const LIBRARY: [&str; 4] = [
        "jellyfin",
        "seerr",
        "calibre-web-automated",
        "audiobookshelf",
    ];

    #[tokio::test]
    async fn starting_waits_until_the_services_are_usable() {
        let engine = Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Healthy);
        let ctx = watching(engine);
        let command = Command::Up {
            forms: vec!["library".to_owned()],
        };

        let produced = report(dispatch(command, &ctx).await);
        assert_eq!(
            produced.map(|report| (
                report.condition,
                report.services.len(),
                report
                    .services
                    .iter()
                    .all(|service| service.state == ServiceState::Healthy)
            )),
            Some((Some(Condition::Active), LIBRARY.len(), true)),
            "started means every service answered, not that a process exists"
        );
    }

    #[tokio::test]
    async fn starting_keeps_asking_until_the_services_are_ready() {
        // Unsettled on the first two listings and healthy on the third, which
        // is what a stack that is genuinely starting looks like. A gate that
        // only ever read the engine once would pass this test by luck and fail
        // every real start.
        let engine =
            Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Starting).settling_after(2);
        let ctx = watching(engine);
        let command = Command::Up {
            forms: vec!["library".to_owned()],
        };

        let produced = report(dispatch(command, &ctx).await);
        assert_eq!(
            produced.map(|report| report.condition),
            Some(Some(Condition::Active)),
            "waiting is the point: the answer changed while it waited"
        );
    }

    #[tokio::test]
    async fn a_service_that_never_becomes_usable_stops_the_start_and_says_which() {
        // A container that is running but still inside its start period is
        // exactly the case a process check would have called success.
        let engine = Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Starting)
            .saying("jellyfin", "Cannot open database, disk is full");
        let ctx = watching(engine).waiting(Duration::ZERO);
        let command = Command::Up {
            forms: vec!["library".to_owned()],
        };

        let refused = dispatch(command, &ctx).await.err();
        assert_eq!(
            refused.as_ref().map(|problem| problem.code),
            Some(super::NEVER_SETTLED)
        );
        assert_eq!(
            refused
                .as_ref()
                .map(|problem| problem.summary.contains("jellyfin")),
            Some(true),
            "the operator is told which service, not that something went wrong"
        );
        assert_eq!(
            refused
                .and_then(|problem| problem.detail)
                .map(|detail| detail.contains("disk is full")),
            Some(true),
            "the explanation is already on screen rather than left to be found"
        );
    }

    #[tokio::test]
    async fn a_service_that_will_not_start_and_says_nothing_still_reports_which() {
        let engine = Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Starting);
        let ctx = watching(engine).waiting(Duration::ZERO);
        let command = Command::Up {
            forms: vec!["library".to_owned()],
        };

        let refused = dispatch(command, &ctx).await.err();
        assert_eq!(
            refused.map(|problem| (problem.code, problem.detail)),
            Some((super::NEVER_SETTLED, None)),
            "silence is reported as silence rather than as an empty quotation"
        );
    }

    #[tokio::test]
    async fn a_crash_loop_is_not_something_starting_waits_out() {
        let engine = Reporting::holding(&LIBRARY, Lifecycle::Restarting, Health::None);
        let ctx = watching(engine).waiting(Duration::from_secs(3600));
        let command = Command::Up {
            forms: vec!["library".to_owned()],
        };

        // Patience of an hour, and this must still return at once: a loop has
        // settled, and waiting for it is waiting forever.
        let produced = report(dispatch(command, &ctx).await);
        assert_eq!(
            produced.map(|report| report.condition),
            Some(Some(Condition::Degraded))
        );
    }

    #[tokio::test]
    async fn stopping_does_not_wait_for_anything() {
        let engine = Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Starting);
        let ctx = watching(engine).waiting(Duration::ZERO);
        let command = Command::Down {
            forms: vec!["library".to_owned()],
        };

        let produced = report(dispatch(command, &ctx).await);
        assert_eq!(
            produced.map(|report| (report.condition, report.services.is_empty())),
            Some((None, true)),
            "stopping is finished when Compose says so"
        );
    }

    #[tokio::test]
    async fn a_compose_invocation_that_failed_is_not_then_waited_on() {
        let settings = Settings {
            protocols: crate::config::Protocols::both(),
            ..Settings::default()
        };
        let ctx = Ctx::new(
            Arc::new(Scripted(Ok(refused("no such image")))),
            Arc::new(Reporting::holding(
                &LIBRARY,
                Lifecycle::Running,
                Health::Starting,
            )),
            Arc::new(crate::adapters::System),
            Arc::new(crate::adapters::Disk),
            stack(),
            settings,
            Environment::MacOs,
        )
        .waiting(Duration::ZERO);

        let command = Command::Up {
            forms: vec!["library".to_owned()],
        };
        let produced = report(dispatch(command, &ctx).await);
        assert_eq!(
            produced.map(|report| (report.status, report.condition)),
            Some((Some(1), None)),
            "waiting for health after Compose refused would report the wrong fault"
        );
    }

    #[tokio::test]
    async fn starting_reports_an_engine_it_cannot_see() {
        let ctx = watching(Reporting::absent());
        let command = Command::Up {
            forms: vec!["library".to_owned()],
        };
        assert_eq!(
            dispatch(command, &ctx).await.err().map(|p| p.code),
            Some(crate::ports::docker::ENGINE_UNREACHABLE)
        );
    }

    /// The status a survey produced, as pairs of service and state.
    fn stated(outcome: Result<Outcome, super::Problem>) -> Option<Vec<(String, ServiceState)>> {
        match outcome {
            Ok(Outcome::Status(report)) => Some(
                report
                    .services
                    .into_iter()
                    .map(|service| (service.id, service.state))
                    .collect(),
            ),
            Ok(
                Outcome::Version(_)
                | Outcome::Lifecycle(_)
                | Outcome::Config(_)
                | Outcome::Doctor(_),
            )
            | Err(_) => None,
        }
    }

    #[tokio::test]
    async fn a_non_status_outcome_has_no_services_to_report() {
        let ctx = ctx(Ok(spoke("v2.32.1")));
        assert_eq!(stated(dispatch(Command::Version, &ctx).await), None);
    }

    #[tokio::test]
    async fn asking_what_is_running_names_every_service_a_form_declares() {
        let engine = Reporting::holding(&["jellyfin"], Lifecycle::Running, Health::Healthy);
        let ctx = watching(engine);
        let command = Command::Ps {
            forms: vec!["library".to_owned()],
        };

        let seen = stated(dispatch(command, &ctx).await).unwrap_or_default();
        assert_eq!(seen.len(), LIBRARY.len());
        assert!(
            seen.iter()
                .any(|(id, state)| id == "jellyfin" && *state == ServiceState::Healthy),
            "{seen:?}"
        );
        assert!(
            seen.iter()
                .any(|(id, state)| id == "seerr" && *state == ServiceState::Absent),
            "a service that was never started is absent, not missing: {seen:?}"
        );
    }

    #[tokio::test]
    async fn asking_what_is_running_without_naming_a_form_covers_the_whole_stack() {
        let ctx = watching(Reporting::holding(&[], Lifecycle::Running, Health::None));
        let seen = stated(dispatch(Command::Ps { forms: Vec::new() }, &ctx).await);

        assert_eq!(
            seen.map(|services| services.len() > LIBRARY.len()),
            Some(true),
            "what is running is a question about the machine, not about a form"
        );
    }

    #[tokio::test]
    async fn asking_what_is_running_reports_an_engine_it_cannot_see() {
        let ctx = watching(Reporting::absent());
        let refusal = dispatch(Command::Ps { forms: Vec::new() }, &ctx)
            .await
            .err()
            .map(|problem| problem.code);
        assert_eq!(
            refusal,
            Some(crate::ports::docker::ENGINE_UNREACHABLE),
            "an unreachable engine is not a stack with nothing in it"
        );
    }

    #[tokio::test]
    async fn asking_about_a_form_this_stack_does_not_have_is_refused() {
        let ctx = watching(Reporting::default());
        let command = Command::Ps {
            forms: vec!["telly".to_owned()],
        };
        assert_eq!(
            dispatch(command, &ctx).await.err().map(|p| p.code),
            Some(crate::stack::closure::NO_SUCH_FORM)
        );
    }

    #[tokio::test]
    async fn asking_what_is_running_from_a_stack_that_cannot_be_read_is_refused() {
        let nowhere = Source::External(std::path::Path::new("/lemonfiber/no/such/stack"));
        let ctx = Ctx::new(
            Arc::new(Scripted(Ok(spoke("")))),
            Arc::new(Reporting::default()),
            Arc::new(crate::adapters::System),
            Arc::new(crate::adapters::Disk),
            nowhere,
            Settings::default(),
            Environment::MacOs,
        );
        assert_eq!(
            dispatch(Command::Ps { forms: Vec::new() }, &ctx)
                .await
                .err()
                .map(|problem| problem.code),
            Some(crate::stack::STACK_UNREADABLE),
            "an operator's own --stack-dir mistake reaches them here too"
        );
    }

    #[tokio::test]
    async fn a_status_serialises_under_its_own_kind() {
        let engine = Reporting::holding(&["jellyfin"], Lifecycle::Running, Health::Healthy);
        let ctx = watching(engine);
        let command = Command::Ps {
            forms: vec!["library".to_owned()],
        };

        let rendered = dispatch(command, &ctx)
            .await
            .ok()
            .map(Outcome::envelope)
            .and_then(|envelope| envelope.to_json().map(|json| (envelope.kind, json)));

        assert_eq!(
            rendered.map(|(kind, json)| (
                kind,
                json.starts_with(r#"{"api_version":1,"kind":"status","data":{"forms":["library"]"#),
                json.contains(r#""state":"healthy""#)
            )),
            Some(("status", true, true))
        );
    }

    /// The services a log stream actually carried lines for.
    async fn heard(ctx: &Ctx, forms: &[String], services: &[String]) -> Vec<String> {
        let (closed, silent) = tokio::sync::mpsc::channel(1);
        drop(closed);

        let query = LogQuery::recent(10);
        let mut lines = super::logs(ctx, forms, services, query)
            .await
            .unwrap_or(silent);

        let mut seen = Vec::new();
        while let Some(line) = lines.recv().await {
            seen.push(line.service);
        }
        seen.sort();
        seen.dedup();
        seen
    }

    #[tokio::test]
    async fn reading_logs_for_a_form_narrows_to_what_that_form_declares() {
        let engine = Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Healthy)
            .saying("jellyfin", "started")
            .saying("sonarr", "also started");
        let ctx = watching(engine);

        assert_eq!(
            heard(&ctx, &["library".to_owned()], &[]).await,
            vec!["jellyfin".to_owned()],
            "a form's log view must not carry another form's output"
        );
    }

    #[tokio::test]
    async fn naming_a_service_narrows_further_still() {
        let engine = Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Healthy)
            .saying("jellyfin", "started")
            .saying("seerr", "also started");
        let ctx = watching(engine);

        assert_eq!(
            heard(&ctx, &["library".to_owned()], &["seerr".to_owned()]).await,
            vec!["seerr".to_owned()]
        );
    }

    #[tokio::test]
    async fn naming_no_form_reads_everything_that_is_saying_anything() {
        let engine = Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Healthy)
            .saying("jellyfin", "started")
            .saying("sonarr", "also started");
        let ctx = watching(engine);

        assert_eq!(
            heard(&ctx, &[], &[]).await,
            vec!["jellyfin".to_owned(), "sonarr".to_owned()]
        );
    }

    #[tokio::test]
    async fn reading_logs_reports_an_engine_it_cannot_see() {
        let ctx = watching(Reporting::absent());
        let query = LogQuery::recent(10);
        let refusal = super::logs(&ctx, &[], &[], query)
            .await
            .err()
            .map(|problem| problem.code);
        assert_eq!(refusal, Some(crate::ports::docker::ENGINE_UNREACHABLE));
    }

    #[tokio::test]
    async fn reading_logs_for_a_form_this_stack_does_not_have_is_refused() {
        let ctx = watching(Reporting::default());
        let query = LogQuery::recent(10);
        let refusal = super::logs(&ctx, &["telly".to_owned()], &[], query)
            .await
            .err()
            .map(|problem| problem.code);
        assert_eq!(refusal, Some(crate::stack::closure::NO_SUCH_FORM));
    }

    #[tokio::test]
    async fn reading_logs_from_a_stack_that_cannot_be_read_is_refused() {
        let nowhere = Source::External(std::path::Path::new("/lemonfiber/no/such/stack"));
        let ctx = Ctx::new(
            Arc::new(Scripted(Ok(spoke("")))),
            Arc::new(Reporting::default()),
            Arc::new(crate::adapters::System),
            Arc::new(crate::adapters::Disk),
            nowhere,
            Settings::default(),
            Environment::MacOs,
        );
        let query = LogQuery::recent(10);
        assert_eq!(
            super::logs(&ctx, &[], &[], query)
                .await
                .err()
                .map(|problem| problem.code),
            Some(crate::stack::STACK_UNREADABLE)
        );
    }

    #[tokio::test]
    async fn a_context_can_be_told_how_long_to_wait() {
        let ctx = watching(Reporting::default()).waiting(Duration::from_secs(7));
        assert_eq!(ctx.patience, Duration::from_secs(7));
    }

    #[tokio::test]
    async fn the_engine_these_tests_use_answers_the_whole_port() {
        // Worth asserting rather than assuming. A fake that answers a method
        // more agreeably than a real engine would makes the path it shortcuts
        // untestable, which is how the log stream's own failure case went
        // missing until it was written down here.
        let engine = Reporting::absent();

        let ran = engine.exec("gluetun", &["true".to_owned()]).await;
        assert!(
            matches!(&ran, Err(EngineFailure::NoSuchContainer { name }) if name == "gluetun"),
            "{ran:?}"
        );

        let sampled = engine.stats("lemonfiber").await;
        assert_eq!(
            sampled.ok().map(|mut samples| samples.try_recv().is_err()),
            Some(true),
            "nothing is sampled, and the stream says so by ending"
        );
    }

    #[test]
    fn a_rehearsing_context_changes_nothing_else() {
        let rehearsal = ctx(Ok(spoke(""))).rehearsing();
        assert!(rehearsal.dry_run);
    }

    mod watching {
        use std::path::Path;
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        use async_trait::async_trait;

        use super::super::{
            supervise, Availability, Ctx, Environment, Loss, Settings, ALREADY_GONE,
            NOTHING_TO_WATCH,
        };
        use super::{spoke, stack, Output, Reporting, Scripted};
        use crate::config::Protocols;
        use crate::ports::filesystem::{Presence, Volume};

        /// A volume that answers each check with the next reading a test scripted,
        /// then stays gone once the script runs out.
        struct Drive {
            readings: Vec<Presence>,
            cursor: AtomicUsize,
        }

        impl Drive {
            fn playing(readings: Vec<Presence>) -> Self {
                Self {
                    readings,
                    cursor: AtomicUsize::new(0),
                }
            }
        }

        #[async_trait]
        impl Volume for Drive {
            async fn presence(&self, _path: &Path) -> Presence {
                let at = self.cursor.fetch_add(1, Ordering::Relaxed);
                self.readings.get(at).copied().unwrap_or(Presence::Gone)
            }
        }

        /// A context whose runner answers the stop with `result`, watching the
        /// given data location.
        fn watching(result: Result<Output, super::Failure>, data_root: Option<&str>) -> Ctx {
            let settings = Settings {
                protocols: Protocols::both(),
                data_root: data_root.map(std::path::PathBuf::from),
                ..Settings::default()
            };
            Ctx::new(
                Arc::new(Scripted(result)),
                Arc::new(Reporting::default()),
                Arc::new(crate::adapters::System),
                Arc::new(crate::adapters::Disk),
                stack(),
                settings,
                Environment::MacOs,
            )
        }

        async fn watch(
            ctx: &Ctx,
            drive: Drive,
        ) -> Result<crate::model::SupervisionReport, super::super::Problem> {
            supervise(
                ctx,
                &drive,
                &["library".to_owned()],
                std::time::Duration::ZERO,
            )
            .await
        }

        #[test]
        fn a_reading_is_judged_against_the_volume_the_watch_started_on() {
            assert!(matches!(
                super::super::assess(9, Presence::Gone),
                Availability::Lost(Loss::Vanished)
            ));
            assert!(matches!(
                super::super::assess(9, Presence::On(9)),
                Availability::Holding
            ));
            assert!(matches!(
                super::super::assess(9, Presence::On(4)),
                Availability::Lost(Loss::Moved)
            ));
        }

        #[tokio::test]
        async fn a_watch_with_no_data_location_says_there_is_nothing_to_watch() {
            let ctx = watching(Ok(spoke("")), None);
            let refused = watch(&ctx, Drive::playing(vec![])).await.err();
            assert_eq!(refused.map(|problem| problem.code), Some(NOTHING_TO_WATCH));
        }

        #[tokio::test]
        async fn a_location_already_gone_when_the_watch_begins_will_not_start() {
            let ctx = watching(Ok(spoke("")), Some("/data"));
            let refused = watch(&ctx, Drive::playing(vec![Presence::Gone]))
                .await
                .err();
            assert_eq!(refused.map(|problem| problem.code), Some(ALREADY_GONE));
        }

        #[tokio::test]
        async fn a_data_root_that_vanishes_stops_the_services() {
            let ctx = watching(Ok(spoke("")), Some("/data"));
            let report = watch(&ctx, Drive::playing(vec![Presence::On(9), Presence::Gone]))
                .await
                .ok();
            assert_eq!(
                report.map(|report| (report.stopped, report.reason.contains("no longer present"))),
                Some((true, true))
            );
        }

        #[tokio::test]
        async fn a_data_root_that_holds_before_it_is_lost_keeps_checking() {
            let ctx = watching(Ok(spoke("")), Some("/data"));
            let report = watch(
                &ctx,
                Drive::playing(vec![Presence::On(9), Presence::On(9), Presence::Gone]),
            )
            .await
            .ok();
            assert_eq!(report.map(|report| report.stopped), Some(true));
        }

        #[tokio::test]
        async fn a_data_root_that_becomes_a_different_volume_is_a_loss() {
            let ctx = watching(Ok(spoke("")), Some("/data"));
            let report = watch(&ctx, Drive::playing(vec![Presence::On(9), Presence::On(4)]))
                .await
                .ok();
            assert_eq!(
                report.map(|report| report.reason.contains("different volume")),
                Some(true)
            );
        }

        #[tokio::test]
        async fn services_that_cannot_be_stopped_are_reported_not_hidden() {
            let ctx = watching(
                Err(super::Failure::NotFound {
                    program: "docker".to_owned(),
                }),
                Some("/data"),
            );
            let report = watch(&ctx, Drive::playing(vec![Presence::On(9), Presence::Gone]))
                .await
                .ok();
            assert_eq!(report.map(|report| report.stopped), Some(false));
        }
    }
}
