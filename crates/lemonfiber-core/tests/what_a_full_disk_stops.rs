//! What a disk with no room left stops, driven through the HTTP port against a fake
//! transport and a volume a fixture describes as full.
//!
//! Driven from here as well as in-crate, and not instead: the app layer is compiled
//! twice, and a branch exercised from only one of the two copies is counted as never run
//! in the other.
//!
//! **The seam is the block rather than the sentence.** What the house is *told* when the
//! disk is full is held next door, in
//! [`what_the_household_is_told_before_they_ask.rs`](what_the_household_is_told_before_they_ask.rs);
//! this is about the asking being stopped and given back, which is the half a heading
//! cannot do on its own.

use std::sync::Arc;

use lemonfiber_core::app::{dispatch, Command, Ctx};
use lemonfiber_core::config::Settings;
use lemonfiber_core::platform::Environment;
use lemonfiber_core::ports::filesystem::{FsKind, StorageFacts};
use lemonfiber_core::ports::http::{Method, Request};
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::ports::Stopped;
use lemonfiber_fixtures::support::{spoke, Reporting, Scripted, SeedFs};
use lemonfiber_ports::docker::{Health, Lifecycle};

/// The account identifier the request service files this household's member under.
const MEMBER: &str = "4";

/// The stack this repository ships.
fn stack() -> Source {
    Source::External(std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/media-stack"
    )))
}

/// A scratch directory of this test's own, emptied first.
fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("lemonfiber-full-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Where a context keeps what the disk is holding back.
fn record_of(ctx: &Ctx) -> std::path::PathBuf {
    ctx.settings
        .env_file
        .as_ref()
        .map(|env| env.with_file_name("held-back.json"))
        .unwrap_or_default()
}

/// What a volume with nothing left on it reports.
///
/// A total that was read and nothing free. A total of nought is a volume nobody could
/// measure, which is not the same answer and does not halt anything.
fn exhausted() -> StorageFacts {
    StorageFacts {
        point: std::path::PathBuf::new(),
        kind: FsKind::Linking("test".to_owned()),
        removable: false,
        available: 0,
        total: 4 * 1024 * 1024 * 1024 * 1024,
    }
}

/// What a volume with room on it reports.
fn roomy() -> StorageFacts {
    StorageFacts {
        available: 3 * 1024 * 1024 * 1024 * 1024,
        ..exhausted()
    }
}

/// The routes, with any broken rule ahead of the working ones.
fn table(broken: Vec<(Option<Method>, &'static str, Answer)>) -> Arc<Fake> {
    let mut routes = broken;
    routes.extend(vec![
        // Ahead of `/Users`, whose text it contains: a route matched by prefix would
        // answer the sign-in with the list of accounts.
        (
            None,
            "/Users/AuthenticateByName",
            Answer::reply(200, r#"{"AccessToken":"token"}"#),
        ),
        (
            None,
            "/Library/MediaFolders",
            Answer::reply(200, r#"{"Items":[]}"#),
        ),
        (
            None,
            "/Localization/ParentalRatings",
            Answer::reply(200, "[]"),
        ),
        (
            None,
            "/Users",
            Answer::reply(
                200,
                r#"[{"Id":"a1","Name":"Alex","HasPassword":true,
                    "Policy":{"EnableAllFolders":true}}]"#,
            ),
        ),
        (None, "/auth/jellyfin", Answer::reply(200, "{}")),
        (
            None,
            "/settings/main",
            Answer::reply(
                200,
                r#"{"defaultPermissions":32,"defaultQuotas":{"movie":{},"tv":{}}}"#,
            ),
        ),
        (
            None,
            "/user/jellyfin/",
            Answer::reply(200, r#"{"id":4,"permissions":32}"#),
        ),
        (
            None,
            "/user/4/quota",
            Answer::reply(
                200,
                r#"{"movie":{"days":7,"limit":5,"used":1},"tv":{"days":7,"limit":0,"used":0}}"#,
            ),
        ),
        (
            None,
            "settings/permissions",
            Answer::reply(200, r#"{"permissions":32}"#),
        ),
        (
            None,
            "/api/v1/request",
            Answer::reply(200, r#"{"pageInfo":{"results":0},"results":[]}"#),
        ),
        (None, "", Answer::reply(200, "[]")),
    ]);
    Fake::by_rules(routes)
}

/// An install over the given transport, on a volume described as `facts` says.
fn context(name: &str, transport: &Arc<Fake>, facts: StorageFacts) -> Ctx {
    let dir = scratch(name);
    let env = dir.join(".env");
    let _ = lemonfiber_core::config::store::set(
        &env,
        lemonfiber_core::config::JELLYFIN_ADMIN_PASSWORD_KEY,
        "minted-earlier",
    );
    Ctx::new(
        Arc::new(Scripted(Ok(spoke("")))),
        Arc::new(Reporting::holding(
            &["jellyfin", "seerr"],
            Lifecycle::Running,
            Health::Healthy,
        )),
        Stopped::today(),
        Arc::new(SeedFs::keyed(None, None).with_facts(facts)),
        stack(),
        Settings {
            env_file: Some(env),
            data_root: Some(dir),
            ..Settings::default()
        },
        Environment::MacOs,
    )
    .with_http(transport.clone())
}

/// Every body written to the narrow permissions endpoint, in order.
fn permissions_written(transport: &Arc<Fake>) -> Vec<String> {
    transport
        .requests()
        .into_iter()
        .filter(|request: &Request| {
            request.url.contains("settings/permissions") && request.method == Method::Post
        })
        .filter_map(|request| request.body)
        .collect()
}

/// Everything the reading said beside the list itself.
async fn findings(ctx: &Ctx) -> String {
    dispatch(Command::Household { member: None }, ctx)
        .await
        .ok()
        .map(lemonfiber_core::app::Outcome::envelope)
        .and_then(|envelope| envelope.to_json())
        .unwrap_or_default()
}

/// A full disk stops the household asking, and writes down what it took.
///
/// The one instrument the request service offers is the permission, and taking it away
/// is a button that is simply gone — which is why it goes out on the same reading that
/// hangs the heading saying the disk has no room and that this is nobody's limit.
#[tokio::test]
async fn a_full_disk_stops_the_household_asking() {
    let transport = table(Vec::new());
    let ctx = context("stopped", &transport, exhausted());

    let said = findings(&ctx).await;

    assert!(
        said.contains("to_hand_over"),
        "the reading itself was lost: {said}"
    );
    assert_eq!(
        permissions_written(&transport),
        vec![r#"{"permissions":0}"#.to_owned()],
        "the household was left able to ask for what cannot be fetched"
    );
    let kept = std::fs::read_to_string(record_of(&ctx)).unwrap_or_default();
    assert!(
        kept.contains(&format!(r#""{MEMBER}":32"#)),
        "what was taken was not written down: {kept}"
    );
}

/// A rehearsal takes nothing away from anybody, however full the disk is.
#[tokio::test]
async fn a_rehearsal_takes_nothing_away() {
    let transport = table(Vec::new());
    let ctx = context("rehearsed", &transport, exhausted());

    let said = dispatch(Command::Household { member: None }, &ctx.rehearsing()).await;

    assert!(said.is_ok(), "a rehearsed reading did not answer");
    assert!(
        permissions_written(&transport).is_empty(),
        "a rehearsal stopped a household asking"
    );
}

/// A disk with room again gives back exactly what was taken, and nothing else.
///
/// The permission an operator narrowed while the disk was full stays narrowed: what
/// goes back is the number that came off, not an account restored from a default.
#[tokio::test]
async fn a_disk_with_room_gives_back_exactly_what_was_taken() {
    let transport = table(vec![(
        None,
        "settings/permissions",
        Answer::reply(200, r#"{"permissions":4194304}"#),
    )]);
    let ctx = context("given-back", &transport, roomy());
    let _ = std::fs::write(record_of(&ctx), format!(r#"{{"{MEMBER}":32}}"#));

    let said = findings(&ctx).await;

    assert!(
        said.contains("to_hand_over"),
        "the reading itself was lost: {said}"
    );
    assert_eq!(
        permissions_written(&transport),
        vec![r#"{"permissions":4194336}"#.to_owned()],
        "what was given back was not what was taken"
    );
    let kept = std::fs::read_to_string(record_of(&ctx)).unwrap_or_default();
    assert_eq!(
        kept, "{}",
        "somebody stayed written down as held back: {kept}"
    );
}

/// A service that will not stop the asking says so, and writes nobody down.
#[tokio::test]
async fn a_service_that_will_not_stop_the_asking_says_so() {
    let transport = table(vec![(
        None,
        "settings/permissions",
        Answer::reply(500, "no"),
    )]);
    let ctx = context("unstoppable", &transport, exhausted());

    let said = findings(&ctx).await;

    assert!(
        said.contains("would not stop the household asking"),
        "a household still able to ask for what cannot be fetched was passed over: {said}"
    );
    assert!(
        !record_of(&ctx).exists(),
        "nothing was taken and somebody was written down as held back anyway"
    );
}

/// A service that will not give it back keeps the record, so the next reading tries.
#[tokio::test]
async fn a_service_that_will_not_give_it_back_keeps_the_record() {
    let transport = table(vec![(
        None,
        "settings/permissions",
        Answer::reply(500, "no"),
    )]);
    let ctx = context("still-held", &transport, roomy());
    let _ = std::fs::write(record_of(&ctx), format!(r#"{{"{MEMBER}":32}}"#));

    let said = findings(&ctx).await;

    assert!(
        said.contains("give the household back"),
        "a household left unable to ask was passed over in silence: {said}"
    );
    let kept = std::fs::read_to_string(record_of(&ctx)).unwrap_or_default();
    assert!(
        kept.contains(&format!(r#""{MEMBER}":32"#)),
        "the record was forgotten, so nothing would ever give it back: {kept}"
    );
}

/// An owner is not written down as held back, because nothing was taken from them.
#[tokio::test]
async fn an_owner_is_not_written_down_as_held_back() {
    let transport = table(vec![(
        None,
        "settings/permissions",
        Answer::reply(200, r#"{"permissions":2}"#),
    )]);
    let ctx = context("the-owner", &transport, exhausted());

    let said = findings(&ctx).await;

    assert!(
        said.contains("to_hand_over"),
        "the reading itself was lost: {said}"
    );
    assert!(
        permissions_written(&transport).is_empty(),
        "the owner's own account was written to for no effect"
    );
    assert!(
        !record_of(&ctx).exists(),
        "the owner was written down as held back"
    );
}

/// A member the request service no longer holds is forgotten rather than carried.
///
/// An account that has gone is nothing to give anything back to, and a record that kept
/// the line would carry somebody who left for as long as the household did.
#[tokio::test]
async fn a_member_the_service_no_longer_holds_is_forgotten() {
    let transport = table(vec![(
        None,
        "settings/permissions",
        Answer::reply(404, r#"{"message":"User not found."}"#),
    )]);
    let ctx = context("gone", &transport, roomy());
    let _ = std::fs::write(record_of(&ctx), format!(r#"{{"{MEMBER}":32}}"#));

    let said = findings(&ctx).await;

    assert!(
        said.contains("to_hand_over"),
        "the reading itself was lost: {said}"
    );
    assert!(
        permissions_written(&transport).is_empty(),
        "an account that is gone was written to"
    );
    let kept = std::fs::read_to_string(record_of(&ctx)).unwrap_or_default();
    assert_eq!(kept, "{}", "somebody who left stayed written down: {kept}");
}

/// Nothing is taken from a member the request service no longer holds either.
#[tokio::test]
async fn nothing_is_taken_from_a_member_who_is_gone() {
    let transport = table(vec![(
        None,
        "settings/permissions",
        Answer::reply(404, r#"{"message":"User not found."}"#),
    )]);
    let ctx = context("gone-full", &transport, exhausted());

    let said = findings(&ctx).await;

    assert!(
        said.contains("to_hand_over"),
        "the reading itself was lost: {said}"
    );
    assert!(
        permissions_written(&transport).is_empty(),
        "an account that is gone was written to"
    );
    assert!(
        !record_of(&ctx).exists(),
        "an account that is gone was written down as held back"
    );
}

/// A record that cannot be written is said out loud rather than swallowed.
#[tokio::test]
async fn a_record_that_cannot_be_written_is_said_out_loud() {
    let transport = table(Vec::new());
    let ctx = context("unwritable", &transport, exhausted());
    // A directory standing where the record goes, which is the one way to make the
    // write fail without making the settings beside it unreachable as well.
    let _ = std::fs::create_dir_all(record_of(&ctx));

    let said = findings(&ctx).await;

    assert!(
        said.contains("could not be written down"),
        "a household held back with no record of it was passed over: {said}"
    );
}
