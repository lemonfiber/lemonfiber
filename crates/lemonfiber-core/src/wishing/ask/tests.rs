use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake as Transport};
use lemonfiber_fixtures::ports::{stamp, DAY, HOUR};

use super::{chosen, counted, said, Choice};
use crate::app::{Ctx, Outcome};
use crate::model::{AskedReport, Became, LeftAfter, TitleKind};
use crate::ports::media::Kind;
use crate::ports::service::{
    Asking, Detail, Headroom, HouseholdRequest, Left, MediaStatus, Quota, RequestStatus, Season,
    Wish,
};
use crate::test_support::requests::{answered, intake, taken, told, unavailable};
use crate::wishing::tests::{
    account, asked, asked_for, call, calls, code, ctx_over, ctx_running, filing, house, rated,
    titled_as, Route, ACCOUNT,
};

/// The report an ask answered with, or nothing where it was refused.
fn report(outcome: Result<Outcome, Box<crate::error::Problem>>) -> Option<AskedReport> {
    match outcome {
        Ok(Outcome::Asked(report)) => Some(report),
        _ => None,
    }
}

/// A film out in Britain, rated twelve, standing in the house as `status`.
fn film(status: MediaStatus) -> Answer {
    answered(&Some(Detail {
        status,
        ..rated(Some("12A"))
    }))
}

/// A series of three seasons: the first here, the second on its way, the third nobody
/// has asked for.
fn series() -> Answer {
    let season = |number, status| Season { number, status };
    answered(&Some(Detail {
        status: MediaStatus::PartlyAvailable,
        seasons: vec![
            season(1, MediaStatus::Available),
            season(2, MediaStatus::Processing),
            season(3, MediaStatus::Unknown),
        ],
        ..rated(None)
    }))
}

/// The house, with the film and the series looked up as given.
fn titled(first: Vec<Route>) -> Arc<Transport> {
    let mut routes = first;
    routes.extend([
        titled_as("603", film(MediaStatus::Unknown)),
        titled_as("1399", series()),
    ]);
    house(routes)
}

/// What the request service was asked to file.
fn filed(transport: &Transport) -> Vec<Wish> {
    told(&transport.requests(), "ask", "wish")
}

/// The film `id` as a wish names it whole.
fn the_film(id: &str) -> Wish {
    Wish {
        id: id.to_owned(),
        kind: Kind::Movies,
        seasons: Vec::new(),
    }
}

/// What the household may ask for where nobody chose otherwise for one person: asks
/// that wait, `quota` of them.
fn household(quota: Option<Quota>) -> Route {
    call(
        "asking",
        answered(&Asking {
            approves_own: false,
            quota,
        }),
    )
}

/// What the member's period has counted where their one film is spent.
fn spent() -> Route {
    call(
        "left",
        answered(&Headroom {
            films: Left {
                limit: Some(1),
                used: 1,
                days: Some(30),
            },
            television: Left::default(),
        }),
    )
}

/// A rehearsal of `ctx`.
fn rehearsing(ctx: Ctx) -> Ctx {
    ctx.rehearsing()
}

#[tokio::test]
async fn a_rehearsal_says_what_the_ask_leaves_and_that_it_waits_and_files_nothing() {
    let transport = titled(Vec::new());
    let ctx = rehearsing(ctx_over(Arc::clone(&transport), "ask-rehearsed"));

    let rehearsed = report(asked(&ctx, "Ada", Kind::Movies, "603", &[]).await);

    assert_eq!(
        rehearsed.as_ref().map(|report| (
            report.outcome,
            report.leaves,
            report.rehearsed,
            report.kind
        )),
        Some((
            Became::Waiting,
            LeftAfter {
                films: Some(0),
                television: Some(5),
            },
            true,
            TitleKind::Movies
        ))
    );
    assert_eq!(
        rehearsed.as_ref().map(|report| report.said.as_str()),
        Some(
            "Asking for this waits for whoever runs the house to say yes. After this, there's \
             room for no more films and 5 more seasons of television."
        )
    );
    assert!(filed(&transport).is_empty());
}

#[tokio::test]
async fn a_rehearsal_for_a_member_who_approves_their_own_says_it_arrives_unasked() {
    let transport = titled(vec![call("requesting", account(true))]);
    let ctx = rehearsing(ctx_over(Arc::clone(&transport), "ask-rehearsed-arrives"));

    let rehearsed = report(asked(&ctx, "Ada", Kind::Movies, "603", &[]).await);

    assert_eq!(
        rehearsed
            .as_ref()
            .map(|report| (report.outcome, report.rehearsed, report.said.as_str())),
        Some((
            Became::Asked,
            true,
            "Asking for this brings it without anybody saying yes first. After this, there's \
             room for no more films and 5 more seasons of television."
        ))
    );
    assert!(filed(&transport).is_empty());
}

#[tokio::test]
async fn an_ask_is_filed_as_the_member_and_arrives_where_they_approve_their_own() {
    let transport = titled(vec![
        call("requesting", account(true)),
        call("ask", filing(false)),
    ]);
    let ctx = ctx_over(Arc::clone(&transport), "ask-arrives");

    let done = report(asked(&ctx, "Ada", Kind::Movies, "603", &[]).await);

    assert_eq!(
        done.as_ref()
            .map(|report| (report.outcome, report.rehearsed)),
        Some((Became::Asked, false))
    );
    assert_eq!(
        done.as_ref().map(|report| report.said.as_str()),
        Some(
            "Asked. It's on its way. After this, there's room for no more films and 5 more \
             seasons of television."
        )
    );
    assert_eq!(filed(&transport), vec![the_film("603")]);
    assert_eq!(
        told::<String>(&transport.requests(), "ask", "member"),
        vec![ACCOUNT.to_owned()]
    );
}

#[tokio::test]
async fn an_ask_from_a_member_whose_asks_wait_waits_for_whoever_runs_the_house() {
    let transport = titled(Vec::new());
    let ctx = ctx_over(Arc::clone(&transport), "ask-waits");

    let done = report(asked(&ctx, "Ada", Kind::Movies, "603", &[]).await);

    assert_eq!(
        done.as_ref().map(|report| report.outcome),
        Some(Became::Waiting)
    );
    assert!(done.as_ref().is_some_and(|report| report
        .said
        .starts_with("Asked. Whoever runs the house will say yes or no.")));
}

#[tokio::test]
async fn a_request_service_that_lets_through_what_should_wait_is_never_reported_as_granted() {
    let transport = titled(vec![call("ask", filing(false))]);
    let ctx = ctx_over(Arc::clone(&transport), "ask-broken-promise");

    let refused = asked(&ctx, "Ada", Kind::Movies, "603", &[]).await;

    assert_eq!(code(&refused).as_deref(), Some("WISH-8"));
    assert_eq!(filed(&transport).len(), 1);
}

#[tokio::test]
async fn a_member_the_request_service_does_not_know_yet_is_given_an_account_and_asks() {
    let transport = titled(vec![
        calls(
            "requesting",
            vec![answered(&None::<String>), account(false)],
        ),
        household(Some(Quota {
            requests: 3,
            days: 7,
        })),
    ]);
    let ctx = ctx_over(Arc::clone(&transport), "ask-unlinked");

    let rehearsed = report(
        asked(
            &rehearsing(ctx_over(titled(Vec::new()), "ask-unlinked-r")),
            "Ada",
            Kind::Movies,
            "603",
            &[],
        )
        .await,
    );
    let done = report(asked(&ctx, "Ada", Kind::Movies, "603", &[]).await);

    assert!(rehearsed.is_some());
    assert_eq!(
        done.as_ref()
            .map(|report| (report.outcome, report.leaves.films)),
        Some((Became::Waiting, Some(2)))
    );
    assert_eq!(
        told::<Vec<String>>(&transport.requests(), "link_members", "members"),
        vec![vec!["a7f3".to_owned()]]
    );
    assert_eq!(filed(&transport).len(), 1);
}

#[tokio::test]
async fn a_member_the_request_service_cannot_be_given_an_account_for_is_not_working() {
    let unknown = || answered(&None::<String>);
    for (accounts, linked, why) in [
        (vec![unknown()], taken(), "still unknown once linked"),
        (vec![unknown()], unavailable(), "linking refused"),
        (
            vec![unknown(), unavailable()],
            taken(),
            "unreadable once linked",
        ),
    ] {
        let transport = titled(vec![
            calls("requesting", accounts),
            call("link_members", linked),
            household(None),
        ]);
        let ctx = ctx_over(Arc::clone(&transport), "ask-unlinkable");

        let refused = asked(&ctx, "Ada", Kind::Movies, "603", &[]).await;

        assert_eq!(code(&refused).as_deref(), Some("WISH-8"), "{why}");
        assert!(asked_for(&transport, intake("link_members")), "{why}");
        assert!(filed(&transport).is_empty(), "{why}");
    }
}

#[test]
fn an_ask_under_no_limit_says_nothing_about_room() {
    let unlimited = LeftAfter {
        films: None,
        television: None,
    };

    assert_eq!(
        said(Became::Asked, true, &unlimited, false),
        "Asked. It's on its way."
    );
}

#[tokio::test]
async fn a_title_outside_the_members_limits_is_no_such_title_and_nothing_is_filed() {
    let transport = titled(vec![titled_as("604", answered(&Some(rated(Some("15")))))]);
    let ctx = ctx_over(Arc::clone(&transport), "ask-restricted");

    let restricted = asked(&ctx, "Bea", Kind::Movies, "604", &[]).await;
    let unknown = asked(&ctx, "Bea", Kind::Movies, "605", &[]).await;
    let blank = asked(&ctx, "Bea", Kind::Movies, " ", &[]).await;
    let allowed = report(
        asked(
            &rehearsing(ctx_over(titled(Vec::new()), "ask-allowed")),
            "Bea",
            Kind::Movies,
            "603",
            &[],
        )
        .await,
    );

    assert_eq!(code(&restricted).as_deref(), Some("WISH-3"));
    assert_eq!(code(&unknown).as_deref(), Some("WISH-3"));
    assert_eq!(code(&blank).as_deref(), Some("WISH-3"));
    assert_eq!(
        restricted
            .as_ref()
            .err()
            .map(|problem| problem.summary.clone()),
        unknown
            .as_ref()
            .err()
            .map(|problem| problem.summary.clone())
    );
    assert!(allowed.is_some());
    assert!(filed(&transport).is_empty());
}

#[tokio::test]
async fn a_kind_the_house_cannot_fetch_is_refused_before_the_title_is_looked_up() {
    for (running, kind, id, said) in [
        ("radarr", Kind::Tv, "1399", "series"),
        ("sonarr", Kind::Movies, "603", "films"),
    ] {
        let transport = titled(Vec::new());
        let ctx = ctx_running(Arc::clone(&transport), "ask-no-kind", &[running]);

        let refused = asked(&ctx, "Ada", kind, id, &[]).await;

        assert_eq!(code(&refused).as_deref(), Some("WISH-4"), "{said}");
        assert_eq!(
            refused
                .as_ref()
                .err()
                .map(|problem| problem.summary.as_str()),
            Some(format!("This house can't fetch {said} right now.").as_str())
        );
        assert!(!asked_for(&transport, intake("detail")), "{said}");
    }
}

#[tokio::test]
async fn a_film_already_here_is_refused_and_one_on_its_way_creates_nothing() {
    let here = titled(vec![titled_as("603", film(MediaStatus::Available))]);
    let coming = titled(vec![titled_as("603", film(MediaStatus::Processing))]);

    let refused = asked(
        &ctx_over(Arc::clone(&here), "ask-here"),
        "Ada",
        Kind::Movies,
        "603",
        &[],
    )
    .await;
    let on_its_way = report(
        asked(
            &ctx_over(Arc::clone(&coming), "ask-coming"),
            "Ada",
            Kind::Movies,
            "603",
            &[],
        )
        .await,
    );

    assert_eq!(code(&refused).as_deref(), Some("WISH-5"));
    assert_eq!(
        on_its_way.as_ref().map(|report| (
            report.outcome,
            report.said.as_str(),
            report.leaves.films
        )),
        Some((
            Became::OnItsWay,
            "Somebody already asked for this, and it's on its way.",
            Some(1)
        ))
    );
    assert!(filed(&here).is_empty());
    assert!(filed(&coming).is_empty());
}

#[tokio::test]
async fn a_series_asks_only_for_the_seasons_not_already_here_or_on_their_way() {
    let transport = titled(Vec::new());
    let ctx = ctx_over(Arc::clone(&transport), "ask-seasons");

    let every = report(asked(&ctx, "Ada", Kind::Tv, "1399", &[]).await);

    assert_eq!(
        every
            .as_ref()
            .map(|report| (report.seasons.clone(), report.leaves)),
        Some((
            vec![3],
            LeftAfter {
                films: Some(1),
                television: Some(4),
            }
        ))
    );
    assert_eq!(
        filed(&transport),
        vec![Wish {
            id: "1399".to_owned(),
            kind: Kind::Tv,
            seasons: vec![3],
        }]
    );
}

#[tokio::test]
async fn seasons_named_wrongly_are_refused_and_seasons_all_here_or_coming_create_nothing() {
    let transport = titled(Vec::new());
    let ctx = ctx_over(Arc::clone(&transport), "ask-named-seasons");

    let missing = asked(&ctx, "Ada", Kind::Tv, "1399", &[9]).await;
    let specials = asked(&ctx, "Ada", Kind::Tv, "1399", &[0]).await;
    let film = asked(&ctx, "Ada", Kind::Movies, "603", &[1]).await;
    let here = asked(&ctx, "Ada", Kind::Tv, "1399", &[1]).await;
    let coming = report(asked(&ctx, "Ada", Kind::Tv, "1399", &[1, 2]).await);

    assert_eq!(code(&missing).as_deref(), Some("WISH-10"));
    assert_eq!(code(&specials).as_deref(), Some("WISH-10"));
    assert_eq!(code(&film).as_deref(), Some("WISH-10"));
    assert_eq!(code(&here).as_deref(), Some("WISH-5"));
    assert_eq!(coming.map(|report| report.outcome), Some(Became::OnItsWay));
    assert!(filed(&transport).is_empty());
}

#[tokio::test]
async fn an_ask_past_the_members_allowance_is_refused_saying_when_it_frees_up() {
    let made =
        |id, member: &str, member_id: Option<&str>, at, statuses: (RequestStatus, MediaStatus)| {
            HouseholdRequest {
                arrived: None,
                shelf_id: None,
                id,
                made: Some(format!("{}.000Z", stamp(at))),
                member: member.to_owned(),
                member_id: member_id.map(str::to_owned),
                kind: Some(Kind::Movies),
                title_id: None,
                item: None,
                request_status: Some(statuses.0),
                media_status: Some(statuses.1),
            }
        };
    let arrived = (RequestStatus::Approved, MediaStatus::Available);
    let requests = vec![
        made(3, "Ada", Some("a7f3"), -DAY + 10 * HOUR, arrived),
        made(
            4,
            "Ada",
            Some("a7f3"),
            -16 * DAY + 10 * HOUR,
            (RequestStatus::Declined, MediaStatus::Unknown),
        ),
        made(5, "Bea", Some("b8e4"), -15 * DAY + 10 * HOUR, arrived),
        made(6, "ada", None, 8 * HOUR, arrived),
    ];
    let transport = titled(vec![spent(), call("requests", answered(&requests))]);
    let ctx = ctx_over(Arc::clone(&transport), "ask-spent");

    let refused = asked(&ctx, "Ada", Kind::Movies, "603", &[]).await;

    assert_eq!(code(&refused).as_deref(), Some("WISH-6"));
    assert_eq!(
        refused
            .as_ref()
            .err()
            .map(|problem| problem.summary.as_str()),
        Some("You've asked for all you can for now.")
    );
    assert!(refused
        .as_ref()
        .err()
        .is_some_and(
            |problem| problem
                .remedies
                .first()
                .is_some_and(|remedy| remedy.action.starts_with(&format!(
                    "You can ask again from {}",
                    &stamp(29 * DAY)[..10]
                )))
        ));
    assert!(filed(&transport).is_empty());
}

#[tokio::test]
async fn an_ask_past_an_allowance_whose_dates_are_unknown_is_refused_with_the_plain_remedy() {
    let transport = titled(vec![spent()]);
    let ctx = ctx_over(Arc::clone(&transport), "ask-spent-undated");

    let refused = asked(&ctx, "Ada", Kind::Movies, "603", &[]).await;

    assert!(refused.as_ref().err().is_some_and(|problem| problem
        .remedies
        .first()
        .is_some_and(|remedy| remedy.action == "Ask again once the allowance frees up.")));
}

#[tokio::test]
async fn a_house_with_no_room_says_so_as_the_houses_and_files_nothing() {
    let transport = titled(Vec::new());
    let mut ctx = ctx_over(Arc::clone(&transport), "ask-no-room");
    let full = crate::ports::filesystem::StorageFacts {
        point: std::path::PathBuf::new(),
        kind: crate::ports::filesystem::FsKind::Linking("test".to_owned()),
        removable: false,
        available: 0,
        total: 4 * 1024 * 1024 * 1024 * 1024,
    };
    ctx = ctx.with_filesystem(Arc::new(
        crate::test_support::SeedFs::keyed(None).with_facts(full),
    ));
    ctx.settings.data_root = ctx
        .settings
        .env_file
        .clone()
        .and_then(|env| env.parent().map(std::path::Path::to_path_buf));

    let refused = asked(&ctx, "Ada", Kind::Movies, "603", &[]).await;

    assert_eq!(code(&refused).as_deref(), Some("WISH-7"));
    assert!(filed(&transport).is_empty());
}

#[tokio::test]
async fn a_title_not_out_yet_is_asked_for_and_said_to_arrive_once_it_is() {
    let transport = titled(vec![titled_as(
        "606",
        answered(&Some(Detail {
            released: false,
            ..Detail::default()
        })),
    )]);
    let ctx = ctx_over(Arc::clone(&transport), "ask-unreleased");

    let done = report(asked(&ctx, "Ada", Kind::Movies, "606", &[]).await);

    assert_eq!(done.as_ref().map(|report| report.released), Some(false));
    assert!(done.as_ref().is_some_and(|report| report
        .said
        .contains("It isn't out yet, so it arrives once it is.")));
}

#[tokio::test]
async fn a_request_service_that_will_not_look_up_or_file_is_asking_not_working() {
    let lookup = titled(vec![titled_as("603", unavailable())]);
    let filing = titled(vec![call("ask", unavailable())]);
    let allowance = titled(vec![call("left", unavailable())]);

    let unlooked = asked(
        &ctx_over(lookup, "ask-lookup-fails"),
        "Ada",
        Kind::Movies,
        "603",
        &[],
    )
    .await;
    let unfiled = asked(
        &ctx_over(filing, "ask-filing-fails"),
        "Ada",
        Kind::Movies,
        "603",
        &[],
    )
    .await;
    let unread = asked(
        &ctx_over(allowance, "ask-allowance-fails"),
        "Ada",
        Kind::Movies,
        "603",
        &[],
    )
    .await;

    assert_eq!(code(&unlooked).as_deref(), Some("WISH-8"));
    assert_eq!(code(&unfiled).as_deref(), Some("WISH-8"));
    assert_eq!(code(&unread).as_deref(), Some("WISH-8"));
}

#[test]
fn a_series_with_no_seasons_known_stands_as_the_title_does() {
    let wish = Wish {
        id: "9".to_owned(),
        kind: Kind::Tv,
        seasons: Vec::new(),
    };
    let detail = |status| Detail {
        status,
        ..Detail::default()
    };
    let choice = |status| chosen(&wish, &detail(status)).ok();
    assert_eq!(choice(MediaStatus::Available), Some(Choice::Here));
    assert_eq!(choice(MediaStatus::Pending), Some(Choice::OnItsWay));
    assert_eq!(choice(MediaStatus::Processing), Some(Choice::OnItsWay));
    assert_eq!(choice(MediaStatus::PartlyAvailable), Some(Choice::OnItsWay));
    assert_eq!(choice(MediaStatus::Unknown), Some(Choice::Ask(Vec::new())));
    assert_eq!(choice(MediaStatus::Deleted), Some(Choice::Ask(Vec::new())));
}

#[test]
fn a_season_taken_away_may_be_asked_for_again() {
    let wish = Wish {
        id: "9".to_owned(),
        kind: Kind::Tv,
        seasons: vec![1],
    };
    let detail = Detail {
        seasons: vec![Season {
            number: 1,
            status: MediaStatus::Deleted,
        }],
        ..Detail::default()
    };
    assert_eq!(chosen(&wish, &detail).ok(), Some(Choice::Ask(vec![1])));
}

#[test]
fn a_film_counts_one_and_a_series_one_for_each_season_or_one_asked_whole() {
    assert_eq!(counted(Kind::Movies, &[]), 1);
    assert_eq!(counted(Kind::Tv, &[1, 2, 3]), 3);
    assert_eq!(counted(Kind::Tv, &[]), 1);
}
