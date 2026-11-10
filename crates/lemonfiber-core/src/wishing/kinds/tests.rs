use crate::app::{Ctx, Outcome, Wishing};
use crate::collected::Medium;
use crate::error::codes::wish::{NOBODY_ASKING, NOT_A_MEMBER, NOT_WORKING};
use crate::model::{AskableKind, TitleKind, TitleUnit};
use crate::ports::docker::{Health, Lifecycle};
use crate::ports::service::{AskedWith, BookUnit, CuratedUnit, Headroom, Left, MusicUnit};
use crate::test_support::adapted::{answered, running, unavailable};
use crate::test_support::collecting::{at_of, speaking, speaking_of, BOOKS};
use crate::test_support::curators::MUSIC;
use crate::test_support::Reporting;
use crate::wishing::tests::{call, code, ctx_engine, house, Route};

fn answering(medium: Medium, units: Vec<CuratedUnit>) -> Route {
    (
        at_of(medium, "profiles"),
        None,
        vec![answered(&AskedWith {
            units,
            root_folders: vec!["/library".to_owned()],
            quality: Vec::new(),
            metadata: Vec::new(),
        })],
    )
}

fn curated(mut routes: Vec<Route>) -> Vec<Route> {
    routes.extend([
        answering(
            Medium::Music,
            vec![MusicUnit::Album.into(), MusicUnit::Artist.into()],
        ),
        answering(
            Medium::Books,
            vec![BookUnit::Book.into(), BookUnit::Author.into()],
        ),
    ]);
    routes
}

fn a_house(routes: Vec<Route>, tag: &str, curating: bool) -> Ctx {
    let routes = curated(routes);
    let engine = Reporting::holding(&["sonarr", "radarr"], Lifecycle::Running, Health::Healthy)
        .alongside(running(&[MUSIC, BOOKS]));
    let ctx = ctx_engine(house(routes), &format!("kinds-{tag}"), engine);
    if curating {
        speaking(&ctx);
        speaking_of(&ctx, crate::collected::Medium::Books);
    }
    ctx
}

async fn kinds(ctx: &Ctx, member: &str) -> Result<Outcome, Box<crate::error::Problem>> {
    crate::wishing::wished(
        ctx,
        Wishing::Kinds {
            member: member.to_owned(),
        },
    )
    .await
}

fn listed(outcome: Result<Outcome, Box<crate::error::Problem>>) -> Option<Vec<AskableKind>> {
    match outcome {
        Ok(Outcome::AskableKinds(report)) => Some(report.kinds),
        _ => None,
    }
}

fn taking(films: Option<u32>, television: Option<u32>) -> Headroom {
    let left = |limit| Left {
        limit,
        used: 0,
        days: Some(7),
    };
    Headroom {
        films: left(films),
        television: left(television),
    }
}

fn kind(kind: TitleKind, units: &[TitleUnit]) -> AskableKind {
    AskableKind {
        kind,
        units: units.to_vec(),
    }
}

#[tokio::test]
async fn every_kind_a_curator_fetches_now_is_listed_with_the_units_it_answers_for() {
    let ctx = a_house(Vec::new(), "every", true);

    let read = listed(kinds(&ctx, "Ada").await);

    assert_eq!(
        read,
        Some(vec![
            kind(TitleKind::Tv, &[]),
            kind(TitleKind::Movies, &[]),
            kind(TitleKind::Music, &[TitleUnit::Album, TitleUnit::Artist]),
            kind(TitleKind::Books, &[TitleUnit::Book, TitleUnit::Author]),
        ])
    );
}

#[tokio::test]
async fn music_and_books_are_left_out_where_no_curator_speaks_for_them() {
    let ctx = a_house(Vec::new(), "uncurated", false);

    let read = listed(kinds(&ctx, "Ada").await);

    assert_eq!(
        read,
        Some(vec![kind(TitleKind::Tv, &[]), kind(TitleKind::Movies, &[])])
    );
}

#[tokio::test]
async fn a_kind_the_members_allowance_takes_none_of_is_left_out() {
    let no_films = a_house(
        vec![call("left", answered(&taking(Some(0), None)))],
        "no-films",
        true,
    );
    let no_series = a_house(
        vec![call("left", answered(&taking(None, Some(0))))],
        "no-series",
        true,
    );

    let films = listed(kinds(&no_films, "Ada").await);
    let series = listed(kinds(&no_series, "Ada").await);

    assert_eq!(films, Some(vec![kind(TitleKind::Tv, &[])]));
    assert_eq!(
        series.map(|kinds| kinds.into_iter().map(|one| one.kind).collect::<Vec<_>>()),
        Some(vec![TitleKind::Movies, TitleKind::Music, TitleKind::Books])
    );
}

#[tokio::test]
async fn the_member_is_checked_as_a_search_checks_them() {
    let ctx = a_house(Vec::new(), "nobody", true);
    let unread = a_house(vec![call("left", unavailable())], "unread", true);

    assert_eq!(
        code(&kinds(&ctx, " ").await),
        Some(NOBODY_ASKING.to_string())
    );
    assert_eq!(
        code(&kinds(&ctx, "Zed").await),
        Some(NOT_A_MEMBER.to_string())
    );
    assert_eq!(
        code(&kinds(&unread, "Ada").await),
        Some(NOT_WORKING.to_string())
    );
}

#[tokio::test]
async fn a_curator_answering_no_units_or_not_at_all_leaves_its_medium_out() {
    let ctx = a_house(
        vec![
            answering(Medium::Music, Vec::new()),
            (at_of(Medium::Books, "profiles"), None, vec![unavailable()]),
        ],
        "no-units",
        true,
    );

    let read = listed(kinds(&ctx, "Ada").await);

    assert_eq!(
        read,
        Some(vec![kind(TitleKind::Tv, &[]), kind(TitleKind::Movies, &[])])
    );
}
