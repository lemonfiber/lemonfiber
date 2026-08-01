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

use crate::config::Settings;
use crate::platform::Environment;
use crate::ports::docker::Engine;
use crate::ports::http::Http;
use crate::ports::random::Random;
use crate::ports::{Clock, FileSystem, Runner};
use crate::stack::Source;

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
    /// How services are reached over HTTP, for the checks and seeding that ask
    /// one what it is or wire it to another.
    pub http: Arc<dyn Http>,
    /// Where unpredictable bytes come from, for the one credential seeding mints
    /// itself.
    pub random: Arc<dyn Random>,
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
            // The real transport is the only sensible default; the one code path
            // that needs to answer for a fake service overrides it with
            // `with_http`, so no test reaches the network to build a context.
            http: Arc::new(crate::adapters::Web::new()),
            random: Arc::new(crate::adapters::Os),
            patience: PATIENCE,
            stack,
            settings,
            environment,
        }
    }

    /// The same context, reaching services over the given transport.
    ///
    /// The seam seeding is driven through in a test: a fake here answers as a
    /// service would, so wiring is exercised with nothing running.
    #[must_use]
    pub fn with_http(mut self, http: Arc<dyn Http>) -> Self {
        self.http = http;
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
