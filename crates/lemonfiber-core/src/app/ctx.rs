//! The context a command runs against: the capabilities it reaches the outside
//! world through, and what the operator chose.
//!
//! Everything a [`super::dispatch`] needs that is not part of the command itself
//! lives here — the ports held as `Arc<dyn …>` so a test can hand a command a fake
//! world, and the settings and environment the surface resolved. Whether a run is
//! a rehearsal is a field, not a second code path, so there is no parallel
//! implementation to fall out of step.

use std::sync::Arc;
use std::time::Duration;

use crate::archive::Archiving;
use crate::config::{Reaching, Settings};
use crate::platform::Environment;
use crate::ports::docker::Engine;
use crate::ports::filesystem::Volume;
use crate::ports::http::Http;
use crate::ports::narration::Silent;
use crate::ports::random::Random;
use crate::ports::{Clock, FileSystem, Narrator, Runner};
use crate::stack::Source;
use crate::validate::{Live, Validator};
use crate::walkthrough::{Narrator as Stepwise, Unheard};

/// Everything a command needs that is not part of the command itself.
pub struct Ctx {
    /// Whether to report what would happen and change nothing.
    pub dry_run: bool,
    /// Whether this run takes the stack from whatever already claimed it.
    pub force: bool,
    /// How programs are run.
    pub runner: Arc<dyn Runner>,
    /// How the engine is observed.
    pub engine: Arc<dyn Engine>,
    /// What time it is, for the one rule that depends on it.
    pub clock: Arc<dyn Clock>,
    /// How the filesystem is reached, for the checks that prove what it can do.
    pub filesystem: Arc<dyn FileSystem>,
    /// How a path is asked whether it is still there, and still the same volume.
    ///
    /// Apart from the filesystem because the question is apart: a guard asks this
    /// and nothing else, and every other check asks the rest and never this.
    pub volume: Arc<dyn Volume>,
    /// How services are reached over HTTP, for the checks and seeding that ask
    /// one what it is or wire it to another.
    pub http: Arc<dyn Http>,
    /// Where unpredictable bytes come from, for the one credential seeding mints
    /// itself.
    pub random: Arc<dyn Random>,
    /// How a credential is proven against the service it authenticates to.
    ///
    /// A port because setup proves one the moment it is entered, on every surface:
    /// a browser submitting an indexer key gets the same live test a terminal run
    /// gives it, and neither is trusted to say for itself that a key works.
    pub validator: Arc<dyn Validator>,
    /// Where a wait says what it is waiting for, for the surface to render.
    ///
    /// A port for the same reason printing is not done here: the core has no
    /// terminal, and a wait that reached for one would be a wait only the command
    /// line could have. A run whose surface is not listening holds a narrator that
    /// says nothing, so there is no second code path for the case where nobody is.
    pub narrator: Arc<dyn Narrator>,
    /// Where a walk says what it has just done, for the surface to render.
    ///
    /// Beside the narrator rather than the same port, because the two carry
    /// different things: a wait says one sentence, and a walk says a step, a
    /// phrase and the evidence for it. Rendering the step into a sentence here
    /// would put the walk's words in the core and the terminal's copy of them in
    /// the binary, which is two accounts of one run.
    pub steps: Arc<dyn Stepwise>,
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
    /// Where this run keeps archives, and what writes them.
    ///
    /// Optional because where lemonfiber's own files live is the surface's answer
    /// and the surface can fail to give one: a machine that will not say where a
    /// configuration home is has no backups directory, and the commands that need
    /// one refuse rather than guessing at a path to write over.
    pub archives: Option<Archiving>,
}

/// A validator proving credentials against the real services, over `http` and over
/// a real NNTP dialer — and only where this machine's settings allow the request.
///
/// Both transports, because setup proves a Usenet login as well as an indexer key,
/// and a validator with no transport for one reports it unreachable rather than
/// pretending it was proven. Wrapped in what the operator permits, so a credential
/// whose proof would leave this machine against their settings is recorded unproven
/// instead of being proven anyway.
fn live(http: &Arc<dyn Http>, reaching: Reaching) -> Arc<dyn Validator> {
    Arc::new(crate::validate::Allowed::new(
        Arc::new(Live::with_nntp(
            Arc::clone(http),
            Arc::new(crate::adapters::Dialer::new()),
        )),
        reaching,
    ))
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
        // The real transport is the only sensible default; the one code path that
        // needs to answer for a fake service overrides it with `with_http`, so no
        // test reaches the network to build a context.
        //
        // Wrapped so a service that is merely still starting is tried again rather
        // than reported. Applied here rather than at each caller: a retry policy
        // written into fifteen call sites is fifteen policies.
        let http: Arc<dyn Http> = Arc::new(crate::adapters::Retrying::around(
            crate::adapters::Web::new(),
        ));
        Self {
            dry_run: false,
            force: false,
            runner,
            engine,
            clock,
            filesystem,
            // The real volume for the same reason the transport is real: the one
            // command that asks is asking about this machine's drives, and a test
            // that means something else says so by name.
            volume: Arc::new(crate::adapters::Disk),
            validator: live(&http, settings.reaching.clone()),
            http,
            random: Arc::new(crate::adapters::Os),
            // Nobody, until a surface says otherwise. A context is built before the
            // thing that would listen exists in both surfaces, and a default that
            // said something would have to guess where.
            narrator: Arc::new(Silent),
            // Nobody either, and for the same reason.
            steps: Arc::new(Unheard),
            patience: PATIENCE,
            stack,
            settings,
            environment,
            // Nowhere, until a surface says where. Resolving the configuration
            // home means asking the operating system, which is the surface's
            // half of this and not something a default could stand in for.
            archives: None,
        }
    }

    /// The same context, keeping its archives where this says.
    ///
    /// How a surface hands over the two things a capture or a restore needs and no
    /// other command does: the layout to write into, and the adapter that turns
    /// trees into a `.tar.gz`. Given here rather than at [`Self::new`] because the
    /// packing lives in the binary, and a core that took it as a required argument
    /// would be a core every test had to hand an archiver it never uses.
    #[must_use]
    pub fn keeping(mut self, archives: Archiving) -> Self {
        self.archives = Some(archives);
        self
    }

    /// The same context, reaching services over the given transport.
    ///
    /// The seam seeding is driven through in a test: a fake here answers as a
    /// service would, so wiring is exercised with nothing running.
    ///
    /// Proving a credential goes over the same transport, so this replaces the
    /// validator too — a context told to reach services through a fake and still
    /// proving keys against the real internet would be reaching the network from a
    /// test that said it was not. A caller that wants to script the outcomes
    /// themselves says so with [`Self::proving`], afterwards.
    #[must_use]
    pub fn with_http(mut self, http: Arc<dyn Http>) -> Self {
        self.validator = live(&http, self.settings.reaching.clone());
        self.http = http;
        self
    }

    /// The same context, proving credentials through the given validator.
    ///
    /// For a caller that wants the outcome itself rather than the service that
    /// produces one: a test naming what a rejected key comes to says so here
    /// instead of scripting the answer an indexer would have given.
    #[must_use]
    pub fn proving(mut self, validator: Arc<dyn Validator>) -> Self {
        self.validator = validator;
        self
    }

    /// The same context, drawing randomness from the given source.
    ///
    /// Lets a test script the bytes a generated secret is rendered from, so the
    /// value it produces is known rather than unpredictable.
    #[must_use]
    pub fn with_random(mut self, random: Arc<dyn Random>) -> Self {
        self.random = random;
        self
    }

    /// The same context, reaching the filesystem through the given seam.
    ///
    /// Lets seeding's key-reading be driven from a fake that hands back a
    /// configuration without a service ever having written one.
    #[must_use]
    pub fn with_filesystem(mut self, filesystem: Arc<dyn FileSystem>) -> Self {
        self.filesystem = filesystem;
        self
    }

    /// The same context, saying what it waits for through the given narrator.
    ///
    /// How a surface hears a wait: the command line puts the words under the command
    /// it is running, and the web surface says them on the stream a browser already
    /// holds open. Neither reaches into the wait, and the wait knows about neither.
    #[must_use]
    pub fn narrating(mut self, narrator: Arc<dyn Narrator>) -> Self {
        self.narrator = narrator;
        self
    }

    /// The same context, asking the given seam whether a path is still there.
    ///
    /// Lets a guard be driven against a drive a test scripted, so what a watch
    /// does when a volume is swapped out under it is exercised with nothing
    /// unplugged.
    #[must_use]
    pub fn with_volume(mut self, volume: Arc<dyn Volume>) -> Self {
        self.volume = volume;
        self
    }

    /// The same context, saying what a walk has done through the given narrator.
    ///
    /// How a surface watches a walk: the command line puts each step under the
    /// command it is running, and the web surface says it on the stream a browser
    /// already holds open.
    #[must_use]
    pub fn narrating_steps(mut self, steps: Arc<dyn Stepwise>) -> Self {
        self.steps = steps;
        self
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
    /// The moment now, as the opaque stamp durable records carry.
    ///
    /// Seconds since the epoch, read through the clock port rather than from the
    /// system directly, so a test can say what time it is and a record written on
    /// one run can be compared with one written on another.
    pub(super) fn stamp(&self) -> String {
        self.clock
            .now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_secs().to_string())
            .unwrap_or_default()
    }

    pub(super) fn today(&self) -> lemonfiber_manifest::Date {
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

    /// The same context, taking the stack from whatever claimed it.
    ///
    /// A per-run decision rather than a setting, and carried here for the same reason
    /// a rehearsal is: it is true of everything this invocation does, and threading it
    /// through every call that might eventually reach a lock would put it in signatures
    /// that have nothing to do with it.
    #[must_use]
    pub fn forcing(mut self) -> Self {
        self.force = true;
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
