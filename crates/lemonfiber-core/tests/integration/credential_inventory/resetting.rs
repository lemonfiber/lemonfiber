//! Replacing a media \*arr's own key and handing the new one to every copy.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;

use super::{asked, ctx, env_at, recorded, the_service_key, SERVICE_CONFIG};
use lemonfiber_core::app::Asking;
use lemonfiber_core::credential::{Inventory, Reach, Settled};
use lemonfiber_core::ports::filesystem::{
    Fault, FileSystem, Identity, Ownership, Storage, StorageFacts,
};
use lemonfiber_core::ports::http::Method;
use lemonfiber_fixtures::files::Files;
use lemonfiber_fixtures::http::{Answer, Fake};

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

/// A filesystem holding each service's configuration as it was until the transport
/// was asked to reset a key, and as the reset left it after.
struct Resetting {
    asked: Arc<Fake>,
    before: Arc<Files>,
    after: Arc<Files>,
}

impl Resetting {
    fn over(asked: &Arc<Fake>) -> Arc<Self> {
        Arc::new(Self {
            asked: asked.clone(),
            before: Files::anywhere(SERVICE_CONFIG),
            after: Files::anywhere(replaced_config()),
        })
    }

    fn now(&self) -> &Files {
        if self.asked.asked_for("/command") {
            &self.after
        } else {
            &self.before
        }
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
