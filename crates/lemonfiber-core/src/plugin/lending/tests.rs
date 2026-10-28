use std::sync::Arc;

use lemonfiber_contract::adapter::{KEY_FILE, UPSTREAM_FILE};
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::serving::Serves;
use lemonfiber_ports::program::Unran;
use lemonfiber_ports::serving::Standing;

use super::{lent, Borrowing};
use crate::config::Settings;
use crate::test_support::context::a_context;
use crate::test_support::curators::TELEVISION;

/// The port the engine says the lent adapter is published on.
const PUBLISHED: u16 = 41_234;

/// A context whose engine serves as `serves` does and whose transport answers as `http`,
/// keeping its plugins' starting files beside an env file of its own.
fn lending(name: &str, serves: &Arc<Serves>, http: Arc<Fake>) -> crate::app::Ctx {
    let dir = lemonfiber_fixtures::scratch::Scratch::named(&format!("lending-{name}")).kept();
    let mut ctx = a_context()
        .settings(Settings {
            env_file: Some(dir.join(".env")),
            ..Settings::default()
        })
        .build();
    ctx.seams.serving = Arc::clone(serves) as Arc<dyn lemonfiber_ports::serving::Serving>;
    ctx.seams.http = http;
    ctx
}

/// Sonarr's plugin, and the adapter in front of it.
fn sonarr() -> (crate::plugin::Installed, crate::plugin::Placed) {
    let installed = TELEVISION
        .installed()
        .unwrap_or_else(|| unreachable!("no plugin"));
    let adapter = installed
        .services
        .iter()
        .find(|placed| placed.service == TELEVISION.adapter)
        .cloned()
        .unwrap_or_else(|| unreachable!("no adapter"));
    (installed, adapter)
}

/// What `adapter` of `installed` is lent with in these tests.
fn borrowing<'a>(
    installed: &'a crate::plugin::Installed,
    adapter: &'a crate::plugin::Placed,
) -> Borrowing<'a> {
    Borrowing {
        installed,
        adapter,
        upstream: "http://theirs:8990".to_owned(),
        reaching: vec!["theirs".to_owned()],
        credentials: vec![("api_key".to_owned(), "their-key".to_owned())],
    }
}

#[tokio::test]
async fn a_lent_adapter_is_pointed_at_their_service_with_their_key_alone_and_taken_away() {
    let serves = Arc::new(Serves::on(PUBLISHED));
    let ready = Fake::by_path(vec![(
        "/lemonfiber/adapter/v1/ready",
        Answer::reply(200, "{}"),
    )]);
    let ctx = lending("lent", &serves, ready);
    let (installed, adapter) = sonarr();

    let lent = lent(&ctx, &borrowing(&installed, &adapter)).await;

    let Ok(lent) = lent else {
        unreachable!("the adapter was not lent: {:?}", lent.err());
    };
    let [asked] = serves.asked().try_into().unwrap_or_else(|_| unreachable!());
    assert_eq!(asked.command, vec!["--upstream", "http://theirs:8990"]);
    assert_eq!(asked.reaching, vec!["theirs"]);
    let held = asked
        .binds
        .first()
        .map(|bind| bind.source.clone())
        .unwrap_or_default();
    let upstream: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(held.join(UPSTREAM_FILE)).unwrap_or_default(),
    )
    .unwrap_or_default();
    assert_eq!(upstream, serde_json::json!({ "api_key": "their-key" }));
    assert!(held.join(KEY_FILE).exists());
    assert_eq!(lent.adapter.service(), TELEVISION.adapter);
    assert!(serves.removed().is_empty());

    lent.returned(&ctx).await;

    assert_eq!(serves.removed().len(), 1);
    assert!(!held.exists(), "its configuration was kept");
}

#[tokio::test]
async fn an_adapter_that_never_comes_ready_is_taken_away_and_said_why() {
    let serves = Arc::new(Serves::on(PUBLISHED).standing(Standing::Exited(1)));
    let ctx = lending("exited", &serves, Fake::silent());
    let (installed, adapter) = sonarr();

    let lent = lent(&ctx, &borrowing(&installed, &adapter)).await;

    assert!(
        lent.as_ref()
            .err()
            .is_some_and(|why| why.contains("exited 1")),
        "{:?}",
        lent.err()
    );
    assert_eq!(serves.removed().len(), 1);
}

#[tokio::test]
async fn an_engine_that_serves_nothing_leaves_nothing_to_take_away() {
    let serves = Arc::new(Serves::refusing(Unran::Unreachable {
        reason: "no engine".to_owned(),
    }));
    let ctx = lending("refused", &serves, Fake::silent());
    let (installed, adapter) = sonarr();

    let lent = lent(&ctx, &borrowing(&installed, &adapter)).await;

    assert!(lent.is_err());
    assert!(serves.removed().is_empty());
}

#[tokio::test]
async fn nothing_is_lent_where_no_starting_files_are_kept() {
    let serves = Arc::new(Serves::on(PUBLISHED));
    let mut ctx = lending("unkept", &serves, Fake::silent());
    ctx.settings.env_file = None;
    let (installed, adapter) = sonarr();

    let lent = lent(&ctx, &borrowing(&installed, &adapter)).await;

    assert!(lent.is_err());
    assert!(serves.asked().is_empty());
}

#[cfg(unix)]
#[tokio::test]
async fn nothing_is_lent_where_its_starting_files_will_not_be_copied() {
    let serves = Arc::new(Serves::on(PUBLISHED));
    let ctx = lending("unkind", &serves, Fake::silent());
    let (installed, adapter) = sonarr();
    let starting = crate::app::plugins::kept::root(&ctx, &installed.plugin)
        .unwrap_or_default()
        .join(crate::project::entry::CONFIGURATION_TREE)
        .join(&adapter.service);
    let _ = std::fs::remove_dir_all(&starting);
    let _ = std::fs::create_dir_all(&starting);
    let _ = std::os::unix::fs::symlink("/etc/hosts", starting.join("linked"));

    let lent = lent(&ctx, &borrowing(&installed, &adapter)).await;

    assert!(
        lent.as_ref()
            .err()
            .is_some_and(|why| why.starts_with("linked is a link")),
        "{:?}",
        lent.err()
    );
    assert!(serves.asked().is_empty());
}

#[tokio::test]
async fn an_adapter_the_engine_publishes_no_port_for_is_taken_away_and_said_why() {
    let serves = Arc::new(Serves::up(lemonfiber_ports::serving::Served {
        container: "4f1c2d3e5a6b7c8d9e0f".to_owned(),
        network: "lemonfiber-lent".to_owned(),
        hostname: "4f1c2d3e5a6b".to_owned(),
        addresses: Vec::new(),
        machine: Vec::new(),
        port: None,
        contract: None,
    }));
    let ctx = lending("portless", &serves, Fake::silent());
    let (installed, adapter) = sonarr();

    let lent = lent(&ctx, &borrowing(&installed, &adapter)).await;

    assert!(
        lent.as_ref()
            .err()
            .is_some_and(|why| why.ends_with("publishes no port, so nothing can ask it")),
        "{:?}",
        lent.err()
    );
    assert_eq!(serves.removed().len(), 1);
}

#[tokio::test]
async fn nothing_is_lent_where_no_key_can_be_made_for_it() {
    let serves = Arc::new(Serves::on(PUBLISHED));
    let ctx = lending("keyless", &serves, Fake::silent())
        .with_random(Arc::new(lemonfiber_fixtures::support::FixedRandom(None)));
    let (installed, adapter) = sonarr();

    let lent = lent(&ctx, &borrowing(&installed, &adapter)).await;

    assert_eq!(
        lent.err().as_deref(),
        Some(crate::secret::NO_RANDOMNESS_FOR_PASSWORD)
    );
    assert!(serves.asked().is_empty());
}
