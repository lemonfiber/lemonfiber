use std::net::IpAddr;
use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake as Transport};
use lemonfiber_fixtures::ports::Resolving;

use super::{beneath_the_catalogue, poster};
use crate::app::Ctx;
use crate::config::{Reaching, REACH_POSTERS_KEY};
use crate::ports::http::{Method, Request};
use crate::ports::service::PICTURE_MOST;
use crate::screening::Pictured;
use crate::wishing::tests::{asked_for, ctx_over, ctx_running, house, route, Route};

/// A film out in Britain, rated twelve, with a poster.
const FILM: &str = r#"{"status":"Released","posterPath":"/film.jpg","releases":{"results":[
    {"iso_3166_1":"GB","release_dates":[{"certification":"12A"}]}]}}"#;

/// A film rated fifteen in Britain, with a poster.
const FILM_FIFTEEN: &str = r#"{"status":"Released","posterPath":"/fifteen.jpg","releases":{"results":[
    {"iso_3166_1":"GB","release_dates":[{"certification":"15"}]}]}}"#;

/// A film whose poster path climbs out from beneath the catalogue's address.
const FILM_CLIMBING: &str = r#"{"status":"Released","posterPath":"/../../../elsewhere.jpg"}"#;

/// A film with no poster.
const FILM_BARE: &str = r#"{"status":"Released"}"#;

/// Where the film's poster is published.
const POSTER_AT: &str = "https://image.tmdb.org/t/p/w342/film.jpg";

/// What the film's poster is.
const JPEG: &str = "a picture's bytes";

/// The house, with the films looked up and the poster answered as `served`.
fn pictured(served: Answer) -> Arc<Transport> {
    let picture: Route = (Method::Get, "image.tmdb.org", vec![served]);
    house(vec![
        picture,
        route(Method::Get, "/api/v1/movie/603", 200, FILM),
        route(Method::Get, "/api/v1/movie/604", 200, FILM_FIFTEEN),
        route(Method::Get, "/api/v1/movie/605", 200, FILM_BARE),
        route(Method::Get, "/api/v1/movie/606", 200, FILM_CLIMBING),
    ])
}

/// A house over `transport` whose every name stands for an address out on the internet.
fn ctx_outward(transport: &Arc<Transport>, tag: &str) -> Ctx {
    ctx_resolving(transport, tag, Resolving::anywhere())
}

/// A house over `transport` whose names are resolved by `resolving`.
fn ctx_resolving(transport: &Arc<Transport>, tag: &str, resolving: Arc<Resolving>) -> Ctx {
    ctx_over(Arc::clone(transport), tag).with_resolver(resolving)
}

/// The code a poster was refused with, or nothing where it was handed over.
fn code(outcome: &Result<Pictured, Box<crate::error::Problem>>) -> Option<String> {
    outcome
        .as_ref()
        .err()
        .map(|problem| problem.code.to_string())
}

#[tokio::test]
async fn a_member_is_handed_the_poster_fetched_from_the_catalogue_with_nothing_of_theirs() {
    let transport = pictured(Answer::served(200, "image/jpeg", JPEG));
    let ctx = ctx_outward(&transport, "poster-handed");

    let handed = poster(&ctx, "Bea", "movies", "603").await;

    assert_eq!(
        handed.ok(),
        Some(Pictured {
            media_type: "image/jpeg",
            bytes: JPEG.as_bytes().to_vec(),
        })
    );
    let fetched: Vec<_> = transport
        .requests()
        .into_iter()
        .filter(|asked| asked.url.contains("image.tmdb.org"))
        .collect();
    assert_eq!(
        fetched,
        vec![Request {
            method: Method::Get,
            url: POSTER_AT.to_owned(),
            headers: Vec::new(),
            body: None,
            pinned: Some(vec![lemonfiber_fixtures::ports::ANYWHERE]),
        }]
    );
}

#[tokio::test]
async fn a_catalogue_host_standing_for_somewhere_inward_is_never_fetched() {
    for (inward, tag) in [
        (IpAddr::from([127, 0, 0, 1]), "poster-loopback"),
        (IpAddr::from([10, 0, 0, 7]), "poster-private"),
        (IpAddr::from([169, 254, 169, 254]), "poster-link-local"),
    ] {
        let transport = pictured(Answer::served(200, "image/jpeg", JPEG));
        let ctx = ctx_resolving(&transport, tag, Resolving::standing_for(&[inward]));

        let refused = poster(&ctx, "Ada", "movies", "603").await;

        assert_eq!(code(&refused).as_deref(), Some("WISH-8"), "{inward}");
        assert!(!asked_for(&transport, "image.tmdb.org"), "{inward}");
    }
}

#[tokio::test]
async fn a_catalogue_host_that_cannot_be_resolved_is_never_fetched() {
    let transport = pictured(Answer::served(200, "image/jpeg", JPEG));
    let ctx = ctx_resolving(
        &transport,
        "poster-unresolved",
        Resolving::failing("no answer"),
    );

    let refused = poster(&ctx, "Ada", "movies", "603").await;

    assert_eq!(code(&refused).as_deref(), Some("WISH-8"));
    assert!(!asked_for(&transport, "image.tmdb.org"));
}

#[tokio::test]
async fn posters_switched_off_fetch_nothing_and_show_no_picture() {
    let transport = pictured(Answer::served(200, "image/jpeg", JPEG));
    let mut ctx = ctx_outward(&transport, "poster-switched-off");
    ctx.settings.reaching = Reaching::without(REACH_POSTERS_KEY);

    let refused = poster(&ctx, "Ada", "movies", "603").await;

    assert_eq!(code(&refused).as_deref(), Some("WISH-13"));
    assert!(!asked_for(&transport, "image.tmdb.org"));
}

#[tokio::test]
async fn a_title_outside_the_members_limits_is_no_title_and_its_poster_is_never_fetched() {
    let transport = pictured(Answer::served(200, "image/jpeg", JPEG));
    let ctx = ctx_outward(&transport, "poster-restricted");

    let refused = poster(&ctx, "Bea", "movies", "604").await;

    assert_eq!(code(&refused).as_deref(), Some("WISH-3"));
    assert!(!asked_for(&transport, "image.tmdb.org"));
}

#[tokio::test]
async fn a_kind_that_names_no_kind_is_no_title() {
    let transport = pictured(Answer::served(200, "image/jpeg", JPEG));
    let ctx = ctx_outward(&transport, "poster-no-kind");

    let refused = poster(&ctx, "Ada", "music", "603").await;

    assert_eq!(code(&refused).as_deref(), Some("WISH-3"));
    assert!(!asked_for(&transport, "/api/v1/movie/"));
}

#[tokio::test]
async fn a_kind_no_running_curator_fetches_is_refused_before_the_title_is_looked_up() {
    let transport = pictured(Answer::served(200, "image/jpeg", JPEG));
    let ctx = ctx_running(Arc::clone(&transport), "poster-no-curator", &["sonarr"])
        .with_resolver(Resolving::anywhere());

    let refused = poster(&ctx, "Ada", "movies", "603").await;

    assert_eq!(code(&refused).as_deref(), Some("WISH-4"));
    assert!(!asked_for(&transport, "/api/v1/movie/"));
}

#[tokio::test]
async fn nothing_that_is_a_small_raster_picture_is_shown() {
    let too_big = "x".repeat(PICTURE_MOST + 1);
    for (served, why) in [
        (Answer::served(200, "text/html", JPEG), "a page"),
        (
            Answer::served(200, "image/svg+xml", JPEG),
            "a drawing that can run",
        ),
        (Answer::served(200, "image/jpeg", too_big), "too large"),
        (Answer::served(404, "image/jpeg", JPEG), "absent"),
        (Answer::served(302, "image/jpeg", JPEG), "sent elsewhere"),
    ] {
        let transport = pictured(served);
        let ctx = ctx_outward(&transport, "poster-not-shown");

        let refused = poster(&ctx, "Ada", "movies", "603").await;

        assert_eq!(code(&refused).as_deref(), Some("WISH-13"), "{why}");
    }
}

#[tokio::test]
async fn a_poster_at_exactly_the_most_a_picture_may_be_is_shown() {
    let largest = "x".repeat(PICTURE_MOST);
    let transport = pictured(Answer::served(200, "image/png", largest));
    let ctx = ctx_outward(&transport, "poster-largest");

    let handed = poster(&ctx, "Ada", "movies", "603").await;

    assert_eq!(
        handed.map(|shown| shown.bytes.len()).ok(),
        Some(PICTURE_MOST)
    );
}

#[tokio::test]
async fn a_title_with_no_poster_shows_none_and_nothing_is_fetched() {
    let transport = pictured(Answer::served(200, "image/jpeg", JPEG));
    let ctx = ctx_outward(&transport, "poster-bare");

    let refused = poster(&ctx, "Ada", "movies", "605").await;

    assert_eq!(code(&refused).as_deref(), Some("WISH-13"));
    assert!(!asked_for(&transport, "image.tmdb.org"));
}

#[tokio::test]
async fn a_location_that_climbs_out_from_beneath_the_catalogue_is_never_fetched() {
    let transport = pictured(Answer::served(200, "image/jpeg", JPEG));
    let ctx = ctx_outward(&transport, "poster-climbing");

    let refused = poster(&ctx, "Ada", "movies", "606").await;

    assert_eq!(code(&refused).as_deref(), Some("WISH-8"));
    assert!(!asked_for(&transport, "image.tmdb.org"));
}

#[tokio::test]
async fn a_catalogue_that_does_not_answer_is_asking_not_working() {
    let transport = pictured(Answer::Silent);
    let ctx = ctx_outward(&transport, "poster-silent");

    let refused = poster(&ctx, "Ada", "movies", "603").await;

    assert_eq!(code(&refused).as_deref(), Some("WISH-8"));
}

#[test]
fn only_a_location_beneath_the_catalogue_is_fetched() {
    assert_eq!(
        beneath_the_catalogue(POSTER_AT).map(|request| request.url),
        Some(POSTER_AT.to_owned())
    );
    for elsewhere in [
        "http://image.tmdb.org/t/p/w342/film.jpg",
        "http://image.tmdb.org:443/t/p/w342/film.jpg",
        "https://images.example/t/p/w342/film.jpg",
        "https://image.tmdb.org.example/t/p/w342/film.jpg",
        "https://image.tmdb.org:8443/t/p/w342/film.jpg",
        "https://someone@image.tmdb.org/t/p/w342/film.jpg",
        "https://someone:secret@image.tmdb.org/t/p/w342/film.jpg",
        "https://:secret@image.tmdb.org/t/p/w342/film.jpg",
        "https://image.tmdb.org/t/p/original/film.jpg",
        "https://image.tmdb.org/t/p/w3420/film.jpg",
        "https://image.tmdb.org/t/p/w342",
        "https://127.0.0.1/t/p/w342/film.jpg",
        "/t/p/w342/film.jpg",
        "not an address",
    ] {
        assert!(beneath_the_catalogue(elsewhere).is_none(), "{elsewhere}");
    }
}
