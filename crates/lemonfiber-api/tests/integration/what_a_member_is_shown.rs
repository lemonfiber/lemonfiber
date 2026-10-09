//! A title's pictures, as a member, the operator and the machine are shown them.
//!
//! Its own file because a picture answers to two rulings: the one a read of its title
//! is under, and which shelf the media server is asked for it from.

use std::fs;

use crate::door;
use crate::what_a_key_admits::sent;
use crate::what_a_member_may_ask_for::{as_a_member, signed_in, MEMBER, WHO};
use door::*;
use lemonfiber_fixtures::http::Answer as Reply;

/// A film on the member's shelf, by an id shaped like the media server's.
const FILM: &str = "0123456789abcdef0123456789abcdef";

/// What the media server holds as the film's every picture.
const A_PICTURE: &str = "a-picture";

/// A title's pictures are theirs as the title is, and are refused nothing a title read
/// would not be: a parameter no read takes, and a title named by anything but an id.
#[tokio::test]
async fn a_member_reaches_a_titles_pictures_as_they_reach_the_title() {
    let (router, carried) = as_a_member("member-pictures").await;
    for path in [
        format!("/api/held/{FILM}/poster"),
        format!("/api/held/{FILM}/backdrop?member=bo"),
    ] {
        let answer = asked(router.clone(), "GET", &path, &carried, "").await;
        assert_ne!(
            answer.status,
            StatusCode::FORBIDDEN,
            "{path}: {}",
            answer.body
        );
    }
    for path in [
        format!("/api/held/{FILM}/poster?most=3"),
        format!("/api/held/{FILM}/poster?defaults=perhaps"),
        "/api/held/not-an-item/backdrop".to_owned(),
    ] {
        let answer = asked(router.clone(), "GET", &path, &carried, "").await;
        assert_eq!(
            answer.status,
            StatusCode::BAD_REQUEST,
            "{path}: {}",
            answer.body
        );
    }
    let _ = fs::remove_dir_all(a_directory("member-pictures"));
}

/// A surface over the stack's own media server, which recognises the member and holds
/// the film on their shelf with its pictures, and the operator's password kept beside
/// it.
fn over_a_picture(named: &str) -> (axum::Router, Arc<Token>, Arc<Fake>) {
    let transport = Fake::by_path(vec![
        ("/Users/AuthenticateByName", Reply::reply(200, SIGNED_IN)),
        ("/Users/Me", Reply::reply(200, STANDING)),
        ("/Images/", Reply::served(200, "image/png", A_PICTURE)),
        (
            "/Items/",
            Reply::reply(
                200,
                format!(r#"{{"Id":"{FILM}","Name":"Heat","Type":"Movie","IsFolder":false}}"#),
            ),
        ),
        ("/Users", Reply::reply(200, HOUSEHOLD)),
    ]);
    let ctx = a_stack(named, Arc::clone(&transport));
    let kept = keeping(&format!("{named}-kept"));
    seeded(&ctx);
    let admitting = Arc::new(Admitting {
        kept: Some(kept),
        household: Some(Arc::new(ctx.clone()) as Arc<dyn HouseholdAtHand>),
        ..Admitting::default()
    });
    let (router, token) = surface(ctx, &admitting);
    (router, token, transport)
}

/// The picture `path` names, shown to whoever carries `secret`, and what the media
/// server was asked for it.
async fn shown_to(
    router: &axum::Router,
    transport: &Fake,
    secret: &str,
    path: &str,
) -> Vec<String> {
    let before = transport.requests().len();
    let (answer, headers) = sent(router.clone(), "GET", path, secret, None, "").await;
    assert_eq!(
        (answer.status, answer.body.as_str()),
        (StatusCode::OK, A_PICTURE),
        "{path}"
    );
    assert_eq!(
        headers
            .get(header::CACHE_CONTROL)
            .and_then(|value| value.to_str().ok()),
        Some("private"),
        "{path}"
    );
    transport
        .requests()
        .into_iter()
        .skip(before)
        .map(|request| request.url)
        .collect()
}

/// A picture is fetched from the shelf the title read is ruled to read: a member's own
/// whoever they named, the shelf of the member the operator named, and the household's
/// defaults where the machine asked for them.
#[tokio::test]
async fn a_picture_comes_from_the_shelf_the_title_read_is_ruled_to_read() {
    let (router, token, transport) = over_a_picture("pictured");
    let theirs = format!("/Items/{FILM}?userId={MEMBER}");

    let member = signed_in(router.clone()).await;
    let asked_for = shown_to(
        &router,
        &transport,
        &member,
        &format!("/api/held/{FILM}/poster?member=bo"),
    )
    .await;
    assert!(
        asked_for.iter().any(|url| url.ends_with(&theirs)),
        "{asked_for:?}"
    );
    assert!(
        !asked_for.iter().any(|url| url.contains("userId=bo")),
        "{asked_for:?}"
    );

    let operator = asked(
        router.clone(),
        "POST",
        SESSION,
        &from_here(),
        &offering(&chosen()),
    )
    .await;
    let asked_for = shown_to(
        &router,
        &transport,
        &session(&operator.body),
        &format!("/api/held/{FILM}/backdrop?member={WHO}"),
    )
    .await;
    assert!(
        asked_for.iter().any(|url| url.ends_with(&theirs)),
        "{asked_for:?}"
    );

    let asked_for = shown_to(
        &router,
        &transport,
        token.as_str(),
        &format!("/api/held/{FILM}/poster?defaults=true"),
    )
    .await;
    assert!(
        asked_for
            .iter()
            .any(|url| url.ends_with(&format!("/Items/{FILM}"))),
        "{asked_for:?}"
    );
    assert!(
        !asked_for.iter().any(|url| url.contains("userId")),
        "{asked_for:?}"
    );

    let _ = fs::remove_dir_all(a_directory("pictured"));
    let _ = fs::remove_dir_all(a_directory("pictured-kept"));
}
