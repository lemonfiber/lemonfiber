use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::scratch::Scratch;

use super::super::tests::asking::SERVED;
use super::{performed, Held, Presented, Reaching};
use crate::plugin::installed::{Installed, Placed};
use crate::plugin::program::{Starting, Throwaway};
use crate::ports::http::Http;
use crate::ports::serving::Served;

const SERVARR: &str =
    "api         = { kind = \"servarr\", key_source = \"config-xml\", path = \"/config/config.xml\", version = 3 }";

fn placed(api: &str) -> Option<Placed> {
    let text = SERVED.replace(SERVARR, api);
    let manifest = lemonfiber_plugin::Manifest::from_toml(&text);
    assert!(manifest.is_ok(), "{manifest:?}");
    manifest
        .ok()
        .and_then(|manifest| Installed::of(&manifest).services.into_iter().next())
}

fn copy(name: &str) -> Option<(Scratch, Throwaway)> {
    let source = Scratch::new(name);
    let copy = Throwaway::of(
        Some(Starting {
            root: &source,
            service: "sonarr",
        }),
        &lemonfiber_fixtures::ports::Chance::cycling(),
    )
    .ok()?;
    Some((source, copy))
}

fn served() -> Served {
    Served {
        container: "4f1c2d3e5a6b7c8d".to_owned(),
        network: "n".to_owned(),
        hostname: "4f1c2d3e5a6b".to_owned(),
        addresses: Vec::new(),
        machine: Vec::new(),
        port: Some(49153),
    }
}

async fn first_run(
    placed: &Placed,
    copy: &Throwaway,
    http: Arc<Fake>,
) -> Result<Option<Held>, String> {
    first_run_on(placed, copy, http, &served()).await
}

async fn first_run_on(
    placed: &Placed,
    copy: &Throwaway,
    http: Arc<Fake>,
    served: &Served,
) -> Result<Option<Held>, String> {
    performed(&Reaching {
        placed,
        copy,
        served,
        keyed: http as Arc<dyn Http>,
    })
    .await
}

#[tokio::test]
async fn a_service_writing_its_own_key_is_first_run_by_reading_it_and_presented_as_its_adapter_does(
) {
    let Some((_source, copy)) = copy("first-servarr") else {
        return;
    };
    let _ = std::fs::write(
        copy.path().join("config.xml"),
        "<Config><ApiKey>abc123abc123</ApiKey></Config>",
    );
    let Some(placed) = placed(SERVARR) else {
        return;
    };
    assert_eq!(
        first_run(&placed, &copy, Fake::silent()).await,
        Ok(Some(Held {
            presented: Presented("X-Api-Key", "abc123abc123".to_owned()),
            made: vec!["abc123abc123".to_owned()],
        }))
    );
}

#[tokio::test]
async fn a_service_with_no_api_or_no_credential_has_no_first_run() {
    let Some((_source, copy)) = copy("first-none") else {
        return;
    };
    for api in [
        "",
        "api         = { kind = \"servarr\", key_source = \"none\", version = 3 }",
    ] {
        if let Some(placed) = placed(api) {
            assert_eq!(
                first_run(&placed, &copy, Fake::silent()).await,
                Ok(None),
                "{api}"
            );
        }
    }
}

/// A service naming an API whose first run is an adapter's, such as a torrent client's, a
/// media server's or a book curator's, has none performed here: it is refused, asking
/// nothing of it.
#[tokio::test]
async fn a_service_whose_first_run_is_an_adapters_is_refused_asking_nothing() {
    let Some((_source, copy)) = copy("first-adapters") else {
        return;
    };
    for kind in [
        "qbittorrent",
        "sabnzbd",
        "jellyfin",
        "audiobookshelf",
        "nzbhydra2",
        "bindery",
    ] {
        let Some(placed) = placed(&format!(
            "api         = {{ kind = \"{kind}\", key_source = \"generated\" }}"
        )) else {
            return;
        };
        let http = Fake::always(Answer::reply(200, "Ok."));
        let held = first_run(&placed, &copy, Arc::clone(&http)).await;
        assert!(
            held.as_ref().is_err_and(|why| why.contains("adapter")),
            "{kind}: {held:?}"
        );
        assert!(http.requests().is_empty(), "{kind}");
    }
}

#[tokio::test]
async fn a_media_servers_first_run_is_its_adapters_and_refused_without_one() {
    let Some((_source, copy)) = copy("first-jellyfin") else {
        return;
    };
    let Some(placed) = placed("api         = { kind = \"jellyfin\", key_source = \"generated\" }")
    else {
        return;
    };
    let said = first_run(&placed, &copy, Fake::silent()).await;
    assert!(
        said.as_ref().is_err_and(|why| why.contains("adapter")),
        "{said:?}"
    );
}

#[tokio::test]
async fn a_key_file_outside_the_configuration_directory_is_refused() {
    let Some((_source, copy)) = copy("first-outside") else {
        return;
    };
    let Some(placed) = placed(
        "api         = { kind = \"servarr\", key_source = \"config-xml\", path = \"/etc/config.xml\", version = 3 }",
    ) else {
        return;
    };
    let said = first_run(&placed, &copy, Fake::silent()).await;
    assert!(
        said.as_ref()
            .is_err_and(|why| why.contains("not beneath its configuration directory")),
        "{said:?}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_key_file_the_container_made_a_link_is_refused_rather_than_followed() {
    let Some((_source, copy)) = copy("first-linked") else {
        return;
    };
    let elsewhere = Scratch::new("first-linked-elsewhere");
    let _ = std::fs::write(
        elsewhere.join("host.xml"),
        "<Config><ApiKey>hostkeyhostkey</ApiKey></Config>",
    );
    let _ = std::os::unix::fs::symlink(elsewhere.join("host.xml"), copy.path().join("config.xml"));
    let Some(placed) = placed(SERVARR) else {
        return;
    };
    let said = first_run(&placed, &copy, Fake::silent()).await;
    assert!(
        said.as_ref()
            .is_err_and(|why| why.contains("is a link or a device")),
        "{said:?}"
    );
}

#[tokio::test(start_paused = true)]
async fn a_key_never_written_fails_the_first_run_at_its_limit() {
    let Some((_source, copy)) = copy("first-unwritten") else {
        return;
    };
    let Some(placed) = placed(SERVARR) else {
        return;
    };
    let said = first_run(&placed, &copy, Fake::silent()).await;
    assert!(
        said.as_ref()
            .is_err_and(|why| why.contains("wrote no key to /config/config.xml within 60")),
        "{said:?}"
    );
}

#[tokio::test]
async fn a_service_publishing_no_port_cannot_be_first_run() {
    let Some((_source, copy)) = copy("first-portless") else {
        return;
    };
    let Some(placed) = placed(SERVARR) else {
        return;
    };
    let portless = Served {
        port: None,
        ..served()
    };
    assert_eq!(
        first_run_on(&placed, &copy, Fake::silent(), &portless).await,
        Err("it publishes no port, so its first run cannot reach it".to_owned())
    );
}

#[tokio::test]
async fn a_key_its_adapter_names_no_file_for_cannot_be_read() {
    let Some((_source, copy)) = copy("first-pathless") else {
        return;
    };
    let Some(placed) =
        placed("api         = { kind = \"servarr\", key_source = \"config-xml\", version = 3 }")
    else {
        return;
    };
    assert_eq!(
        first_run(&placed, &copy, Fake::silent()).await,
        Err("its adapter names no file its key is written to".to_owned())
    );
}
