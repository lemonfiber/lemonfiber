//! Finding a title in the library.

use crate::{reader, SIGNED_IN};
use lemonfiber_core::ports::service::Failure;
use lemonfiber_core::recyclarr::Kind;
use lemonfiber_fixtures::http::{Answer, Fake};

#[tokio::test]
async fn a_present_title_signs_in_then_finds_it_in_the_library() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, r#"{"Items":[{"Name":"The Expanse"}]}"#),
    ]);
    // The term matches the library title the same case-insensitive way the *arr found it.
    assert_eq!(
        reader(&fake).has_item(Kind::Sonarr, "expanse").await.ok(),
        Some(true)
    );

    let requests = fake.requests();
    // The sign-in is first: a POST to AuthenticateByName, identifying the client and
    // carrying the household admin credential in the body.
    assert!(requests.first().is_some_and(|request| {
        request.url.ends_with("/Users/AuthenticateByName")
            && request
                .headers
                .iter()
                .any(|(name, value)| name == "X-Emby-Authorization" && value.contains("lemonfiber"))
            && request.body.as_deref().is_some_and(|body| {
                // The admin name identifies the sign-in; the password is carried under
                // `Pw` (its value built from a range, not asserted as a literal here).
                body.contains(r#""Username":"admin""#) && body.contains(r#""Pw":""#)
            })
    }));
    // Then the library read, narrowed to series and carrying the minted token.
    assert!(requests.get(1).is_some_and(|request| {
        request.url.contains("IncludeItemTypes=Series")
            && request
                .headers
                .iter()
                .any(|(name, value)| name == "X-Emby-Token" && value == "token")
    }));
}

#[tokio::test]
async fn a_library_without_the_title_reads_as_absent() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, r#"{"Items":[{"Name":"Some Other Show"}]}"#),
    ]);
    assert_eq!(
        reader(&fake).has_item(Kind::Sonarr, "expanse").await.ok(),
        Some(false)
    );
}

#[tokio::test]
async fn a_film_read_asks_the_library_for_movies() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, r#"{"Items":[{"Name":"Dune"}]}"#),
    ]);
    assert_eq!(
        reader(&fake).has_item(Kind::Radarr, "dune").await.ok(),
        Some(true)
    );
    assert!(fake
        .requests()
        .get(1)
        .is_some_and(|read| read.url.contains("IncludeItemTypes=Movie")));
}

#[tokio::test]
async fn a_refused_sign_in_fails_before_any_library_read() {
    let fake = Fake::in_turn(vec![Answer::reply(401, "")]);
    assert!(matches!(
        reader(&fake).has_item(Kind::Sonarr, "expanse").await,
        Err(Failure::Unauthorised { .. })
    ));
    // The library was never read: without a token there is nothing to read it with.
    assert_eq!(fake.requests().len(), 1);
}

#[tokio::test]
async fn an_unreadable_library_is_refused() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, "not json"),
    ]);
    assert!(matches!(
        reader(&fake).has_item(Kind::Sonarr, "expanse").await,
        Err(Failure::Refused { .. })
    ));
}

#[tokio::test]
async fn an_unreachable_media_server_is_unavailable_for_a_library_read() {
    let fake = Fake::silent();
    assert!(matches!(
        reader(&fake).has_item(Kind::Sonarr, "expanse").await,
        Err(Failure::Unavailable { .. })
    ));
}
