use std::sync::Arc;

use super::stands;
use crate::app::Outcome;
use crate::model::{AskableReport, Stands, TitleKind};
use crate::ports::http::Method;
use crate::ports::service::MediaStatus;
use crate::wishing::place::Place;
use crate::wishing::tests::{
    asked_for, code, ctx_over, ctx_running, found, house, route, Route, APPROVES_OWN,
};

/// A first page of three: a film here, a series nobody has asked for, and a person, on a
/// search with a second page.
const FIRST: &str = r#"{"page":1,"totalPages":2,"results":[
    {"id":603,"mediaType":"movie","title":"The Matrix","releaseDate":"1999-03-31","mediaInfo":{"status":5}},
    {"id":1399,"mediaType":"tv","name":"Harbour Lights","firstAirDate":"2011-04-17"},
    {"id":7,"mediaType":"person","name":"Somebody"}]}"#;

/// A second and last page: the film again, and one more film on its way.
const SECOND: &str = r#"{"page":2,"totalPages":2,"results":[
    {"id":603,"mediaType":"movie","title":"The Matrix","releaseDate":"1999-03-31","mediaInfo":{"status":5}},
    {"id":604,"mediaType":"movie","title":"Harbour","releaseDate":"2003-05-15","mediaInfo":{"status":3}}]}"#;

/// The film, rated fifteen in Britain.
const FIFTEEN: &str = r#"{"status":"Released","releases":{"results":[
    {"iso_3166_1":"GB","release_dates":[{"certification":"15"}]}]},"mediaInfo":{"status":5}}"#;

/// The series, rated 12A in Britain.
const TWELVE: &str = r#"{"status":"Ended","firstAirDate":"2011-04-17",
    "contentRatings":{"results":[{"iso_3166_1":"GB","rating":"12A"}]},"seasons":[{"seasonNumber":1}]}"#;

/// The series, rated fifteen in Britain.
const SERIES_FIFTEEN: &str = r#"{"status":"Ended","firstAirDate":"2011-04-17",
    "contentRatings":{"results":[{"iso_3166_1":"GB","rating":"15"}]}}"#;

/// A film rated 12A in Britain.
const FILM_TWELVE: &str = r#"{"status":"Released","releases":{"results":[
    {"iso_3166_1":"GB","release_dates":[{"certification":"12A"}]}]}}"#;

/// A film nobody has rated anywhere.
const UNRATED: &str = r#"{"status":"Released","releases":{"results":[]}}"#;

/// The search's two pages.
fn pages() -> Vec<Route> {
    vec![
        route(
            Method::Get,
            "/api/v1/search?query=harbour&page=1",
            200,
            FIRST,
        ),
        route(
            Method::Get,
            "/api/v1/search?query=harbour&page=2",
            200,
            SECOND,
        ),
    ]
}

/// The report a search answered with, or nothing where it was refused.
fn report(outcome: Result<Outcome, Box<crate::error::Problem>>) -> Option<AskableReport> {
    match outcome {
        Ok(Outcome::Askable(report)) => Some(report),
        _ => None,
    }
}

/// The ids on a page, in order.
fn ids(report: Option<&AskableReport>) -> Vec<String> {
    report
        .map(|report| report.titles.iter().map(|title| title.id.clone()).collect())
        .unwrap_or_default()
}

#[tokio::test]
async fn a_member_nothing_holds_back_is_shown_every_title_of_a_kind_the_house_fetches() {
    let transport = house(pages());
    let ctx = ctx_over(Arc::clone(&transport), "find-open");

    let page = report(found(&ctx, "Ada", "  harbour ", None).await);

    assert_eq!(ids(page.as_ref()), vec!["603", "1399"]);
    let shown: Vec<(TitleKind, Stands, Option<u16>)> = page
        .as_ref()
        .map(|page| {
            page.titles
                .iter()
                .map(|title| (title.kind, title.stands, title.year))
                .collect()
        })
        .unwrap_or_default();
    assert_eq!(
        shown,
        vec![
            (TitleKind::Movies, Stands::Here, Some(1999)),
            (TitleKind::Tv, Stands::Askable, Some(2011)),
        ]
    );
    assert_eq!(
        page.as_ref()
            .map(|page| (page.member.as_str(), page.term.as_str())),
        Some(("Ada", "harbour"))
    );
    assert_eq!(
        page.as_ref().map(|page| page.kinds.clone()),
        Some(vec![TitleKind::Tv, TitleKind::Movies])
    );
    assert_eq!(
        page.as_ref().map(|page| page.approval.as_str()),
        Some("Nothing arrives until whoever runs the house has said yes to it.")
    );
    assert_eq!(
        page.as_ref()
            .and_then(|page| page.next.as_deref())
            .and_then(|next| Place::read(Some(next)).ok()),
        Some(Place {
            page: 2,
            shown: vec!["603".to_owned(), "1399".to_owned()],
        })
    );
    assert!(!asked_for(&transport, "/api/v1/movie/"));
    assert!(!asked_for(&transport, "/api/v1/tv/"));
}

#[tokio::test]
async fn the_next_page_shows_no_title_an_earlier_page_did_and_is_the_last() {
    let transport = house(pages());
    let ctx = ctx_over(Arc::clone(&transport), "find-next");
    let first = report(found(&ctx, "Ada", "harbour", None).await);
    let after = first.and_then(|first| first.next);

    let second = report(found(&ctx, "Ada", "harbour", after.as_deref()).await);

    assert_eq!(ids(second.as_ref()), vec!["604"]);
    assert_eq!(
        second
            .as_ref()
            .and_then(|page| page.titles.first())
            .map(|title| title.stands),
        Some(Stands::OnItsWay)
    );
    assert_eq!(second.and_then(|page| page.next), None);
}

#[tokio::test]
async fn a_member_held_to_twelve_is_never_shown_what_is_older_or_unrated() {
    let mut routes = pages();
    routes.extend([
        route(Method::Get, "/api/v1/movie/603", 200, FIFTEEN),
        route(Method::Get, "/api/v1/tv/1399", 200, TWELVE),
    ]);
    let transport = house(routes);
    let ctx = ctx_over(Arc::clone(&transport), "find-limited");

    let page = report(found(&ctx, "Bea", "harbour", None).await);

    assert_eq!(ids(page.as_ref()), vec!["1399"]);
    assert!(asked_for(&transport, "/api/v1/movie/603"));
}

#[tokio::test]
async fn an_unrated_title_follows_the_members_own_unrated_policy() {
    let routes = || {
        let mut routes = pages();
        routes.extend([
            route(Method::Get, "/api/v1/movie/603", 200, UNRATED),
            route(Method::Get, "/api/v1/tv/1399", 200, TWELVE),
        ]);
        routes
    };
    let held = ctx_over(house(routes()), "find-unrated-held");
    let let_through = ctx_over(house(routes()), "find-unrated-let");

    let bea = report(found(&held, "Bea", "harbour", None).await);
    let cy = report(found(&let_through, "Cy", "harbour", None).await);

    assert_eq!(ids(bea.as_ref()), vec!["1399"]);
    assert_eq!(ids(cy.as_ref()), vec!["603", "1399"]);
}

#[tokio::test]
async fn a_title_that_cannot_be_looked_up_for_a_held_member_is_left_out() {
    let mut routes = pages();
    routes.extend([
        route(Method::Get, "/api/v1/movie/603", 500, ""),
        route(Method::Get, "/api/v1/tv/1399", 404, "{}"),
    ]);
    let transport = house(routes);
    let ctx = ctx_over(Arc::clone(&transport), "find-unlooked");

    let page = report(found(&ctx, "Cy", "harbour", None).await);

    assert_eq!(ids(page.as_ref()), Vec::<String>::new());
    assert!(asked_for(&transport, "page=2"), "an empty page reads on");
}

#[tokio::test]
async fn a_page_with_nothing_the_member_may_see_reads_on_to_the_next() {
    let mut routes = pages();
    routes.extend([
        route(Method::Get, "/api/v1/movie/603", 200, FIFTEEN),
        route(Method::Get, "/api/v1/tv/1399", 200, SERIES_FIFTEEN),
        route(Method::Get, "/api/v1/movie/604", 200, FILM_TWELVE),
    ]);
    let transport = house(routes);
    let ctx = ctx_over(Arc::clone(&transport), "find-reads-on");

    let page = report(found(&ctx, "Cy", "harbour", None).await);

    assert_eq!(ids(page.as_ref()), vec!["604"]);
    assert_eq!(page.and_then(|page| page.next), None);
}

#[tokio::test]
async fn only_kinds_a_running_curator_fetches_are_searched() {
    let transport = house(pages());
    let ctx = ctx_running(Arc::clone(&transport), "find-series-only", &["sonarr"]);

    let page = report(found(&ctx, "Ada", "harbour", None).await);

    assert_eq!(ids(page.as_ref()), vec!["1399"]);
    assert_eq!(page.map(|page| page.kinds), Some(vec![TitleKind::Tv]));
}

#[tokio::test]
async fn a_house_with_no_curator_running_cannot_fetch_anything() {
    let transport = house(pages());
    let ctx = ctx_running(Arc::clone(&transport), "find-nothing-runs", &["jellyfin"]);

    let refused = found(&ctx, "Ada", "harbour", None).await;

    assert_eq!(code(&refused).as_deref(), Some("WISH-4"));
    assert!(!asked_for(&transport, "/api/v1/search"));
}

#[tokio::test]
async fn too_few_or_too_many_letters_are_refused_before_anything_is_asked() {
    let transport = house(pages());
    let ctx = ctx_over(Arc::clone(&transport), "find-term");

    for term in ["", " h ", &"h".repeat(101)] {
        let refused = found(&ctx, "Ada", term, None).await;
        assert_eq!(code(&refused).as_deref(), Some("WISH-1"), "{term:.10}");
    }
    let refused = found(&ctx, "Ada", "harbour", Some("nowhere")).await;
    assert_eq!(code(&refused).as_deref(), Some("WISH-2"));
    assert!(transport.requests().is_empty());
}

#[tokio::test]
async fn a_request_service_that_will_not_search_is_asking_not_working() {
    let transport = house(vec![route(Method::Get, "/api/v1/search", 500, "")]);
    let ctx = ctx_over(Arc::clone(&transport), "find-silent");

    let refused = found(&ctx, "Ada", "harbour", None).await;

    assert_eq!(code(&refused).as_deref(), Some("WISH-8"));
}

#[tokio::test]
async fn a_request_service_that_will_not_say_what_the_member_may_ask_for_is_not_working() {
    let mut routes = pages();
    routes.push(route(Method::Get, "/quota", 500, ""));
    let transport = house(routes);
    let ctx = ctx_over(Arc::clone(&transport), "find-allowance-unread");

    let refused = found(&ctx, "Ada", "harbour", None).await;

    assert_eq!(code(&refused).as_deref(), Some("WISH-8"));
    assert!(!asked_for(&transport, "/api/v1/search"));
}

#[tokio::test]
async fn the_page_says_in_the_households_words_what_the_member_may_ask_for() {
    let mut routes = pages();
    routes.push(route(
        Method::Get,
        "/api/v1/user/jellyfin/",
        200,
        APPROVES_OWN,
    ));
    let transport = house(routes);
    let ctx = ctx_over(Arc::clone(&transport), "find-trusted");

    let page = report(found(&ctx, "Ada", "harbour", None).await);

    assert_eq!(
        page.as_ref().map(|page| page.approval.as_str()),
        Some(
            "Everything you ask for arrives without anybody saying yes first, as long as your \
             allowance has room for it."
        )
    );
    assert_eq!(
        page.map(|page| (
            page.asking.films.remaining,
            page.asking.television.remaining
        )),
        Some((Some(1), Some(5)))
    );
}

#[test]
fn where_a_title_stands_is_said_in_four_words_the_household_reads() {
    assert_eq!(stands(MediaStatus::Available), Stands::Here);
    assert_eq!(stands(MediaStatus::PartlyAvailable), Stands::PartlyHere);
    assert_eq!(stands(MediaStatus::Pending), Stands::OnItsWay);
    assert_eq!(stands(MediaStatus::Processing), Stands::OnItsWay);
    assert_eq!(stands(MediaStatus::Unknown), Stands::Askable);
    assert_eq!(stands(MediaStatus::Deleted), Stands::Askable);
}
