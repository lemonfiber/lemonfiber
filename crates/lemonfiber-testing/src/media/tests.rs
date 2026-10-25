use lemonfiber_core::ports::http::{Method, Request};

use lemonfiber_core::config::Settings;
use lemonfiber_core::ports::filesystem::Beneath;
use lemonfiber_fixtures::scratch::Scratch;

use super::super::context::a_context;
use super::{asked, asked_about, key_at, keyed_in, moved, served, ADAPTER, KEY, PLUGIN};

fn sent(url: &str, body: Option<&str>) -> Request {
    Request {
        method: Method::Post,
        url: format!("http://127.0.0.1:8080{url}"),
        headers: Vec::new(),
        body: body.map(str::to_owned),
        pinned: None,
    }
}

#[test]
fn what_was_asked_is_read_per_operation_in_order() {
    let invited = super::identity("invite");
    let withdrawn = super::identity("withdraw");
    let refreshed = super::serve("refresh");
    let requests = [
        sent(invited, Some(r#"{"id":"1","name":"ana"}"#)),
        sent(withdrawn, Some(r#"{"id":"2"}"#)),
        sent(invited, None),
        sent(invited, Some("not json")),
        sent(withdrawn, Some(r#"{"name":"nobody"}"#)),
        sent(withdrawn, Some(r#"{"id":"3"}"#)),
        sent(refreshed, Some(r#"{"id":"4"}"#)),
    ];

    assert_eq!(
        asked(&requests, "invite"),
        vec![serde_json::json!({"id": "1", "name": "ana"})]
    );
    assert_eq!(asked_about(&requests, "withdraw"), vec!["2", "3"]);
    assert_eq!(
        served(&requests, "refresh"),
        vec![serde_json::json!({"id": "4"})]
    );
    assert!(asked(&requests, "refresh").is_empty());
}

#[test]
fn the_media_server_moved_in_is_jellyfins_plugin_keyed_wherever_its_stack_is() {
    let directory = Scratch::new("media-keyed-in-moved");
    let mut ctx = a_context()
        .settings(Settings {
            stack_dir: Some(directory.to_path_buf()),
            ..Settings::default()
        })
        .build();
    ctx.settings.env_file = Some(directory.join(".env"));

    let ctx = keyed_in(ctx);

    assert_eq!(
        moved().map(|installed| installed.plugin),
        Some(PLUGIN.to_owned())
    );
    let key = key_at(&ctx).unwrap_or_default();
    assert_eq!(std::fs::read_to_string(&key).unwrap_or_default(), KEY);
    assert_eq!(
        ctx.seams.confined.read(
            &lemonfiber_core::plugin::key_file(&Scratch::new("media-elsewhere"), ADAPTER),
            &directory
        ),
        Beneath::Read(KEY.to_owned())
    );
}
