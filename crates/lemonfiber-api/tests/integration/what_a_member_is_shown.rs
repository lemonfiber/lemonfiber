//! A title's pictures, as a member, the operator and the machine are shown them.
//!
//! Its own file because a picture answers to two rulings: the one a read of its title
//! is under, and which shelf the media server is asked for it from.

use std::fs;

use crate::door;
use crate::what_a_key_admits::sent;
use crate::what_a_member_may_ask_for::{as_a_member, signed_in, MEMBER, WHO};
use door::*;
use lemonfiber_core::ports::service::{Holds, Image, Item, ItemDetail, Medium};

/// A film on the member's shelf, by an id shaped like the media server's.
const FILM: &str = "0123456789abcdef0123456789abcdef";

/// What the media server holds as the film's every picture.
const A_PICTURE: &str = "a-picture";

/// The film, as the media server details it, holding both its pictures.
fn the_film() -> ItemDetail {
    ItemDetail {
        item: Item {
            id: FILM.to_owned(),
            title: "Heat".to_owned(),
            year: None,
            medium: Medium::Film,
            holds: Holds {
                poster: true,
                backdrop: true,
                ..Holds::default()
            },
        },
        overview: None,
        minutes: None,
        genres: Vec::new(),
        certificate: None,
        released: None,
        seasons: Vec::new(),
    }
}

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
        (media::identity("whoever"), recognised()),
        (media::identity("standing"), standing()),
        (
            media::serve("picture"),
            media::answered(&Some(Image {
                media_type: "image/png".to_owned(),
                bytes: A_PICTURE.as_bytes().to_vec(),
            })),
        ),
        (media::serve("title"), media::answered(&Some(the_film()))),
        (media::identity("household"), media::answered(&household())),
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

/// The picture `path` names, shown to whoever carries `secret`, and whose shelf the media
/// server was asked for it from.
async fn shown_to(
    router: &axum::Router,
    transport: &Fake,
    secret: &str,
    path: &str,
) -> Vec<serde_json::Value> {
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
    let sent: Vec<_> = transport.requests().into_iter().skip(before).collect();
    media::served(&sent, "picture")
        .into_iter()
        .filter_map(|asked| asked.get("member").cloned())
        .collect()
}

/// A picture is fetched from the shelf the title read is ruled to read: a member's own
/// whoever they named, the shelf of the member the operator named, and the household's
/// defaults where the machine asked for them.
#[tokio::test]
async fn a_picture_comes_from_the_shelf_the_title_read_is_ruled_to_read() {
    let (router, token, transport) = over_a_picture("pictured");
    let theirs = serde_json::json!(MEMBER);

    let member = signed_in(router.clone()).await;
    let asked_for = shown_to(
        &router,
        &transport,
        &member,
        &format!("/api/held/{FILM}/poster?member=bo"),
    )
    .await;
    assert_eq!(asked_for, std::slice::from_ref(&theirs));

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
    assert_eq!(asked_for, [theirs]);

    let asked_for = shown_to(
        &router,
        &transport,
        token.as_str(),
        &format!("/api/held/{FILM}/poster?defaults=true"),
    )
    .await;
    assert_eq!(asked_for, [serde_json::Value::Null]);

    let _ = fs::remove_dir_all(a_directory("pictured"));
    let _ = fs::remove_dir_all(a_directory("pictured-kept"));
}

/// The poster of a title a search found is the member's own, as the search is: it reaches
/// the core for them, and a parameter the read does not take is refused before it does.
#[tokio::test]
async fn a_member_reaches_a_found_titles_poster_as_they_reach_the_search() {
    let (router, carried) = as_a_member("member-found-poster").await;

    let answer = asked(
        router.clone(),
        "GET",
        "/api/askable/603/poster?kind=movies&member=bo",
        &carried,
        "",
    )
    .await;
    assert_ne!(answer.status, StatusCode::FORBIDDEN, "{}", answer.body);
    assert!(answer.body.contains("WISH-"), "{}", answer.body);

    let answer = asked(
        router.clone(),
        "GET",
        "/api/askable/603/poster?term=heat",
        &carried,
        "",
    )
    .await;
    assert_eq!(answer.status, StatusCode::BAD_REQUEST, "{}", answer.body);
    assert!(!answer.body.contains("WISH-"), "{}", answer.body);

    let _ = fs::remove_dir_all(a_directory("member-found-poster"));
}

/// The operator's own session names the member the poster is for, and naming nobody is
/// the member's search naming nobody.
#[tokio::test]
async fn a_found_titles_poster_for_nobody_is_refused_as_the_search_is() {
    let (router, _, _) = over_a_picture("found-poster-nobody");
    let operator = asked(
        router.clone(),
        "POST",
        SESSION,
        &from_here(),
        &offering(&chosen()),
    )
    .await;
    let (answer, _) = sent(
        router,
        "GET",
        "/api/askable/603/poster?kind=movies",
        &session(&operator.body),
        None,
        "",
    )
    .await;

    assert_eq!(answer.status, StatusCode::BAD_REQUEST, "{}", answer.body);
    assert!(answer.body.contains("WISH-11"), "{}", answer.body);

    let _ = fs::remove_dir_all(a_directory("found-poster-nobody"));
    let _ = fs::remove_dir_all(a_directory("found-poster-nobody-kept"));
}
