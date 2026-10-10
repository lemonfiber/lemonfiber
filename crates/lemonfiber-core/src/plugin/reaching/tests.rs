use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::scratch::Scratch;

use super::{filling, reached};
use crate::app::Ctx;
use crate::ports::docker::{Health, Lifecycle};
use crate::test_support::{a_context, a_placed, Reporting};

const ADAPTER: &str = "plex-adapter";

/// A stack directory holding `key` beside the adapter's configuration, and a context
/// whose engine publishes the adapter as `published` and whose transport answers
/// `answer`.
fn keyed(
    tag: &str,
    key: Option<&str>,
    published: &[(&str, &str, u16)],
) -> (Scratch, Ctx, Arc<Fake>) {
    let stack = Scratch::new(&format!("reaching-{tag}"));
    if let Some(key) = key {
        let at = crate::plugin::key_file(&stack, ADAPTER);
        let _ = std::fs::create_dir_all(at.parent().unwrap_or(&at));
        let _ = std::fs::write(&at, key);
    }
    let engine =
        Reporting::holding(&[ADAPTER], Lifecycle::Running, Health::Healthy).publishing(published);
    let fake = Fake::always(Answer::reply(
        200,
        r#"{"speaks":["media.serve@1"],"upstream":"a media server","releases":[]}"#,
    ));
    let ctx = a_context()
        .engine(Arc::new(engine))
        .build()
        .with_http(fake.clone());
    (stack, ctx, fake)
}

fn speaking(listens: Option<u16>) -> crate::plugin::Placed {
    crate::plugin::Placed {
        speaks: vec!["media.serve@1".to_owned()],
        ..a_placed(ADAPTER, &[], None, listens)
    }
}

#[tokio::test]
async fn an_adapter_is_asked_on_loopback_at_its_published_port_under_its_key() {
    let (stack, ctx, fake) = keyed(
        "reached",
        Some("the-key\n"),
        &[(ADAPTER, "127.0.0.1", 8080)],
    );
    let adapter = reached(&ctx, &stack, &speaking(Some(8080))).await;
    let about = match adapter {
        Ok(adapter) => adapter.about().await.ok(),
        Err(why) => unreachable!("{why}"),
    };
    assert_eq!(
        about.map(|about| about.speaks),
        Some(vec!["media.serve@1".to_owned()])
    );
    let request = fake.request();
    assert_eq!(
        request.as_ref().map(|request| request.url.as_str()),
        Some("http://127.0.0.1:8080/lemonfiber/adapter/v1/about")
    );
    assert!(request.is_some_and(|request| request
        .headers
        .contains(&("Authorization".to_owned(), "Bearer the-key".to_owned()))));
}

#[tokio::test]
async fn an_adapter_that_cannot_be_reached_says_what_is_missing() {
    for (tag, key, published, listens, says) in [
        (
            "no-port",
            Some("k"),
            vec![(ADAPTER, "127.0.0.1", 8080)],
            None,
            "says no port",
        ),
        (
            "no-key",
            None,
            vec![(ADAPTER, "127.0.0.1", 8080)],
            Some(8080),
            "holds no key",
        ),
        (
            "blank-key",
            Some("  \n"),
            vec![(ADAPTER, "127.0.0.1", 8080)],
            Some(8080),
            "holds no key",
        ),
        (
            "on-the-lan",
            Some("k"),
            vec![(ADAPTER, "0.0.0.0", 8080)],
            Some(8080),
            "publishes no port",
        ),
        (
            "unpublished",
            Some("k"),
            Vec::new(),
            Some(8080),
            "publishes no port",
        ),
    ] {
        let (stack, ctx, _) = keyed(tag, key, &published);
        let refused = reached(&ctx, &stack, &speaking(listens)).await.err();
        assert!(
            refused.as_deref().is_some_and(|why| why.contains(says)),
            "{tag}: {refused:?}"
        );
    }
}

#[tokio::test]
async fn a_key_that_is_not_a_plain_file_is_not_read() {
    let (stack, ctx, _) = keyed("linked-key", None, &[(ADAPTER, "127.0.0.1", 8080)]);
    let at = crate::plugin::key_file(&stack, ADAPTER);
    let _ = std::fs::create_dir_all(at.parent().unwrap_or(&at));
    let _ = std::os::unix::fs::symlink("/etc/hosts", &at);
    let refused = reached(&ctx, &stack, &speaking(Some(8080))).await.err();
    assert!(
        refused
            .as_deref()
            .is_some_and(|why| why.contains("not a plain file")),
        "{refused:?}"
    );
}

#[tokio::test]
async fn an_engine_that_will_not_list_is_said() {
    let (stack, _, _) = keyed("engine-down", Some("k"), &[]);
    let ctx = a_context().engine(Arc::new(Reporting::absent())).build();
    let refused = reached(&ctx, &stack, &speaking(Some(8080))).await.err();
    assert!(
        refused
            .as_deref()
            .is_some_and(|why| why.contains("would not say")),
        "{refused:?}"
    );
}

/// The adapter as the service filling a capability, configured beneath `stack` where one
/// is given.
fn filler(stack: Option<&std::path::Path>) -> crate::wiring::Filler {
    crate::wiring::Filler {
        id: ADAPTER.to_owned(),
        name: ADAPTER.to_owned(),
        origin: crate::origin::Origin::Plugin {
            named: "plex".to_owned(),
        },
        address: Some(crate::wiring::Address {
            host: ADAPTER.to_owned(),
            port: 8080,
        }),
        adapter: None,
        published: None,
        key_file: None,
        confined_to: stack.map(|stack| {
            crate::plugin::key_file(stack, ADAPTER)
                .parent()
                .map(std::path::Path::to_path_buf)
                .unwrap_or_default()
        }),
        media_types: Vec::new(),
        provides: vec!["media.serve".to_owned()],
        contracts: vec!["media.serve@1".to_owned()],
        first_party: false,
        majors: Vec::new(),
        fronts: None,
        native: None,
    }
}

#[tokio::test]
async fn the_service_filling_a_capability_is_asked_beside_its_configuration() {
    let (stack, ctx, fake) = keyed("filling", Some("the-key"), &[(ADAPTER, "127.0.0.1", 8080)]);
    let about = match filling(&ctx, &filler(Some(&stack))).await {
        Ok(adapter) => adapter.about().await.ok(),
        Err(why) => unreachable!("{why}"),
    };
    assert!(about.is_some());
    assert!(fake.request().is_some_and(|request| request
        .headers
        .contains(&("Authorization".to_owned(), "Bearer the-key".to_owned()))));

    let unwritten = filling(&ctx, &filler(None)).await;
    assert!(
        unwritten
            .as_ref()
            .is_err_and(|why| why.contains("no configuration on this machine")),
        "{unwritten:?}"
    );
}
