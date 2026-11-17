//! The world every example is answered in.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use lemonfiber::archive::Tar;
use lemonfiber_core::admission::Credential;
use lemonfiber_core::app::Ctx;
use lemonfiber_core::archive::Archiving;
use lemonfiber_core::config::paths::Paths;
use lemonfiber_core::config::Settings;
use lemonfiber_core::pass::{self, KEY_BYTES};
use lemonfiber_core::plugin::configuration;
use lemonfiber_core::ports::docker::{Health, Lifecycle};
use lemonfiber_core::ports::filesystem::Presence;
use lemonfiber_core::ports::hosting::Manager;
use lemonfiber_core::screening::door::DOOR;
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::erasing::Erasing;
use lemonfiber_fixtures::hosting::Hosting;
use lemonfiber_fixtures::http::Fake;
use lemonfiber_fixtures::located::Located;
use lemonfiber_fixtures::mounted::Drive;
use lemonfiber_fixtures::passwords::a_password;
use lemonfiber_fixtures::ports::{Chance, Stopped};
use lemonfiber_fixtures::pulled::Pulled;
use lemonfiber_fixtures::support::{spoke, Reporting, Scripted};
use lemonfiber_fixtures::walking::Walking;
use lemonfiber_testing::adapted;

use crate::disk::kept;
use crate::house::PARTS;
use serde_json::{json, Value};

/// Where every world writes the repository's stack, beneath its own directory.
const STACK: &str = "stack";

/// The domain the household reaches the front door at.
const HOUSEHOLD_DOMAIN: &str = "home.lan";

/// The bytes of the key every pass is sealed with: none random, so every pass is the same
/// on every run.
const PASS_KEY: [u8; KEY_BYTES] = [9; KEY_BYTES];

/// A line the engine writes while it pulls an image.
const PULLED: &str = "sonarr Pulled";

/// The instant every world's clock is stopped at.
pub(crate) const AT: u64 = 1_893_456_000;

/// Where every world keeps its directory: the same path on every machine, so what an
/// answer derives from a path is the same on every machine too.
pub(crate) const ROOT: &str = "/tmp/lemonfiber-examples";

/// What a world's own directory is written as wherever an answer names it.
pub(crate) const STAND_IN: &str = "/srv/lemonfiber";

/// Where the household reaches this machine.
const HOUSEHOLD_HOST: &str = "192.168.1.20";

/// The run-and-exit service a scheduling world holds, and the plugin holding it.
pub(crate) const SCHEDULED: &str = "configarr";

/// The times that service runs at: every minute, so one fell due whatever the zone.
const EVERY_MINUTE: &str = "* * * * *";

/// The services every world's engine reports running and healthy.
const RUNNING: &[&str] = &["jellyfin", "sonarr", "radarr", "prowlarr", "qbittorrent"];

/// One world: a context over the core's fakes, and the directory it keeps its files in.
pub(crate) struct World {
    /// The context a command runs against.
    pub(crate) ctx: Ctx,
    /// The directory of this world's own.
    root: PathBuf,
}

impl World {
    /// A world of its own for the example called `name`.
    pub(crate) fn named(name: &str) -> Self {
        let root = Path::new(ROOT).join(name);
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::create_dir_all(root.join("data"));
        let state = root.join("state");
        let _ = std::fs::create_dir_all(state.join("companion"));
        let stack = root.join(STACK);
        let admission = state.join("admission");
        lemonfiber_fixtures::stack::directory_into(&stack);
        let settings = Settings {
            env_file: Some(root.join(".env")),
            stack_dir: Some(stack.clone()),
            data_root: Some(root.join("data")),
            storage_state: Some(state.join("storage")),
            admission: Some(admission.clone()),
            companion: Some(state.join("companion")),
            household_host: Some(HOUSEHOLD_HOST.to_owned()),
            program: Some(root.join("lemonfiber")),
            hosted: Some(root.join("hosted")),
            ..Settings::default()
        };
        let ctx = lemonfiber_testing::a_context()
            .engine(Arc::new(running()))
            .clock(Stopped::at(AT))
            .filesystem(Arc::new(kept()))
            .images(Pulled::holding(Vec::new()))
            .over(Source::External(Box::leak(stack.into_boxed_path())))
            .settings(settings)
            .build()
            .with_random(Arc::new(Chance::cycling()))
            .with_hosting(Hosting::with(Manager::Systemd))
            .with_eraser(Erasing::willing())
            .with_locations(Located::holding(&[]))
            .with_occupancy(Walking::holding(Vec::new()))
            .with_archives(Archiving {
                paths: Paths::at(&root, &root),
                vault: Arc::new(Tar),
            })
            .recording_at(root.join("outbound.log"));
        let _ = Credential::set(&a_password(), &Chance::cycling())
            .map(|held| lemonfiber_core::admission::keep(&admission, &held));
        Self { ctx, root }
    }

    /// The same world with nothing running in it.
    #[must_use]
    pub(crate) fn stopped(mut self) -> Self {
        self.ctx.seams.engine =
            Arc::new(Reporting::holding(RUNNING, Lifecycle::Exited, Health::None));
        self
    }

    /// The same world, its front door serving the household at [`HOUSEHOLD_DOMAIN`] with a
    /// certificate of its own and its pass service holding its key, and every part the
    /// bundle installs recorded, so a title and a channel are located at the door.
    #[must_use]
    pub(crate) fn passing(mut self) -> Self {
        self.ctx.settings.household_domain = Some(HOUSEHOLD_DOMAIN.to_owned());
        adapted::parts_recorded(&self.ctx);
        let project = self.root.join(STACK);
        crate::companion::certified(&project.join(DOOR));
        let pass = configuration(&project, pass::SERVICE);
        let _ = std::fs::create_dir_all(&pass);
        let _ = std::fs::write(
            pass.join(pass::File::Key.name()),
            pass::Key::of(PASS_KEY).written(),
        );
        self
    }

    /// The same world, its engine writing [`PULLED`] for every command it is given.
    #[must_use]
    pub(crate) fn pulling(mut self) -> Self {
        self.ctx.seams.runner = Arc::new(Scripted(Ok(spoke(PULLED))));
        self
    }

    /// The same world, its data location there for one look and gone after it.
    #[must_use]
    pub(crate) fn unmounting(mut self) -> Self {
        self.ctx = self
            .ctx
            .clone()
            .with_volume(Arc::new(Drive::playing(vec![Presence::On(1)])));
        self
    }

    /// The same world, its household's media server, curators and request service moved
    /// into their plugins and answering through `transport`.
    #[must_use]
    pub(crate) fn housing(mut self, transport: Arc<Fake>) -> Self {
        self.ctx = adapted::asking(self.ctx.clone().with_http(transport), &PARTS);
        self.ctx.seams.engine = Arc::new(running().alongside(adapted::running(&PARTS)));
        for part in PARTS {
            part.moved_in(&self.ctx);
        }
        let _ = lemonfiber_core::config::store::set(
            &self.root.join(".env"),
            lemonfiber_core::config::IDENTITY_SOURCE_ADMIN_PASSWORD_KEY,
            &a_password(),
        );
        self
    }

    /// The same world, its plugins recorded as one holding [`SCHEDULED`], run to completion
    /// at [`EVERY_MINUTE`].
    #[must_use]
    pub(crate) fn scheduling(self) -> Self {
        let register = json!({ "installed": [{
            "plugin": SCHEDULED,
            "version": "1.0.0",
            "services": [{
                "service": SCHEDULED,
                "image": "example.invalid/configarr",
                "digest": "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945",
                "tag": "1.0.0",
                "config_path": "/config",
                "takes_data": false,
                "fields": { "schedule": {
                    "setting": format!("{SCHEDULED}:schedule"),
                    "default": EVERY_MINUTE,
                } },
            }],
        }] });
        let _ = std::fs::write(
            self.root.join(lemonfiber_core::config::paths::PLUGINS),
            register.to_string(),
        );
        self
    }

    /// The same world, its clock stopped at `seconds` since the epoch.
    #[must_use]
    pub(crate) fn at(mut self, seconds: u64) -> Self {
        self.ctx.seams.clock = Stopped::at(seconds);
        self
    }

    /// This world's own settings holding `value` for `setting`.
    pub(crate) fn trusting(&self, setting: &str, value: &str) {
        let kept = json!({ "schema": 1, "written_by": "0.0.0", "values": { setting: value } });
        let _ = std::fs::write(self.root.join("settings.json"), kept.to_string());
    }

    /// The directory of this world's own.
    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    /// `answer` as it reads, with this world's directory placed.
    pub(crate) fn read(&self, answer: Option<String>) -> Option<Value> {
        serde_json::from_str(&self.placed(&answer?)).ok()
    }

    /// `said` with this world's directory written as [`STAND_IN`], so what an answer
    /// says is the same on every machine.
    pub(crate) fn placed(&self, said: &str) -> String {
        let canonical = self.root.canonicalize();
        let canonical = canonical.as_deref().unwrap_or(self.root.as_path());
        said.replace(&shown(canonical), STAND_IN)
            .replace(&shown(&self.root), STAND_IN)
    }
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// The engine's account of the stack's services, running and healthy.
fn running() -> Reporting {
    Reporting::holding(RUNNING, Lifecycle::Running, Health::Healthy).saying_at(
        "sonarr",
        "2029-12-31T23:59:59Z",
        "Application started.",
    )
}

/// A path as an answer writes it.
fn shown(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}
