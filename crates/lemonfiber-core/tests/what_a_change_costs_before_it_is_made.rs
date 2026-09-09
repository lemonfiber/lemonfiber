//! Changing one setting, driven through the dispatcher: what the change would do,
//! what it takes to make it, and what happens to the file when it cannot be made.
//!
//! From here rather than only from a `#[cfg(test)]` module for the reason the forms
//! listing and the machine's own commands are: the app layer is compiled twice, and a
//! path exercised only in-crate has its coverage counted from the copy that never ran.
//! The envelope is serialised here too, since what a browser reads about a staged
//! change is the core's promise rather than the terminal's.
//!
//! **What is asserted is the file on disk beside the answer.** A report that said a
//! change was staged while the setting had already moved would be the failure the whole
//! gate exists to prevent, and a report is not evidence of its own truthfulness.

mod common;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use common::stack::project;
use lemonfiber_core::adapters::{Daemon, Disk, Local, System};
use lemonfiber_core::app::{dispatch, Command, Ctx, Outcome, Setting};
use lemonfiber_core::config::{
    store, Settings, DATA_ROOT_KEY, INDEXER_APIKEY_KEY, PROVIDER_PORT_KEY,
};
use lemonfiber_core::model::ConfigReport;
use lemonfiber_core::platform::Environment;
use lemonfiber_core::reconfigure::{Cost, Stance};
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::http::{Answer, Fake};

/// A search a Torznab indexer answers with, which proves a key.
const ANSWERED: &str = "<rss><channel><item/></channel></rss>";

/// A settings file at a scratch path of this test's own, holding `contents`.
fn env_at(name: &str, contents: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("lemonfiber-review-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.join(".env");
    let written = std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(&path, contents));
    assert!(written.is_ok(), "the scratch settings file is written");
    path
}

/// A machine whose settings live at `env_file`.
fn ctx(env_file: PathBuf) -> Ctx {
    Ctx::new(
        Arc::new(Local),
        Arc::new(Daemon::local()),
        Arc::new(System),
        Arc::new(Disk),
        Source::External(project()),
        Settings {
            env_file: Some(env_file),
            ..Settings::default()
        },
        Environment::MacOs,
    )
}

/// The same, reaching every service through a transport that answers `answer`.
fn reaching(env_file: PathBuf, answer: Answer) -> Ctx {
    ctx(env_file).with_http(Fake::always(answer))
}

/// What a change to one setting came to.
async fn changing(ctx: &Ctx, key: &str, value: &str, confirmed: bool) -> Option<ConfigReport> {
    let asked = Command::ConfigSet(Setting::to(key, value).agreed(confirmed));
    match dispatch(asked, ctx).await {
        Ok(Outcome::Config(report)) => Some(report),
        _ => None,
    }
}

/// What a setting holds on disk now.
fn on_disk(path: &Path, key: &str) -> Option<String> {
    store::read(path)
        .ok()
        .and_then(|file| file.get(key).map(str::to_owned))
}

#[tokio::test]
async fn a_consequential_change_is_staged_with_its_difference_and_its_cost() {
    let path = env_at("staged", "DATA_ROOT=/srv/old\n");
    let staged = changing(&ctx(path.clone()), DATA_ROOT_KEY, "/srv/new", false).await;

    let review = staged.as_ref().and_then(|report| report.review.clone());
    assert_eq!(
        review.as_ref().map(|review| review.stance),
        Some(Stance::Pending)
    );
    assert_eq!(
        review.as_ref().map(|review| review.change.cost),
        Some(Cost::Consequential)
    );
    assert_eq!(
        review.map(|review| (review.change.from, review.change.to)),
        Some((Some("/srv/old".to_owned()), "/srv/new".to_owned()))
    );
    assert!(
        staged
            .and_then(|report| report.consequence)
            .is_some_and(|said| said.contains("points at nothing")),
        "the sharpest consequence in the product was not stated before the change"
    );
    assert_eq!(
        on_disk(&path, DATA_ROOT_KEY).as_deref(),
        Some("/srv/old"),
        "a change nobody agreed to reached the file"
    );
}

#[tokio::test]
async fn the_same_change_confirmed_is_the_one_that_lands() {
    let path = env_at("confirmed", "DATA_ROOT=/srv/old\n");
    let applied = changing(&ctx(path.clone()), DATA_ROOT_KEY, "/srv/new", true).await;

    assert_eq!(
        applied.and_then(|report| report.review).map(|r| r.stance),
        Some(Stance::Applied)
    );
    assert_eq!(on_disk(&path, DATA_ROOT_KEY).as_deref(), Some("/srv/new"));
}

/// What a browser is handed about a staged change, which is the core's promise
/// rather than anything the terminal decided about how to say it.
#[tokio::test]
async fn a_staged_change_serialises_with_its_difference_and_where_it_stands() {
    let path = env_at("envelope", "DATA_ROOT=/srv/old\n");
    let asked = Command::ConfigSet(Setting::to(DATA_ROOT_KEY, "/srv/new"));
    let json = dispatch(asked, &ctx(path))
        .await
        .ok()
        .map(Outcome::envelope)
        .and_then(|envelope| envelope.to_json())
        .unwrap_or_default();

    assert!(json.contains(r#""kind":"config""#), "{json}");
    assert!(json.contains(r#""stance":"pending""#), "{json}");
    assert!(json.contains(r#""cost":"consequential""#), "{json}");
    assert!(json.contains(r#""from":"/srv/old""#), "{json}");
    assert!(json.contains(r#""to":"/srv/new""#), "{json}");
}

#[tokio::test]
async fn a_replacement_key_the_indexer_refuses_leaves_the_working_one_in_place() {
    let path = env_at(
        "refused",
        "INDEXER_URL=https://indexer.example/api\nINDEXER_APIKEY=old-key\n",
    );
    let ctx = reaching(path.clone(), Answer::reply(401, ""));
    let outcome = changing(&ctx, INDEXER_APIKEY_KEY, "mistyped", false).await;

    let review = outcome.and_then(|report| report.review);
    assert_eq!(
        review.as_ref().map(|review| review.stance),
        Some(Stance::Blocked)
    );
    assert!(
        review
            .and_then(|review| review.refusal)
            .is_some_and(|why| why.contains("the one in force was kept")),
        "the refusal says nothing about what was kept"
    );
    assert_eq!(
        on_disk(&path, INDEXER_APIKEY_KEY).as_deref(),
        Some("old-key")
    );
}

#[tokio::test]
async fn a_replacement_key_the_indexer_accepts_is_the_one_that_lands() {
    let path = env_at(
        "accepted",
        "INDEXER_URL=https://indexer.example/api\nINDEXER_APIKEY=old-key\n",
    );
    let ctx = reaching(path.clone(), Answer::reply(200, ANSWERED));
    let outcome = changing(&ctx, INDEXER_APIKEY_KEY, "new-key", false).await;

    assert_eq!(
        outcome.and_then(|report| report.review).map(|r| r.stance),
        Some(Stance::Applied)
    );
    assert_eq!(
        on_disk(&path, INDEXER_APIKEY_KEY).as_deref(),
        Some("new-key")
    );
}

/// A value the product cannot read is not a credential question at all: nothing
/// could be dialled and nothing could correct it, so the file is left alone.
#[tokio::test]
async fn a_value_that_cannot_be_read_leaves_the_file_alone_and_says_why() {
    let path = env_at(
        "unreadable",
        "USENET_HOST=news.example.net\nUSENET_PORT=563\nUSENET_USER=someone\n\
         USENET_PASS=old-pass\nUSENET_TLS=on\n",
    );
    let outcome = changing(&ctx(path.clone()), PROVIDER_PORT_KEY, "not-a-port", true).await;

    let review = outcome.and_then(|report| report.review);
    assert_eq!(
        review.as_ref().map(|review| review.stance),
        Some(Stance::Blocked)
    );
    assert!(
        review
            .and_then(|review| review.refusal)
            .is_some_and(|why| why.contains("1 to 65535")),
        "the refusal names nothing a port could be corrected to"
    );
    assert_eq!(on_disk(&path, PROVIDER_PORT_KEY).as_deref(), Some("563"));
}
