use std::path::Path;
use std::sync::Arc;

use lemonfiber_fixtures::http::Fake;
use lemonfiber_fixtures::proving::Proving;
use lemonfiber_fixtures::serving::Serves;
use lemonfiber_fixtures::support::FixedRandom;

use super::{answering, engine, recorded, source, FRONTED, PROVED, WATCH};
use crate::plugin::recording::{record, Recorded, Recorder, Retaking, Took};
use crate::ports::http::Http;

const BUILT: &str = "plugin-jellyfin:pull-request";
const STRAY: &str = "conformance/subtitles.fetch@1/retired-case.json";

async fn checked(at: &Path, serving: &Serves, http: &Arc<Fake>, adapter: Option<&str>) -> Recorded {
    let program = Proving::unreachable();
    let shared = || -> Arc<dyn Http> { Arc::clone(http) as Arc<dyn Http> };
    let taking = Retaking {
        check: true,
        adapter: adapter.map(str::to_owned),
        built: None,
    };
    let read = record(
        at,
        Recorder {
            program: program.as_ref(),
            serving,
            transport: &shared,
            random: &FixedRandom(Some(vec![7; 32])),
            taking: &taking,
            confined: &lemonfiber_adapters::Disk,
        },
    )
    .await;
    assert!(read.is_ok(), "{read:?}");
    read.unwrap_or_else(|_| unreachable!("asserted to be read above"))
}

fn noted(at: &Path) {
    for fixture in [PROVED, WATCH, super::WATCHING, super::REFUSES] {
        let at = at.join(fixture);
        let text = std::fs::read_to_string(&at).unwrap_or_default();
        let _ = std::fs::write(
            &at,
            text.replacen("\"note\": \"\"", "\"note\": \"Why.\"", 1),
        );
    }
}

fn texts(at: &Path) -> Vec<String> {
    [PROVED, WATCH, super::WATCHING, super::REFUSES]
        .iter()
        .map(|fixture| std::fs::read_to_string(at.join(fixture)).unwrap_or_default())
        .collect()
}

fn outcomes(read: &Recorded) -> Vec<Took> {
    read.recordings.iter().map(|one| one.took.clone()).collect()
}

#[tokio::test]
async fn a_check_asks_the_adapter_it_names_and_holds_every_recording_to_it() {
    let at = source("fronted-checked", FRONTED);
    let _ = recorded(&at, &engine(), &answering(), Some(7)).await;
    noted(&at);
    let before = texts(&at);

    let serving = engine();
    let read = checked(&at, &serving, &answering(), Some(BUILT)).await;

    assert_eq!(outcomes(&read), vec![Took::Reproduced; 4]);
    assert!(read.holds);
    assert_eq!(texts(&at), before);
    let images: Vec<String> = serving.asked().into_iter().map(|one| one.image).collect();
    assert_eq!(images.get(1).map(String::as_str), Some(BUILT));
    assert_eq!(
        images.first().map(String::as_str),
        Some(super::PINNED),
        "the upstream is still served from its pin"
    );
}

#[tokio::test]
async fn a_check_without_an_image_starts_the_adapter_from_its_pin() {
    let at = source("fronted-checked-pinned", FRONTED);
    let _ = recorded(&at, &engine(), &answering(), Some(7)).await;
    noted(&at);

    let serving = engine();
    let read = checked(&at, &serving, &answering(), None).await;

    assert!(read.holds);
    assert!(serving.asked().get(1).is_some_and(|one| one
        .image
        .starts_with("ghcr.io/lemonfiber/jellyfin-adapter@sha256:")));
}

#[tokio::test]
async fn a_check_names_what_differs_what_is_missing_and_what_no_case_asks_and_writes_nothing() {
    let at = source("fronted-checked-differs", FRONTED);
    let _ = recorded(&at, &engine(), &answering(), Some(7)).await;
    noted(&at);
    let watch = at.join(WATCH);
    let changed = std::fs::read_to_string(&watch).unwrap_or_default().replace(
        "\"body_starts_with\": \"null\"",
        "\"body_starts_with\": \"[]\"",
    );
    let _ = std::fs::write(&watch, &changed);
    let _ = std::fs::remove_file(at.join(PROVED));
    let _ = std::fs::write(at.join(STRAY), "{}");

    let read = checked(&at, &engine(), &answering(), Some(BUILT)).await;

    assert!(!read.holds);
    assert_eq!(
        outcomes(&read),
        vec![
            Took::Unreproduced {
                why: format!("{PROVED} is not there"),
            },
            Took::Reproduced,
            Took::Reproduced,
            Took::Unreproduced {
                why: format!("{WATCH} is not what was taken again"),
            },
            Took::Unasked,
        ]
    );
    assert_eq!(std::fs::read_to_string(&watch).unwrap_or_default(), changed);
    assert!(!at.join(PROVED).exists());
    assert!(at.join(STRAY).exists());
}

#[tokio::test]
async fn writing_removes_what_no_case_asks() {
    let at = source("fronted-unasked-removed", FRONTED);
    let _ = recorded(&at, &engine(), &answering(), Some(7)).await;
    noted(&at);
    let _ = std::fs::write(at.join(STRAY), "{}");

    let read = recorded(&at, &engine(), &answering(), Some(7)).await;

    assert!(read.holds, "{read:?}");
    assert_eq!(outcomes(&read).last(), Some(&Took::Removed));
    assert!(!at.join(STRAY).exists());
}

#[tokio::test]
async fn a_check_names_a_recording_taken_again_that_holds_no_note() {
    let at = source("fronted-checked-unnoted", FRONTED);
    let _ = recorded(&at, &engine(), &answering(), Some(7)).await;
    let before = texts(&at);

    let read = checked(&at, &engine(), &answering(), Some(BUILT)).await;

    assert!(!read.holds);
    assert_eq!(outcomes(&read), vec![Took::Unnoted; 4]);
    assert_eq!(texts(&at), before);
}

#[tokio::test]
async fn writing_from_the_adapter_it_names_writes_what_that_adapter_answered() {
    let at = source("fronted-written-from-built", FRONTED);
    let serving = engine();
    let program = Proving::unreachable();
    let http = answering();
    let shared = || -> Arc<dyn Http> { Arc::clone(&http) as Arc<dyn Http> };
    let taking = Retaking {
        check: false,
        adapter: Some(BUILT.to_owned()),
        built: None,
    };
    let read = record(
        &at,
        Recorder {
            program: program.as_ref(),
            serving: &serving,
            transport: &shared,
            random: &FixedRandom(Some(vec![7; 32])),
            taking: &taking,
            confined: &lemonfiber_adapters::Disk,
        },
    )
    .await;

    assert!(
        read.as_ref().is_ok_and(|read| read.failed.is_empty()),
        "{read:?}"
    );
    assert!(at.join(WATCH).exists(), "the adapter's answers are written");
    let images: Vec<String> = serving.asked().into_iter().map(|one| one.image).collect();
    assert_eq!(images.get(1).map(String::as_str), Some(BUILT));
    assert_eq!(images.first().map(String::as_str), Some(super::PINNED));
}
