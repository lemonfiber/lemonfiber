//! A full disk stopping the household asking, and giving back what it took.

use super::*;

/// The same, on a volume with no room left, keeping the transport so what was
/// written to the request service can be read back off it.
fn no_room_with(fake: &Fake, tag: &str) -> (Ctx, Arc<Transport>) {
    let transport = fake.transport();
    (ctx_over(Arc::clone(&transport), tag, true), transport)
}

/// Where this context keeps what the disk is holding back.
fn record_of(ctx: &Ctx) -> std::path::PathBuf {
    ctx.settings
        .env_file
        .as_ref()
        .map(|env| env.with_file_name("held-back.json"))
        .unwrap_or_default()
}

/// A household the request service holds an account for, able to ask for things.
fn asking_household(permissions: (u16, &'static str)) -> Fake {
    Fake {
        account: Some(r#"{"id":5,"permissions":32}"#),
        permissions,
        ..Fake::default()
    }
}

/// Every body written to the narrow permissions endpoint, in order.
fn permissions_written(transport: &Arc<Transport>) -> Vec<String> {
    transport
        .requests()
        .into_iter()
        .filter(|request| {
            request.url.contains("/settings/permissions")
                && request.method == crate::ports::http::Method::Post
        })
        .filter_map(|request| request.body)
        .collect()
}

/// A full disk stops the household asking, and writes down what it took.
///
/// The block a full disk needs and the sentence explaining it go out on the one
/// reading. What is taken has to be written down because giving it back is not a
/// grant: what a household may ask for is the operator's to decide.
#[tokio::test]
async fn a_full_disk_stops_the_household_asking_and_writes_down_what_it_took() {
    let (context, transport) = no_room_with(
        &asking_household((200, r#"{"permissions":32}"#)),
        "held-back",
    );

    let report = household(&context, None).await.unwrap_or_default();

    assert!(
        report
            .findings
            .iter()
            .all(|said| !said.contains("would not stop")),
        "{report:?}"
    );
    assert_eq!(
        permissions_written(&transport),
        vec![r#"{"permissions":0}"#.to_owned()],
        "the household was left able to ask for what cannot be fetched"
    );
    let kept = std::fs::read_to_string(record_of(&context)).unwrap_or_default();
    assert!(
        kept.contains(r#""5":32"#),
        "what was taken was not written down: {kept}"
    );
}

/// A disk with room again gives back exactly what was taken, and nothing else.
///
/// The permission the operator narrowed while the disk was full stays narrowed:
/// this puts back the number that came off rather than restoring an account.
#[tokio::test]
async fn a_disk_with_room_gives_back_exactly_what_was_taken() {
    let transport = asking_household((200, r#"{"permissions":4194304}"#)).transport();
    let context = ctx_over(Arc::clone(&transport), "given-back", false);
    let _ = std::fs::write(record_of(&context), r#"{"5":32}"#);

    let report = household(&context, None).await.unwrap_or_default();

    assert!(report.available, "{report:?}");
    assert_eq!(
        permissions_written(&transport),
        vec![r#"{"permissions":4194336}"#.to_owned()],
        "what was given back was not what was taken"
    );
    let kept = std::fs::read_to_string(record_of(&context)).unwrap_or_default();
    assert_eq!(
        kept, "{}",
        "somebody stayed written down as held back: {kept}"
    );
}

/// A service that will not stop the asking says so, rather than reporting a block.
#[tokio::test]
async fn a_service_that_will_not_stop_the_asking_says_so() {
    let (context, _) = no_room_with(&asking_household((500, "no")), "unstoppable");

    let report = household(&context, None).await.unwrap_or_default();

    assert!(
        report
            .findings
            .iter()
            .any(|said| said.contains("would not stop the household asking")),
        "{report:?}"
    );
    assert!(
        !record_of(&context).exists(),
        "nothing was taken and somebody was written down as held back anyway"
    );
}

/// An owner is not written down as held back, because nothing was taken from them.
///
/// The request service reads that permission first and answers yes whatever else
/// is set, so a bit taken off would block nothing and a bit given back would be a
/// change made to their account for no effect.
#[tokio::test]
async fn an_owner_is_not_written_down_as_held_back() {
    let (context, transport) = no_room_with(
        &asking_household((200, r#"{"permissions":2}"#)),
        "the-owner",
    );

    let report = household(&context, None).await.unwrap_or_default();

    assert!(report.available, "{report:?}");
    assert!(
        permissions_written(&transport).is_empty(),
        "the owner's own account was written to"
    );
    assert!(
        !record_of(&context).exists(),
        "the owner was written down as held back"
    );
}

/// A service that will not give it back keeps the record, so the next reading tries.
#[tokio::test]
async fn a_service_that_will_not_give_it_back_keeps_the_record() {
    let context = ctx_over(
        asking_household((500, "no")).transport(),
        "still-held",
        false,
    );
    let _ = std::fs::write(record_of(&context), r#"{"5":32}"#);

    let report = household(&context, None).await.unwrap_or_default();

    assert!(
        report
            .findings
            .iter()
            .any(|said| said.contains("give the household back")),
        "{report:?}"
    );
    let kept = std::fs::read_to_string(record_of(&context)).unwrap_or_default();
    assert!(
        kept.contains(r#""5":32"#),
        "the record was forgotten: {kept}"
    );
}

/// A record that cannot be written is said out loud rather than swallowed.
///
/// Silence there would leave an operator believing a household could be let go
/// again from a note that was never made.
#[tokio::test]
async fn a_record_that_cannot_be_written_is_said_out_loud() {
    let (context, _) = no_room_with(
        &asking_household((200, r#"{"permissions":32}"#)),
        "unwritable",
    );
    // A directory standing where the record goes, which is the one way to make the
    // write fail without making the settings unreachable as well.
    let _ = std::fs::create_dir_all(record_of(&context));

    let report = household(&context, None).await.unwrap_or_default();

    assert!(
        report
            .findings
            .iter()
            .any(|said| said.contains("could not be written down")),
        "{report:?}"
    );
}

/// A member the request service no longer holds is forgotten rather than carried.
///
/// An account that has gone is nothing to give anything back to, and a record that
/// kept the line would carry somebody who left for as long as the household did.
#[tokio::test]
async fn a_member_the_service_no_longer_holds_is_forgotten() {
    let transport = asking_household((404, r#"{"message":"User not found."}"#)).transport();
    let context = ctx_over(Arc::clone(&transport), "gone", false);
    let _ = std::fs::write(record_of(&context), r#"{"5":32}"#);

    let report = household(&context, None).await.unwrap_or_default();

    assert!(report.available, "{report:?}");
    assert!(
        permissions_written(&transport).is_empty(),
        "an account that is gone was written to"
    );
    let kept = std::fs::read_to_string(record_of(&context)).unwrap_or_default();
    assert_eq!(kept, "{}", "somebody who left stayed written down: {kept}");
}

/// Nothing is taken from a member the request service no longer holds either.
#[tokio::test]
async fn nothing_is_taken_from_a_member_who_is_gone() {
    let (context, transport) = no_room_with(
        &asking_household((404, r#"{"message":"User not found."}"#)),
        "gone-full",
    );

    let report = household(&context, None).await.unwrap_or_default();

    assert!(report.available, "{report:?}");
    assert!(
        permissions_written(&transport).is_empty(),
        "an account that is gone was written to"
    );
    assert!(
        !record_of(&context).exists(),
        "an account that is gone was written down as held back"
    );
}

/// A rehearsal takes nothing away from anybody, however full the disk is.
#[tokio::test]
async fn a_rehearsal_takes_nothing_away() {
    let (context, transport) = no_room_with(
        &asking_household((200, r#"{"permissions":32}"#)),
        "rehearsed",
    );

    let report = household(&context.rehearsing(), None)
        .await
        .unwrap_or_default();

    assert!(report.available, "{report:?}");
    assert!(
        permissions_written(&transport).is_empty(),
        "a rehearsal stopped a household asking"
    );
}
