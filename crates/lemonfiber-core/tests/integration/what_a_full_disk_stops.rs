//! What a disk with no room left stops, driven through the HTTP port against a fake
//! transport and a volume a fixture describes as full.
//!
//! Driven from here as well as in-crate, and not instead: the app layer is compiled
//! twice, and a branch exercised from only one of the two copies is counted as never run
//! in the other.
//!
//! **The seam is the block rather than the sentence.** This is about the asking being
//! stopped and given back, which is the half a heading cannot do on its own.

use std::sync::Arc;

use lemonfiber_core::app::{dispatch, Command, Ctx};
use lemonfiber_core::config::Settings;
use lemonfiber_core::ports::filesystem::{FsKind, StorageFacts};
use lemonfiber_core::ports::service::{Holding, Invited};
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::support::{Reporting, SeedFs};
use lemonfiber_ports::docker::{Health, Lifecycle};
use lemonfiber_testing::media;
use lemonfiber_testing::requests::{self, REQUESTS};

use crate::common::household::MEMBER;

/// A scratch directory of this test's own, emptied first.
fn scratch(name: &str) -> lemonfiber_fixtures::scratch::Scratch {
    lemonfiber_fixtures::scratch::Scratch::new(name)
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

/// What the request service took off the member, in its own numbering.
const TAKEN: u64 = 32;

/// The routes, with any broken rule ahead of the working ones: a request service taking
/// [`TAKEN`] off the member when asked to hold them back, and giving back what it is
/// handed.
fn table(broken: Vec<(&'static str, Answer)>) -> Arc<Fake> {
    let mut routes = broken;
    routes.extend([
        (
            media::identity("when_invited"),
            media::answered(&Vec::<Invited>::new()),
        ),
        requests::answering("hold_requests", &Holding { taken: TAKEN }),
        (requests::intake("release_requests"), requests::taken()),
    ]);
    crate::common::household::table(routes)
}

/// An install over the given transport, on a volume described as `facts` says.
fn context(name: &str, transport: &Arc<Fake>, facts: StorageFacts) -> Ctx {
    let dir = scratch(name).kept();
    let env = dir.join(".env");
    let _ = lemonfiber_core::config::store::set(
        &env,
        lemonfiber_core::config::MEDIA_SERVER_ADMIN_PASSWORD_KEY,
        "minted-earlier",
    );
    REQUESTS.keyed_in(media::keyed_in(
        lemonfiber_testing::a_context()
            .engine(Arc::new(
                Reporting::holding(&["jellyfin"], Lifecycle::Running, Health::Healthy)
                    .alongside(media::running())
                    .alongside(REQUESTS.running()),
            ))
            .filesystem(Arc::new(SeedFs::keyed(None).with_facts(facts)))
            .settings(Settings {
                env_file: Some(env),
                data_root: Some(dir),
                ..Settings::default()
            })
            .build()
            .with_http(transport.clone()),
    ))
}

/// Each member the request service was asked to hold back, in order.
fn held_back(transport: &Arc<Fake>) -> Vec<String> {
    requests::told(&transport.requests(), "hold_requests", "id")
}

/// What the request service was handed back, in order.
fn given_back(transport: &Arc<Fake>) -> Vec<u64> {
    requests::told::<Holding>(&transport.requests(), "release_requests", "holding")
        .into_iter()
        .map(|holding| holding.taken)
        .collect()
}

/// What the record says was taken off the member.
fn written_down() -> String {
    format!(r#"{{"{MEMBER}":{TAKEN}}}"#)
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
        held_back(&transport),
        vec![MEMBER.to_owned()],
        "the household was left able to ask for what cannot be fetched"
    );
    let kept = std::fs::read_to_string(record_of(&ctx)).unwrap_or_default();
    assert_eq!(kept, written_down(), "what was taken was not written down");
}

/// A rehearsal takes nothing away from anybody, however full the disk is.
#[tokio::test]
async fn a_rehearsal_takes_nothing_away() {
    let transport = table(Vec::new());
    let ctx = context("rehearsed", &transport, exhausted());

    let said = dispatch(Command::Household { member: None }, &ctx.rehearsing()).await;

    assert!(said.is_ok(), "a rehearsed reading did not answer");
    assert!(
        held_back(&transport).is_empty(),
        "a rehearsal stopped a household asking"
    );
}

/// A disk with room again gives back exactly what was taken, and nothing else.
///
/// The permission an operator narrowed while the disk was full stays narrowed: what
/// goes back is the number that came off, not an account restored from a default.
#[tokio::test]
async fn a_disk_with_room_gives_back_exactly_what_was_taken() {
    let transport = table(Vec::new());
    let ctx = context("given-back", &transport, roomy());
    let _ = std::fs::write(record_of(&ctx), written_down());

    let said = findings(&ctx).await;

    assert!(
        said.contains("to_hand_over"),
        "the reading itself was lost: {said}"
    );
    assert_eq!(
        given_back(&transport),
        vec![TAKEN],
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
        requests::intake("hold_requests"),
        requests::unavailable(),
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
        requests::intake("release_requests"),
        requests::unavailable(),
    )]);
    let ctx = context("still-held", &transport, roomy());
    let _ = std::fs::write(record_of(&ctx), written_down());

    let said = findings(&ctx).await;

    assert!(
        said.contains("give the household back"),
        "a household left unable to ask was passed over in silence: {said}"
    );
    let kept = std::fs::read_to_string(record_of(&ctx)).unwrap_or_default();
    assert_eq!(
        kept,
        written_down(),
        "the record was forgotten, so nothing would ever give it back"
    );
}

/// Somebody nothing was taken from is not written down as held back: an owner, whose
/// asking the request service reads past, or somebody it no longer holds.
#[tokio::test]
async fn somebody_nothing_was_taken_from_is_not_written_down_as_held_back() {
    let transport = table(vec![requests::answering(
        "hold_requests",
        &Holding { taken: 0 },
    )]);
    let ctx = context("the-owner", &transport, exhausted());

    let said = findings(&ctx).await;

    assert!(
        said.contains("to_hand_over"),
        "the reading itself was lost: {said}"
    );
    assert!(
        !record_of(&ctx).exists(),
        "somebody nothing was taken from was written down as held back"
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
