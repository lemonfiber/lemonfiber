use std::collections::BTreeMap;

use lemonfiber_contract::adapter::{UPSTREAM_FILE, UPSTREAM_PASSWORD};
use lemonfiber_fixtures::http::{Answer, Fake};

use super::{pending, settle};
use crate::app::targets::{record_secret, recorded_secret, service_config_dir, MediaServer};
use crate::config;
use crate::test_support::{a_contracted_media_server, contracted_context, MEDIA_ADAPTER};

/// What a settled replacement left: the password under the server's setting, what was
/// still pending, and the credential the adapter reads its upstream's from.
struct Settled {
    recorded: Option<String>,
    left_pending: Option<String>,
    handed: BTreeMap<String, String>,
}

/// Settle a replacement left pending for a contracted media server, its adapter
/// answering each sign-in in turn.
async fn settled_over_its_contract(name: &str, signs_in: Vec<Answer>) -> Settled {
    let env = crate::test_support::env_without_password(name);
    let project = env
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .join("stack");
    let asking = Fake::by_path_in_turn(vec![("identity.source/v1/whoever", signs_in)]);
    let ctx = contracted_context(&project, MEDIA_ADAPTER, true)
        .settings(config::Settings {
            env_file: Some(env.clone()),
            stack_dir: Some(project.clone()),
            ..config::Settings::default()
        })
        .build()
        .with_http(asking);
    let server = crate::test_support::stack()
        .manifest()
        .ok()
        .and_then(|manifest| {
            MediaServer::of(&crate::wiring::Fillers::of(
                &manifest,
                &[a_contracted_media_server(Some("upstream"))],
                &crate::wiring::Chosen::read(Some("identity.source=media-adapter")),
                Some(&project),
            ))
        });
    let Some(server) = server else {
        unreachable!("the contracted media server fills the identity source");
    };
    let setting = server.setting.clone();
    let _ = record_secret(&ctx, &setting, "in-force");
    let _ = record_secret(&ctx, &pending(&setting), "replacement");

    settle(&ctx, &server).await;

    let upstream = service_config_dir(&project, MEDIA_ADAPTER).join(UPSTREAM_FILE);
    let settled = Settled {
        recorded: recorded_secret(&ctx, &setting),
        left_pending: recorded_secret(&ctx, &pending(&setting)),
        handed: std::fs::read(&upstream)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default(),
    };
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
    settled
}

/// A replacement the contracted server takes is moved into place and handed to its
/// adapter, so the adapter signs in to its upstream with the password in force.
#[tokio::test]
async fn a_replacement_a_contracted_server_takes_is_handed_to_its_adapter() {
    let settled = settled_over_its_contract(
        "pending-contracted-taken",
        vec![
            Answer::reply(200, "null"),
            Answer::reply(200, r#"{"id":"admin","token":"granted"}"#),
        ],
    )
    .await;

    assert_eq!(settled.recorded.as_deref(), Some("replacement"));
    assert_eq!(settled.left_pending, None);
    assert_eq!(
        settled.handed.get(UPSTREAM_PASSWORD).map(String::as_str),
        Some("replacement")
    );
}

/// Where the contracted server still takes the password in force, the replacement is
/// forgotten and the adapter is handed nothing.
#[tokio::test]
async fn a_contracted_server_still_taking_the_one_in_force_forgets_the_replacement() {
    let settled = settled_over_its_contract(
        "pending-contracted-kept",
        vec![Answer::reply(200, r#"{"id":"admin","token":"granted"}"#)],
    )
    .await;

    assert_eq!(settled.recorded.as_deref(), Some("in-force"));
    assert_eq!(settled.left_pending, None);
    assert!(settled.handed.is_empty(), "{:?}", settled.handed);
}

/// A contracted server that will not answer settles nothing: both stay for a later run.
#[tokio::test]
async fn a_contracted_server_that_will_not_answer_leaves_both_for_later() {
    let settled =
        settled_over_its_contract("pending-contracted-silent", vec![Answer::reply(503, "")]).await;

    assert_eq!(settled.recorded.as_deref(), Some("in-force"));
    assert_eq!(settled.left_pending.as_deref(), Some("replacement"));
    assert!(settled.handed.is_empty(), "{:?}", settled.handed);
}
