use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake};

use crate::ports::http::Method;
use crate::ports::media::Kind;
use crate::ports::service::{Asked, MediaStatus, Searching, Wish};
use crate::seerr::Seerr;

/// The request service, carrying its own key, answering over `http`.
fn service(http: &Arc<Fake>) -> Seerr {
    Seerr::keyed(http.clone(), "http://seerr:5055", "seerr", "owner-key")
}

/// A page of a search: a series, a person and a film, as the service answers them.
const PAGE: &str = r#"{"page":1,"totalPages":3,"totalResults":42,"results":[
    {"id":227484,"mediaType":"tv","name":"East Harbour Heroes","firstAirDate":"2023-05-29",
     "posterPath":"/series.jpg","mediaInfo":{"status":4}},
    {"id":35029,"mediaType":"person","name":"David Harbour"},
    {"id":603,"mediaType":"movie","title":"The Matrix","releaseDate":"1999-03-31"}]}"#;

#[tokio::test]
async fn a_search_finds_films_and_series_of_the_kinds_asked_and_says_where_the_next_page_is() {
    let http = Fake::by_path(vec![("/api/v1/search", Answer::reply(200, PAGE))]);

    let page = service(&http)
        .search("harbour & co", &Kind::ALL, 1)
        .await
        .unwrap_or_default();

    assert_eq!(page.next, Some(2));
    assert_eq!(page.titles.len(), 2);
    assert_eq!(
        page.titles.first().map(|series| (
            series.id.as_str(),
            series.kind,
            series.title.as_str(),
            series.year,
            series.status,
            series.poster.as_deref()
        )),
        Some((
            "227484",
            Kind::Tv,
            "East Harbour Heroes",
            Some(2023),
            MediaStatus::PartlyAvailable,
            Some("https://image.tmdb.org/t/p/w342/series.jpg")
        ))
    );
    assert_eq!(
        page.titles
            .get(1)
            .map(|film| (film.kind, film.status, film.poster.as_deref())),
        Some((Kind::Movies, MediaStatus::Unknown, None))
    );
    assert!(http.requests().iter().any(|asked| asked
        .url
        .ends_with("/api/v1/search?query=harbour%20%26%20co&page=1")));
}

#[tokio::test]
async fn a_search_for_one_kind_leaves_the_other_out_and_the_last_page_has_no_next() {
    let last = PAGE.replace(r#""page":1"#, r#""page":3"#);
    let http = Fake::by_path(vec![("/api/v1/search", Answer::reply(200, last))]);

    let page = service(&http)
        .search("harbour", &[Kind::Movies], 3)
        .await
        .unwrap_or_default();

    assert_eq!(page.next, None);
    assert_eq!(
        page.titles.iter().map(|one| one.kind).collect::<Vec<_>>(),
        vec![Kind::Movies]
    );
}

#[tokio::test]
async fn a_films_certification_is_its_first_one_in_the_region_asked() {
    let film = r#"{"overview":"Set in the 22nd century.","status":"Released","releases":{"results":[
        {"iso_3166_1":"NL","release_dates":[{"certification":""},{"certification":"12"}]},
        {"iso_3166_1":"US","release_dates":[{"certification":"R"}]}]}}"#;
    let http = Fake::by_path(vec![("/api/v1/movie/603", Answer::reply(200, film))]);

    let detail = service(&http)
        .detail(Kind::Movies, "603", "NL")
        .await
        .ok()
        .flatten()
        .unwrap_or_default();

    assert_eq!(detail.certification.as_deref(), Some("12"));
    assert!(detail.released);
    assert!(detail.seasons.is_empty());
    assert_eq!(detail.overview.as_deref(), Some("Set in the 22nd century."));
}

#[tokio::test]
async fn a_series_is_rated_by_region_and_never_offers_its_specials() {
    let series = r#"{"status":"Ended","firstAirDate":"2011-04-17",
        "contentRatings":{"results":[{"iso_3166_1":"US","rating":"TV-MA"},{"iso_3166_1":"NL","rating":"16"}]},
        "seasons":[{"seasonNumber":0},{"seasonNumber":1},{"seasonNumber":2}],
        "mediaInfo":{"status":4,"seasons":[{"seasonNumber":1,"status":5}]}}"#;
    let http = Fake::by_path(vec![("/api/v1/tv/1399", Answer::reply(200, series))]);

    let detail = service(&http)
        .detail(Kind::Tv, "1399", "us")
        .await
        .ok()
        .flatten()
        .unwrap_or_default();

    assert_eq!(detail.certification.as_deref(), Some("TV-MA"));
    assert!(detail.released);
    assert_eq!(
        detail
            .seasons
            .iter()
            .map(|season| (season.number, season.status))
            .collect::<Vec<_>>(),
        vec![(1, MediaStatus::Available), (2, MediaStatus::Unknown)]
    );
    assert_eq!(detail.overview, None);
}

#[tokio::test]
async fn a_title_with_no_certification_in_the_region_has_none_and_one_not_out_is_unreleased() {
    let planned = r#"{"status":"Planned","firstAirDate":"2030-01-01",
        "contentRatings":{"results":[{"iso_3166_1":"US","rating":""}]},"seasons":[]}"#;
    let http = Fake::by_path(vec![("/api/v1/tv/9", Answer::reply(200, planned))]);

    let detail = service(&http)
        .detail(Kind::Tv, "9", "NL")
        .await
        .ok()
        .flatten()
        .unwrap_or_default();

    assert_eq!(detail.certification, None);
    assert!(!detail.released);
}

#[tokio::test]
async fn a_title_the_service_does_not_know_is_nothing() {
    let http = Fake::by_path(vec![("/api/v1/movie/1", Answer::reply(404, "{}"))]);

    let detail = service(&http).detail(Kind::Movies, "1", "NL").await;

    assert_eq!(detail.ok(), Some(None));
}

#[tokio::test]
async fn an_id_that_is_not_the_services_number_is_nothing_and_nothing_is_asked() {
    let http = Fake::by_path(vec![("/api/v1/", Answer::reply(200, "{}"))]);
    let seerr = service(&http);

    for id in ["..", "603/../../settings", "-603", ""] {
        assert_eq!(seerr.detail(Kind::Movies, id, "NL").await.ok(), Some(None));
    }
    assert!(http.requests().is_empty());
}

#[tokio::test]
async fn an_ask_is_filed_for_the_member_with_every_season_or_the_ones_named() {
    let http = Fake::by_route_in_turn(vec![(
        Method::Post,
        "/api/v1/request",
        vec![
            Answer::reply(201, r#"{"id":7,"status":1}"#),
            Answer::reply(201, r#"{"id":8,"status":2}"#),
        ],
    )]);
    let seerr = service(&http);
    let wish = |seasons: Vec<u32>| Wish {
        id: "1399".to_owned(),
        kind: Kind::Tv,
        seasons,
    };

    let every = seerr.ask("4", &wish(Vec::new())).await;
    let named = seerr.ask("4", &wish(vec![2, 3])).await;

    assert_eq!(
        every.ok(),
        Some(Asked {
            request: 7,
            waiting: true
        })
    );
    assert_eq!(
        named.ok(),
        Some(Asked {
            request: 8,
            waiting: false
        })
    );
    let bodies: Vec<serde_json::Value> = http
        .requests()
        .iter()
        .filter_map(|asked| asked.body.as_deref())
        .filter_map(|body| serde_json::from_str(body).ok())
        .collect();
    assert_eq!(
        bodies,
        vec![
            serde_json::json!({"mediaType":"tv","mediaId":1399,"userId":4,"seasons":"all"}),
            serde_json::json!({"mediaType":"tv","mediaId":1399,"userId":4,"seasons":[2,3]}),
        ]
    );
}

#[tokio::test]
async fn a_film_is_asked_for_without_seasons_and_an_unnamed_member_or_title_is_refused_unasked() {
    let http = Fake::by_route(vec![(
        Method::Post,
        "/api/v1/request",
        Answer::reply(201, r#"{"id":9,"status":2}"#),
    )]);
    let seerr = service(&http);
    let film = Wish {
        id: "603".to_owned(),
        kind: Kind::Movies,
        seasons: vec![1],
    };

    assert!(seerr.ask("somebody", &film).await.is_err());
    assert!(seerr.ask("-4", &film).await.is_err());
    let unnamed = Wish {
        id: "603&userId=1".to_owned(),
        ..film.clone()
    };
    assert!(seerr.ask("4", &unnamed).await.is_err());
    assert!(http.requests().is_empty());

    let asked = seerr.ask("4", &film).await;
    assert_eq!(
        asked.ok(),
        Some(Asked {
            request: 9,
            waiting: false
        })
    );
    assert_eq!(
        http.requests()
            .first()
            .and_then(|asked| asked.body.as_deref())
            .and_then(|body| serde_json::from_str::<serde_json::Value>(body).ok()),
        Some(serde_json::json!({"mediaType":"movie","mediaId":603,"userId":4}))
    );
}
