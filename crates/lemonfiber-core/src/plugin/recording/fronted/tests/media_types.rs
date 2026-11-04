use std::path::Path;
use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::proving::Proving;
use lemonfiber_fixtures::support::FixedRandom;

use super::super::super::{record, Recorded, Recorder, Retaking};
use super::{engine, source, FRONTED};
use crate::plugin::claimed::Unreadable;
use crate::plugin::conformed;
use crate::ports::http::Http;

/// The fronted fixture, its adapter speaking `media.serve@1` for movies and tv, and its
/// upstream declaring `upstream` as the media it files.
fn serving_media(upstream: &str) -> String {
    FRONTED
        .replace(
            "speaks      = [\"subtitles.fetch@1\"]\nfronts      = \"jellyfin\"\nprovides    = [\"subtitles.fetch\"]\n",
            "speaks      = [\"media.serve@1\"]\nfronts      = \"jellyfin\"\nprovides    = [\"media.serve\"]\nmedia_types = [\"movies\", \"tv\"]\n",
        )
        .replace(
            "takes_data  = true\nconfig_path = \"/config\"\nlistens     = 8096\n",
            &format!("takes_data  = true\n{upstream}config_path = \"/config\"\nlistens     = 8096\n"),
        )
}

async fn taken(at: &Path, http: &Arc<Fake>, check: bool) -> Result<Recorded, Unreadable> {
    let program = Proving::unreachable();
    let shared = || -> Arc<dyn Http> { Arc::clone(http) as Arc<dyn Http> };
    record(
        at,
        Recorder {
            program: program.as_ref(),
            serving: &engine(),
            transport: &shared,
            random: &FixedRandom(None),
            taking: &Retaking {
                check,
                ..Retaking::default()
            },
            confined: &lemonfiber_adapters::Disk,
            patience: std::time::Duration::from_secs(2),
        },
    )
    .await
}

fn names_the_fix(said: &[lemonfiber_plugin::Violation]) -> bool {
    said.iter().any(|one| {
        one.location == "service jellyfin.media_types"
            && one.message.contains("declares no media types")
            && one.message.contains("media.serve@1")
            && one
                .message
                .contains("declare media_types = [\"movies\", \"tv\"] on the [[service]] jellyfin")
    })
}

#[tokio::test]
async fn record_refuses_an_upstream_declaring_no_media_types_and_asks_nothing() {
    let at = source("media-types-record", &serving_media(""));
    let http = Fake::always(Answer::reply(200, "true"));
    let read = taken(&at, &http, false).await;
    assert!(
        read.as_ref().is_ok_and(|read| names_the_fix(&read.refusals)
            && read.recordings.is_empty()
            && !read.holds),
        "{read:?}"
    );
    assert!(http.requests().is_empty());
}

#[tokio::test]
async fn record_check_refuses_an_upstream_declaring_no_media_types_and_asks_nothing() {
    let at = source("media-types-check", &serving_media(""));
    let http = Fake::always(Answer::reply(200, "true"));
    let read = taken(&at, &http, true).await;
    assert!(
        read.as_ref().is_ok_and(|read| names_the_fix(&read.refusals)
            && read.recordings.is_empty()
            && !read.holds),
        "{read:?}"
    );
    assert!(http.requests().is_empty());
}

#[tokio::test]
async fn conform_refuses_an_upstream_declaring_no_media_types() {
    let at = source("media-types-conform", &serving_media(""));
    let read = conformed(&at).await;
    assert!(
        read.as_ref()
            .is_ok_and(|read| names_the_fix(&read.refusals) && !read.conforms),
        "{read:?}"
    );
}

#[tokio::test]
async fn an_upstream_declaring_its_adapter_s_media_types_is_not_refused_for_them() {
    let at = source(
        "media-types-declared",
        &serving_media("media_types = [\"movies\", \"tv\"]\n"),
    );
    let read = conformed(&at).await;
    assert!(
        read.as_ref().is_ok_and(|read| !read
            .refusals
            .iter()
            .any(|one| one.location.ends_with(".media_types"))),
        "{read:?}"
    );
}
