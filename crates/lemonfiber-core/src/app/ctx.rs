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
use crate::ports::docker::{Engine, Images};
use crate::ports::filesystem::{Eraser, Storage, Volume};
use crate::ports::hosting::Host;
use crate::ports::http::Http;
use crate::ports::machine::{Started, Supply};
use crate::ports::narration::Silent;
use crate::ports::network::Site;
use crate::ports::nntp::Nntp;
use crate::ports::occupancy::Occupancy;
use crate::ports::random::Random;
use crate::ports::seams::Seams;
use crate::ports::{Clock, FileSystem, Narrator, Runner};
use crate::stack::Source;
use crate::validate::{Live, Validator};
use crate::walkthrough::{Narrator as Stepwise, Unheard};

mod moment;

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
    /// How the engine is asked what it has pulled, and who is standing on each of
    /// them.
    ///
    /// Apart from the engine because the question is apart: one command asks it, and
    /// every other reading of the engine asks the rest and never this.
    pub images: Arc<dyn Images>,
    /// What time it is, for the one rule that depends on it.
    pub clock: Arc<dyn Clock>,
    /// How the filesystem is reached, for the checks that prove what it can do.
    pub filesystem: Arc<dyn FileSystem>,
    /// How a path is asked whether it is still there, and still the same volume.
    ///
    /// Apart from the filesystem because the question is apart: a guard asks this
    /// and nothing else, and every other check asks the rest and never this.
    pub volume: Arc<dyn Volume>,
    /// How a directory and everything beneath it is removed.
    ///
    /// Apart from the filesystem for the reason the volume is: the one command that
    /// asks needs nothing else of a filesystem, and this is the one operation here
    /// that cannot be undone.
    pub eraser: Arc<dyn Eraser>,
    /// How a tree is walked to find out what is actually in it.
    ///
    /// Apart from the filesystem for the reason the other two are: the reckoning
    /// that asks needs nothing else of a filesystem, and every other implementation
    /// of the wider trait would gain a method it never calls.
    pub occupancy: Arc<dyn Occupancy>,
    /// How this machine is asked to keep a long-running command running.
    ///
    /// A port because which service manager a platform has, and what it says when
    /// asked to load something, are the two facts a test can never settle for
    /// itself — and because a run that decided them by asking the operating system
    /// directly would have exactly one of its three answers reachable from any one
    /// machine.
    pub hosting: Arc<dyn Host>,
    /// How services are reached over HTTP, for the checks and seeding that ask
    /// one what it is or wire it to another.
    pub http: Arc<dyn Http>,
    /// Where unpredictable bytes come from, for the one credential seeding mints
    /// itself.
    pub random: Arc<dyn Random>,
    /// How this machine is asked where it is, for the one answer that is an
    /// address rather than a state: what to hand somebody who lives here.
    ///
    /// A port because both halves of that answer — what this machine is called and
    /// which address a device in the same house reaches it at — are facts about
    /// one particular machine, so a test written against the real ones would pass
    /// where it was written and nowhere else.
    pub site: Arc<dyn Site>,
    /// How this machine is asked when it last started.
    ///
    /// A port beside [`Self::site`] and for the same reason: it is a fact about one
    /// particular machine at one particular moment, so a test written against the
    /// real one would pass where it was written and nowhere else. It is what lets a
    /// run tell "this machine has restarted since I last looked" from "it has not",
    /// which nothing in this workspace could say before.
    pub started: Arc<dyn Started>,
    /// How this machine is asked where its power is coming from.
    ///
    /// Beside the one above, built over the same runner, and read by exactly one
    /// decision: whether a start nobody asked for should happen while a laptop is on
    /// its battery. A media stack started on battery empties one in an afternoon,
    /// which is a thing to have chosen rather than a thing to discover.
    pub power: Arc<dyn Supply>,
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
    /// How a Usenet provider is reached, which is a connection rather than a request.
    ///
    /// Kept because replacing the transport rebuilds the validator, and a validator
    /// rebuilt without this would prove an indexer key and report a Usenet login
    /// unreachable — which reads as a broken provider rather than as a missing seam.
    pub nntp: Arc<dyn Nntp>,
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
fn live(http: &Arc<dyn Http>, nntp: Arc<dyn Nntp>, reaching: Reaching) -> Arc<dyn Validator> {
    Arc::new(crate::validate::Allowed::new(
        Arc::new(Live::with_nntp(Arc::clone(http), nntp)),
        reaching,
    ))
}

impl Ctx {
    /// The filesystem as something that can only be asked what it *is*.
    ///
    /// The same object, held as the narrower trait. A caller that needs to know whether
    /// a path can hold a hardlink gets exactly that and no way to write, remove, or link
    /// anything — which is what lets a migration survey report what somebody's layout
    /// costs without being able to rearrange it.
    #[must_use]
    pub fn storage(&self) -> Arc<dyn Storage> {
        Arc::clone(&self.filesystem) as Arc<dyn Storage>
    }
    /// A context that runs programs for real, against a given stack.
    #[must_use]
    pub fn new(
        runner: Arc<dyn Runner>,
        engine: Arc<dyn Engine>,
        clock: Arc<dyn Clock>,
        seams: Seams,
        stack: Source,
        settings: Settings,
        environment: Environment,
    ) -> Self {
        // Handed over rather than built here. The implementations live in a crate this
        // one does not depend on, so a context cannot manufacture a socket and a caller
        // that means a fake says so by name.
        let Seams {
            filesystem,
            http,
            images,
            volume,
            eraser,
            occupancy,
            hosting,
            random,
            nntp,
        } = seams;
        // Built here rather than handed over, because it is written over the runner
        // rather than over the machine: asking this machine its name means running a
        // program, and which program runner that is, is this context's answer already.
        let site: Arc<dyn Site> = Arc::new(crate::network::Here::over(Arc::clone(&runner)));
        // One object answering both questions about this machine's state, held as the
        // two seams that ask them. Built here rather than handed over for the reason
        // the site is: asking means running a program, and which program runner that
        // is, is this context's answer already.
        let asking = Arc::new(crate::machine::Asking::over(Arc::clone(&runner)));
        let started: Arc<dyn Started> = Arc::clone(&asking) as Arc<dyn Started>;
        let power: Arc<dyn Supply> = asking as Arc<dyn Supply>;
        Self {
            dry_run: false,
            force: false,
            runner,
            engine,
            images,
            clock,
            filesystem,
            volume,
            eraser,
            occupancy,
            hosting,
            validator: live(&http, Arc::clone(&nntp), settings.reaching.clone()),
            nntp,
            http,
            random,
            site,
            started,
            power,
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

    /// The same context, listing images through the given seam.
    ///
    /// Lets what an uninstall would take be driven against an engine a test wrote
    /// down, so an image another project is standing on is exercised with one
    /// daemon and no second project.
    #[must_use]
    pub fn with_images(mut self, images: Arc<dyn Images>) -> Self {
        self.images = images;
        self
    }

    /// The same context, removing through the given eraser.
    ///
    /// The one seam a test cannot let out into a real filesystem and still be a
    /// test: what it removes does not come back.
    #[must_use]
    pub fn erasing(mut self, eraser: Arc<dyn Eraser>) -> Self {
        self.eraser = eraser;
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
        self.validator = live(
            &http,
            Arc::clone(&self.nntp),
            self.settings.reaching.clone(),
        );
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
    /// The same context, writing down what leaves this machine.
    ///
    /// Given a path rather than finding one: where this machine keeps its files is
    /// the edge's question, and a core that answered it would be a core that knows
    /// what an operating system is.
    ///
    /// Wrapped **outside** whatever is already there, so what is written down is
    /// what actually left rather than what a caller asked for — three attempts at
    /// one request are three things that went, and an operator checking what was
    /// sent is owed all three.
    pub fn recording_at(self, at: std::path::PathBuf) -> Self {
        let http: Arc<dyn Http> = Arc::new(crate::recording::Recording::around(
            Arc::clone(&self.http),
            Some(at),
            Arc::clone(&self.clock),
        ));
        self.with_http(http)
    }

    /// The same context, taking its randomness from the given seam.
    #[must_use]
    pub fn with_random(mut self, random: Arc<dyn Random>) -> Self {
        self.random = random;
        self
    }

    /// The same context, asking the given seam where this machine is.
    ///
    /// Lets the address a household is handed be asserted, which the real one
    /// cannot be: what this machine is called and which address it answers on are
    /// different on every machine that runs the tests.
    #[must_use]
    pub fn with_site(mut self, site: Arc<dyn Site>) -> Self {
        self.site = site;
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

    /// The same context, walking trees through the given seam.
    ///
    /// Lets a reckoning be driven over a tree a test described, so what a full disk
    /// comes to is exercised without one.
    #[must_use]
    pub fn surveying(mut self, occupancy: Arc<dyn Occupancy>) -> Self {
        self.occupancy = occupancy;
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

    /// The same context, asking the given seam when this machine last started.
    ///
    /// So a test can drive the one decision that turns on it — whether this run is
    /// the first since a restart — without a machine that has actually restarted,
    /// which no test can arrange.
    #[must_use]
    pub fn with_started(mut self, started: Arc<dyn Started>) -> Self {
        self.started = started;
        self
    }

    /// The same context, asking the given seam where the power comes from.
    ///
    /// For the reason above: the machine a test runs on is plugged into whatever it
    /// is plugged into, and both answers have to be exercised from it.
    #[must_use]
    pub fn with_power(mut self, power: Arc<dyn Supply>) -> Self {
        self.power = power;
        self
    }

    /// The same context, handing long-running commands to the given manager.
    ///
    /// The default is the one that configures nothing, because which manager this
    /// machine has is resolved where the platform is read and handed in — leaving
    /// a context nobody told answering honestly that it hosts nothing rather than
    /// guessing at a manager.
    #[must_use]
    pub fn hosting_with(mut self, hosting: Arc<dyn Host>) -> Self {
        self.hosting = hosting;
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

/// How long starting waits for every service to settle.
///
/// Long enough for the slowest first run on a spinning disk, and bounded
/// because a wait with no end is indistinguishable from a hang.
///
/// Reachable because two other places state this length rather than choose it:
/// the sample the contract publishes, and the test that reads back the sentences
/// a wait produces. Both were a hand-copied `180` and would have gone on passing
/// after this was tuned, saying a length no run was held to.
pub const PATIENCE: Duration = Duration::from_secs(180);
