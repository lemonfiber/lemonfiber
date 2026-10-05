//! Replacing a media \*arr's own key and handing the new one to every copy.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;

use super::{asked, ctx, env_at, recorded, sealed, the_service_key, SERVICE_CONFIG};
use lemonfiber_adapters::Disk;
use lemonfiber_core::app::Asking;
use lemonfiber_core::config::{store, Settings};
use lemonfiber_core::credential::{Inventory, Reach, Settled};
use lemonfiber_core::ports::filesystem::{
    Fault, FileSystem, Identity, Ownership, Storage, StorageFacts,
};
use lemonfiber_core::ports::http::Method;
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::files::Files;
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::support::Reporting;
use lemonfiber_sidecar::gate::{Credential, Kind, Upstream, Upstreams};

/// A Servarr status body, as a healthy service answers `system/status` with.
const SONARR_STATUS: &str = r#"{"instanceName":"Sonarr","version":"4.0.20.2967"}"#;

/// Prowlarr's application for Sonarr, its stored key masked.
const PROWLARR_HOLDS: &str = r#"[{"id":7,"name":"Sonarr","fields":[{"name":"baseUrl","value":"http://sonarr:8989"},{"name":"apiKey","value":"********"}]}]"#;

/// The key Sonarr writes when it replaces [`the_service_key`], split for the same
/// reason that one is.
fn the_new_key() -> String {
    format!("{}{}", "9999eeee", "77776666aaaa")
}

/// Sonarr's configuration once it has replaced its key.
fn replaced_config() -> String {
    format!("<Config><ApiKey>{}</ApiKey></Config>", the_new_key())
}

/// The subtitle finder's configuration, holding its own key under `auth`.
const BAZARR_CONFIG: &str = "auth:\n  apikey: bazarrkeybazarrkey\n";

/// A filesystem holding each \*arr's configuration as it was until the transport was
/// asked to reset a key and as the reset left it after, the subtitle finder's
/// configuration, and whatever `rest` holds everywhere else.
struct Resetting {
    asked: Arc<Fake>,
    rest: Arc<dyn FileSystem>,
}

impl Resetting {
    fn over(asked: &Arc<Fake>) -> Arc<Self> {
        Self::beside(asked, Files::empty())
    }

    fn beside(asked: &Arc<Fake>, rest: Arc<dyn FileSystem>) -> Arc<Self> {
        Arc::new(Self {
            asked: asked.clone(),
            rest,
        })
    }

    fn now(&self) -> &dyn FileSystem {
        self.rest.as_ref()
    }
}

#[async_trait]
impl FileSystem for Resetting {
    async fn canonicalize(&self, path: &Path) -> Result<PathBuf, Fault> {
        self.now().canonicalize(path).await
    }

    async fn touch(&self, path: &Path) -> Result<(), Fault> {
        self.now().touch(path).await
    }

    async fn link(&self, from: &Path, to: &Path) -> Result<(), Fault> {
        self.now().link(from, to).await
    }

    async fn identify(&self, path: &Path) -> Result<Identity, Fault> {
        self.now().identify(path).await
    }

    async fn remove(&self, path: &Path) {
        self.now().remove(path).await;
    }

    async fn read(&self, path: &Path) -> Option<String> {
        let name = path.to_string_lossy();
        if name.ends_with("config.xml") {
            return Some(if self.asked.asked_for("/command") {
                replaced_config()
            } else {
                SERVICE_CONFIG.to_owned()
            });
        }
        if name.ends_with("config.yaml") {
            return Some(BAZARR_CONFIG.to_owned());
        }
        self.now().read(path).await
    }

    async fn write(&self, path: &Path, contents: &str) {
        self.now().write(path, contents).await;
    }

    async fn ownership(&self, path: &Path) -> Option<Ownership> {
        self.now().ownership(path).await
    }
}

#[async_trait]
impl Storage for Resetting {
    async fn describe(&self, path: &Path) -> StorageFacts {
        self.now().describe(path).await
    }
}

/// Rotate Sonarr's key over this transport and these files.
async fn rotated(env: PathBuf, files: Arc<dyn FileSystem>, http: Arc<Fake>) -> Inventory {
    asked(
        &ctx(env, files, http),
        Asking::Rotate {
            credential: "Sonarr API key".to_owned(),
        },
    )
    .await
}

/// A reset that lands is proven with the new key, recorded, and handed to Prowlarr's
/// copy in the same run.
#[tokio::test]
async fn a_reset_key_is_proven_recorded_and_handed_to_prowlarr() {
    let env = env_at("reset", &[("SONARR_API_KEY", &the_service_key())]);
    let http = Fake::by_route_in_turn(vec![
        (Method::Post, "/command", vec![Answer::reply(201, "{}")]),
        (
            Method::Get,
            "/system/status",
            vec![Answer::reply(200, SONARR_STATUS)],
        ),
        (
            Method::Post,
            "/applications/test",
            vec![Answer::reply(400, "[]"), Answer::reply(200, "")],
        ),
        (
            Method::Put,
            "/applications/7",
            vec![Answer::reply(202, "{}")],
        ),
        (
            Method::Get,
            "/applications",
            vec![Answer::reply(200, PROWLARR_HOLDS)],
        ),
    ]);

    let inventory = rotated(env.clone(), Resetting::over(&http), http.clone()).await;

    let rotation = inventory.rotated;
    assert!(
        matches!(
            rotation.as_ref().map(|one| &one.settled),
            Some(Settled::Replaced { observed })
                if observed == "Sonarr 4.0.20.2967 replaced its key and answered to the new one"
        ),
        "{rotation:?}"
    );
    assert_eq!(recorded(&env, "SONARR_API_KEY"), Some(the_new_key()));
    let prowlarr = rotation
        .into_iter()
        .flat_map(|one| one.consumers)
        .find(|one| one.consumer == "Prowlarr, which supplies Sonarr with indexers");
    assert_eq!(prowlarr.map(|one| one.reach), Some(Reach::Updated));
    let rekeyed = http
        .requests()
        .into_iter()
        .find(|request| request.method == Method::Put)
        .and_then(|request| request.body)
        .unwrap_or_default();
    assert!(
        rekeyed.contains(&the_new_key()),
        "Prowlarr was not given the new key"
    );
}

/// A service that will not replace its key leaves the old one recorded and in force.
#[tokio::test]
async fn a_refused_reset_leaves_the_old_key_in_force() {
    let env = env_at("reset-refused", &[("SONARR_API_KEY", &the_service_key())]);
    let http = Fake::by_route(vec![(Method::Post, "/command", Answer::reply(500, "busy"))]);

    let inventory = rotated(env.clone(), Files::anywhere(SERVICE_CONFIG), http).await;

    let said = format!("{:?}", inventory.rotated.map(|one| one.settled));
    assert!(said.starts_with("Some(Refused"), "{said}");
    assert!(said.contains("would not replace its key"), "{said}");
    assert!(said.contains("still in force"), "{said}");
    assert_eq!(recorded(&env, "SONARR_API_KEY"), Some(the_service_key()));
}

/// A reset that writes no new key is said as a replaced key that did not answer, and
/// nothing is recorded.
#[tokio::test(start_paused = true)]
async fn a_reset_that_writes_no_new_key_says_the_old_one_is_gone() {
    let env = env_at("reset-unwritten", &[("SONARR_API_KEY", &the_service_key())]);
    let http = Fake::by_route(vec![(Method::Post, "/command", Answer::reply(201, "{}"))]);

    let inventory = rotated(env.clone(), Files::anywhere(SERVICE_CONFIG), http).await;

    let said = format!("{:?}", inventory.rotated.map(|one| one.settled));
    assert!(said.starts_with("Some(ReplacedUnproven"), "{said}");
    assert!(said.contains("wrote no new key"), "{said}");
    assert!(said.contains("The old key no longer works"), "{said}");
    assert_eq!(recorded(&env, "SONARR_API_KEY"), Some(the_service_key()));
}

/// A new key the service will not answer to is not handed out.
#[tokio::test]
async fn a_new_key_that_does_not_answer_is_not_handed_out() {
    let env = env_at(
        "reset-unanswered",
        &[("SONARR_API_KEY", &the_service_key())],
    );
    let http = Fake::by_route(vec![
        (Method::Post, "/command", Answer::reply(201, "{}")),
        (Method::Get, "/system/status", Answer::reply(401, "")),
    ]);

    let inventory = rotated(env.clone(), Resetting::over(&http), http.clone()).await;

    let said = format!("{:?}", inventory.rotated.map(|one| one.settled));
    assert!(said.starts_with("Some(ReplacedUnproven"), "{said}");
    assert!(
        said.contains("run `lemonfiber seed` once the service answers"),
        "{said}"
    );
    assert!(!said.contains(&the_new_key()), "{said}");
    assert_eq!(recorded(&env, "SONARR_API_KEY"), Some(the_service_key()));
}

/// A service that has written no key has none to replace.
#[tokio::test]
async fn a_service_that_has_written_no_key_has_none_to_replace() {
    let env = env_at("reset-nokey", &[]);
    let http = Fake::by_path(Vec::new());

    let inventory = rotated(env, Files::empty(), http.clone()).await;

    let said = format!("{:?}", inventory.rotated.map(|one| one.settled));
    assert!(said.contains("none to replace"), "{said}");
    assert!(!http.asked_for("/command"), "a key nobody holds was reset");
}

/// A rehearsed reset asks the service nothing: the reset is the one step a rehearsal
/// never takes.
#[tokio::test]
async fn a_rehearsed_reset_asks_the_service_nothing() {
    let env = env_at("reset-rehearsed", &[("SONARR_API_KEY", &the_service_key())]);
    let http = Fake::by_path(Vec::new());
    let ctx = ctx(env.clone(), Files::anywhere(SERVICE_CONFIG), http.clone()).rehearsing();

    let inventory = asked(
        &ctx,
        Asking::Rotate {
            credential: "Sonarr API key".to_owned(),
        },
    )
    .await;

    let said = format!("{:?}", inventory.rotated.map(|one| one.settled));
    assert!(said.starts_with("Some(Rehearsed"), "{said}");
    assert!(said.contains("Nothing was replaced"), "{said}");
    assert!(http.requests().is_empty(), "a rehearsal reached a service");
    assert_eq!(recorded(&env, "SONARR_API_KEY"), Some(the_service_key()));
}

/// The gate's routes, with Jellyfin's presenting `key` and no \*arr route yet.
fn gate_routes(key: &str) -> String {
    Upstreams::of(vec![Upstream {
        route: "jellyfin".to_owned(),
        kind: Kind::Jellyfin,
        address: "http://jellyfin:8096".to_owned(),
        credential: Credential::new(key),
        majors: vec![10],
    }])
    .written()
}

/// The subtitle finder and the request gate's route each get the new key in the same
/// run as the reset.
#[tokio::test]
async fn a_reset_key_reaches_the_subtitle_finder_and_the_gate() {
    let env = crate::common::household::recorded_admin("reset-copies");
    assert!(
        store::set(&env, "SONARR_API_KEY", &the_service_key()).is_ok(),
        "the scratch settings file is written"
    );
    let stack: &'static Path =
        Box::leak(crate::common::stack::with_the_gate("reset-copies").into_boxed_path());
    let routes_file = stack.join("config/request-gate/upstreams.json");
    let _ = std::fs::create_dir_all(stack.join("config/request-gate"));
    let _ = std::fs::write(&routes_file, gate_routes("held"));
    let http = Fake::by_route_in_turn(vec![
        (Method::Post, "/command", vec![Answer::reply(201, "{}")]),
        (
            Method::Get,
            "/system/status",
            vec![Answer::reply(200, SONARR_STATUS)],
        ),
        (
            Method::Post,
            "/Users/AuthenticateByName",
            vec![Answer::reply(200, r#"{"AccessToken":"token"}"#)],
        ),
        (
            Method::Get,
            "/Auth/Keys",
            vec![Answer::reply(
                200,
                r#"{"Items":[{"AppName":"lemonfiber-request-gate","AccessToken":"held"}]}"#,
            )],
        ),
        (
            Method::Post,
            "/api/system/settings",
            vec![Answer::reply(204, "")],
        ),
        (Method::Get, "/applications", vec![Answer::reply(200, "[]")]),
        (
            Method::Post,
            "/applications",
            vec![Answer::reply(201, "{}")],
        ),
    ]);
    let ctx = lemonfiber_testing::a_context()
        .over(Source::External(stack))
        .engine(Arc::new(Reporting::default()))
        .filesystem(Resetting::beside(&http, Arc::new(Disk)))
        .settings(Settings {
            env_file: Some(env.clone()),
            ..Settings::default()
        })
        .build()
        .with_http(http.clone());

    let inventory = asked(
        &ctx,
        Asking::Rotate {
            credential: "Sonarr API key".to_owned(),
        },
    )
    .await;
    let routed = std::fs::read_to_string(&routes_file).unwrap_or_default();
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(Path::new("/")));
    let _ = std::fs::remove_dir_all(stack);

    let consumers = inventory
        .rotated
        .map(|one| one.consumers)
        .unwrap_or_default();
    let reach = |named: &str| {
        consumers
            .iter()
            .find(|one| one.consumer == named)
            .map(|one| one.reach.clone())
    };
    assert_eq!(
        reach("Bazarr, which finds subtitles for Sonarr"),
        Some(Reach::Updated),
        "{consumers:?}"
    );
    assert_eq!(
        reach("the request gate, which reaches Sonarr for the request service"),
        Some(Reach::Updated),
        "{consumers:?}"
    );
    assert!(
        routed.contains(&the_new_key()),
        "the gate's route kept the old key"
    );
    let watched = http
        .requests()
        .into_iter()
        .find(|request| request.url.ends_with("/api/system/settings"))
        .and_then(|request| request.body)
        .unwrap_or_default();
    assert!(
        watched.contains(&the_new_key()),
        "the finder was not given the new key"
    );
}

/// A reset the service made and answered to whose new key cannot be recorded says the
/// old key is gone and what finishes the job.
#[tokio::test]
async fn a_reset_key_that_cannot_be_recorded_says_what_finishes_the_job() {
    let env = env_at(
        "reset-unrecordable",
        &[("SONARR_API_KEY", &the_service_key())],
    );
    sealed(&env);
    let http = Fake::by_route(vec![
        (Method::Post, "/command", Answer::reply(201, "{}")),
        (
            Method::Get,
            "/system/status",
            Answer::reply(200, SONARR_STATUS),
        ),
    ]);

    let inventory = rotated(env.clone(), Resetting::over(&http), http.clone()).await;

    let said = format!("{:?}", inventory.rotated.map(|one| one.settled));
    assert!(said.starts_with("Some(ReplacedUnproven"), "{said}");
    assert!(said.contains("could not be recorded"), "{said}");
    assert!(said.contains("lemonfiber seed"), "{said}");
    assert_eq!(recorded(&env, "SONARR_API_KEY"), Some(the_service_key()));
}
