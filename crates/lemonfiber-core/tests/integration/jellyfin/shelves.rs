//! What one member can watch.

use super::{reader, A_SHELF, SIGNED_IN};
use lemonfiber_core::ports::service::{Failure, Medium};
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::service::Household;

/// **The design claim, asserted rather than described.** The account whose shelf this
/// is appears in the path, not in a filter applied afterwards — which is what makes the
/// media server the thing that applies the age limit, the blocked kinds and the library
/// access, and lemonfiber the thing that holds no second copy of any of them.
#[tokio::test]
async fn a_shelf_is_asked_for_against_the_members_own_account() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, A_SHELF),
    ]);
    let held = reader(&fake).holdings("a7f3", 25).await;
    assert!(
        held.is_ok(),
        "a shelf the server answered was not read: {held:?}"
    );

    let asked = fake.requests();
    let url = asked
        .last()
        .map(|request| request.url.clone())
        .unwrap_or_default();
    assert!(
        url.contains("/Users/a7f3/Items"),
        "the shelf was not asked for as the member: {url}"
    );
    assert!(
        url.contains("Limit=25"),
        "the count asked for did not reach the server: {url}"
    );
    assert!(
        url.contains("IncludeItemTypes=Series,Movie"),
        "the shelf asked for something other than the two kinds: {url}"
    );
    // The query is written across two source lines, and a continuation that stopped
    // joining them would put a run of spaces inside it — which every assertion above
    // survives, because each looks at one segment.
    assert!(
        !url.contains(char::is_whitespace),
        "the query carries whitespace the server will not read: {url}"
    );
}

/// The two kinds arrive as this product's own words, and a third arrives rather than
/// vanishing: the query named two, so a third is the server describing something in a
/// way this build has not been taught — and leaving it out would be this deciding what
/// a household owns.
#[tokio::test]
async fn what_the_server_calls_things_arrives_in_this_products_words() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, A_SHELF),
    ]);
    let held = reader(&fake).holdings("a7f3", 25).await.unwrap_or_default();

    assert_eq!(
        held.iter().map(|one| one.medium).collect::<Vec<_>>(),
        vec![Medium::Film, Medium::Series, Medium::Other],
        "a kind was renamed or dropped on the way in"
    );
    assert_eq!(
        held.first().map(|one| (one.title.clone(), one.year)),
        Some(("A Film".to_owned(), Some(1994)))
    );
    // Absent rather than nought, because a year of nought is a year and this is the
    // absence of one — which is exactly what two films sharing a title turn on.
    assert_eq!(
        held.get(1).map(|one| one.year),
        Some(None),
        "a missing year was invented"
    );
}

#[tokio::test]
async fn a_shelf_the_server_would_not_describe_is_refused_rather_than_read_as_empty() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, "not json"),
    ]);
    assert!(matches!(
        reader(&fake).holdings("a7f3", 25).await,
        Err(Failure::Refused { .. })
    ));
}

/// A server answering a shelf with nothing on it is answering, and the empty list is
/// that answer. What it is not is the failure above, and the two reach the household
/// read as different things on purpose.
#[tokio::test]
async fn a_shelf_with_nothing_on_it_is_an_answer() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, "{}"),
    ]);
    assert_eq!(
        reader(&fake).holdings("a7f3", 25).await.ok(),
        Some(Vec::new())
    );
}
