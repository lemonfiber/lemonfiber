use std::sync::Arc;

use lemonfiber_fixtures::http::Answer;

use super::stands;
use crate::app::Outcome;
use crate::model::{AskableReport, Stands, TitleKind};
use crate::ports::media::Kind;
use crate::ports::service::{Detail, Found, MediaStatus, Page, Season};
use crate::test_support::requests::{answered, intake, unavailable};
use crate::wishing::place::Place;
use crate::wishing::tests::{
    about, account, asked_about, asked_for, call, code, ctx_over, ctx_running, found, house, rated,
    titled_as, Route,
};

/// A title the search finds.
fn finding(id: &str, kind: Kind, title: &str, year: u16, status: MediaStatus) -> Found {
    Found {
        id: id.to_owned(),
        kind,
        title: title.to_owned(),
        year: Some(year),
        status,
        poster: None,
    }
}

/// The film the search finds on both pages, already here.
fn the_matrix() -> Found {
    finding(
        "603",
        Kind::Movies,
        "The Matrix",
        1999,
        MediaStatus::Available,
    )
}

/// A first page of two, a film here and a series nobody has asked for, on a search with a
/// second page.
fn first() -> Page {
    Page {
        titles: vec![
            the_matrix(),
            finding(
                "1399",
                Kind::Tv,
                "Harbour Lights",
                2011,
                MediaStatus::Unknown,
            ),
        ],
        next: Some(2),
    }
}

/// A second and last page: the film again, and one more film on its way.
fn second() -> Page {
    Page {
        titles: vec![
            the_matrix(),
            finding(
                "604",
                Kind::Movies,
                "Harbour",
                2003,
                MediaStatus::Processing,
            ),
        ],
        next: None,
    }
}

/// The series, rated `certification` in Britain, its one season nobody has asked for.
fn series(certification: &str) -> Detail {
    Detail {
        seasons: vec![Season {
            number: 1,
            status: MediaStatus::Unknown,
        }],
        ..rated(Some(certification))
    }
}

/// The search's two pages.
fn pages() -> Vec<Route> {
    vec![
        about("search", r#""page":1"#, answered(&first())),
        about("search", r#""page":2"#, answered(&second())),
    ]
}

/// The search's two pages, the film and the series looked up as `film` and `series`.
fn looked_up(film: Answer, series: Answer) -> Vec<Route> {
    let mut routes = pages();
    routes.extend([titled_as("603", film), titled_as("1399", series)]);
    routes
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
    assert!(!asked_for(&transport, intake("detail")));
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
    let transport = house(looked_up(
        answered(&Some(rated(Some("15")))),
        answered(&Some(series("12A"))),
    ));
    let ctx = ctx_over(Arc::clone(&transport), "find-limited");

    let page = report(found(&ctx, "Bea", "harbour", None).await);

    assert_eq!(ids(page.as_ref()), vec!["1399"]);
    assert!(asked_about(&transport, "detail", r#""id":"603""#));
}

#[tokio::test]
async fn an_unrated_title_follows_the_members_own_unrated_policy() {
    let routes = || looked_up(answered(&Some(rated(None))), answered(&Some(series("12A"))));
    let held = ctx_over(house(routes()), "find-unrated-held");
    let let_through = ctx_over(house(routes()), "find-unrated-let");

    let bea = report(found(&held, "Bea", "harbour", None).await);
    let cy = report(found(&let_through, "Cy", "harbour", None).await);

    assert_eq!(ids(bea.as_ref()), vec!["1399"]);
    assert_eq!(ids(cy.as_ref()), vec!["603", "1399"]);
}

#[tokio::test]
async fn a_title_that_cannot_be_looked_up_for_a_held_member_is_left_out() {
    let transport = house(looked_up(unavailable(), answered(&None::<Detail>)));
    let ctx = ctx_over(Arc::clone(&transport), "find-unlooked");

    let page = report(found(&ctx, "Cy", "harbour", None).await);

    assert_eq!(ids(page.as_ref()), Vec::<String>::new());
    assert!(
        asked_about(&transport, "search", r#""page":2"#),
        "an empty page reads on"
    );
}

#[tokio::test]
async fn a_page_with_nothing_the_member_may_see_reads_on_to_the_next() {
    let mut routes = looked_up(
        answered(&Some(rated(Some("15")))),
        answered(&Some(series("15"))),
    );
    routes.push(titled_as("604", answered(&Some(rated(Some("12A"))))));
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
    assert!(!asked_for(&transport, intake("search")));
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
    let transport = house(vec![call("search", unavailable())]);
    let ctx = ctx_over(Arc::clone(&transport), "find-silent");

    let refused = found(&ctx, "Ada", "harbour", None).await;

    assert_eq!(code(&refused).as_deref(), Some("WISH-8"));
}

#[tokio::test]
async fn a_request_service_that_will_not_say_what_the_member_may_ask_for_is_not_working() {
    let mut routes = vec![call("left", unavailable())];
    routes.extend(pages());
    let transport = house(routes);
    let ctx = ctx_over(Arc::clone(&transport), "find-allowance-unread");

    let refused = found(&ctx, "Ada", "harbour", None).await;

    assert_eq!(code(&refused).as_deref(), Some("WISH-8"));
    assert!(!asked_for(&transport, intake("search")));
}

#[tokio::test]
async fn the_page_says_in_the_households_words_what_the_member_may_ask_for() {
    let mut routes = vec![call("requesting", account(true))];
    routes.extend(pages());
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
