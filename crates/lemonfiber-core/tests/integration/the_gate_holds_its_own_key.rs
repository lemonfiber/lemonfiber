//! The request gate's own media server key is listed, printed on a confirmed ask, and
//! replaced in the order that keeps a working key at every moment, all through the
//! dispatcher.

use std::path::Path;
use std::sync::Arc;

use crate::common::household::recorded_admin;
use lemonfiber_core::app::{dispatch, Command, Outcome};
use lemonfiber_core::config::Settings;
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::http::Fake;
use lemonfiber_sidecar::gate::{Credential, Kind, Upstream, Upstreams};
use lemonfiber_testing::media;

/// The gate's routes, with the media server's presenting `key`.
fn routes(key: &str) -> String {
    Upstreams::of(vec![Upstream {
        route: "jellyfin".to_owned(),
        kind: Kind::Jellyfin,
        address: "http://jellyfin:8096".to_owned(),
        credential: Credential::new(key),
        majors: vec![10],
        pins: lemonfiber_sidecar::Pins::default(),
    }])
    .written()
}

#[tokio::test]
async fn the_gate_key_is_listed_shown_and_replaced() {
    let env = recorded_admin("gate-key-credentials");
    let stack: &'static Path =
        Box::leak(crate::common::stack::with_the_gate("key-credentials").into_boxed_path());
    let routes_file = stack.join("config/request-gate/upstreams.json");
    let _ = std::fs::create_dir_all(stack.join("config/request-gate"));
    let _ = std::fs::write(&routes_file, routes("old"));
    let filed = |keys: &[&str]| media::answered(&keys);
    let http = Fake::by_path_in_turn(vec![
        (
            media::serve("filed_as"),
            vec![
                filed(&["old"]),
                filed(&["old", "fresh"]),
                filed(&["old", "fresh"]),
            ],
        ),
        (media::serve("mint"), vec![media::answered(&"fresh")]),
        (media::serve("answers_to"), vec![media::taken()]),
        (media::serve("revoke"), vec![media::taken()]),
    ]);
    let ctx = media::keyed_in(
        lemonfiber_testing::a_context()
            .over(Source::External(stack))
            .engine(Arc::new(media::running()))
            .settings(Settings {
                env_file: Some(env.clone()),
                ..Settings::default()
            })
            .build()
            .with_http(http.clone()),
    );
    let asked = |asking| dispatch(Command::Credentials(asking), &ctx);

    let shown = match asked(lemonfiber_core::app::Asking::Reveal {
        credential: "Jellyfin request-gate key".to_owned(),
        confirmed: true,
    })
    .await
    {
        Ok(Outcome::Credentials(inventory)) => inventory.revealed.and_then(|one| one.value),
        _ => None,
    };
    let rotated = match asked(lemonfiber_core::app::Asking::Rotate {
        credential: "Jellyfin request-gate key".to_owned(),
        agreement: None,
    })
    .await
    {
        Ok(Outcome::Credentials(inventory)) => inventory.rotated.map(|one| one.settled),
        _ => None,
    };
    let held_now = std::fs::read_to_string(&routes_file).unwrap_or_default();
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(Path::new("/")));
    let _ = std::fs::remove_dir_all(stack);

    assert_eq!(shown.as_deref(), Some("old"));
    assert!(
        matches!(
            rotated,
            Some(lemonfiber_core::credential::Settled::Replaced { .. })
        ),
        "{rotated:?}"
    );
    assert_eq!(held_now, routes("fresh"));
    assert!(http.requests().iter().any(|asked| {
        asked.url.ends_with(media::serve("revoke"))
            && asked.body.as_deref() == Some(r#"{"key":"old"}"#)
    }));
}
