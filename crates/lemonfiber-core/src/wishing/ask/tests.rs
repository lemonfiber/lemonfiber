use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake as Transport};
use lemonfiber_fixtures::ports::{stamp, DAY, HOUR};

use super::{chosen, counted, said, Choice};
use crate::app::{Ctx, Outcome};
use crate::model::{AskedReport, Became, LeftAfter, TitleKind};
use crate::ports::http::Method;
use crate::ports::media::Kind;
use crate::ports::service::{Detail, MediaStatus, Season, Wish};
use crate::wishing::tests::{
    asked, asked_for, code, ctx_over, ctx_running, house, route, Route, APPROVES_OWN,
};

/// A film out in Britain, rated twelve, that nobody has asked for.
const FILM: &str = r#"{"status":"Released","releases":{"results":[
    {"iso_3166_1":"GB","release_dates":[{"certification":"12A"}]}]}}"#;

/// The same film, already here.
const FILM_HERE: &str = r#"{"status":"Released","mediaInfo":{"status":5}}"#;

/// The same film, on its way.
const FILM_COMING: &str = r#"{"status":"Released","mediaInfo":{"status":3}}"#;

/// A film rated fifteen in Britain.
const FILM_FIFTEEN: &str = r#"{"status":"Released","releases":{"results":[
    {"iso_3166_1":"GB","release_dates":[{"certification":"15"}]}]}}"#;

/// A film not out yet.
const FILM_COMING_OUT: &str = r#"{"status":"Post Production"}"#;

/// A series of three seasons: the first here, the second on its way, the third nobody
/// has asked for.
const SERIES: &str = r#"{"status":"Ended","firstAirDate":"2011-04-17",
    "seasons":[{"seasonNumber":0},{"seasonNumber":1},{"seasonNumber":2},{"seasonNumber":3}],
    "mediaInfo":{"status":4,"seasons":[{"seasonNumber":1,"status":5},{"seasonNumber":2,"status":3}]}}"#;

/// The ask filed and approved on the spot.
const FILED_APPROVED: &str = r#"{"id":8,"status":2}"#;

/// The report an ask answered with, or nothing where it was refused.
fn report(outcome: Result<Outcome, Box<crate::error::Problem>>) -> Option<AskedReport> {
    match outcome {
        Ok(Outcome::Asked(report)) => Some(report),
        _ => None,
    }
}

/// The house, with the film and the series looked up as given.
fn titled(first: Vec<Route>) -> Arc<Transport> {
    let mut routes = first;
    routes.extend([
        route(Method::Get, "/api/v1/movie/603", 200, FILM),
        route(Method::Get, "/api/v1/tv/1399", 200, SERIES),
    ]);
    house(routes)
}

/// The bodies the request service was asked to file.
fn filed(transport: &Transport) -> Vec<serde_json::Value> {
    transport
        .requests()
        .into_iter()
        .filter(|request| request.method == Method::Post && request.url.ends_with("/request"))
        .filter_map(|request| request.body)
        .filter_map(|body| serde_json::from_str(&body).ok())
        .collect()
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
    let transport = titled(vec![route(
        Method::Get,
        "/api/v1/user/jellyfin/",
        200,
        APPROVES_OWN,
    )]);
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
        route(Method::Get, "/api/v1/user/jellyfin/", 200, APPROVES_OWN),
        route(Method::Post, "/api/v1/request", 201, FILED_APPROVED),
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
    assert_eq!(
        filed(&transport),
        vec![serde_json::json!({"mediaType":"movie","mediaId":603})]
    );
    assert!(transport.requests().iter().any(|request| request
        .headers
        .iter()
        .any(|(name, value)| name == "X-API-User" && value == "4")));
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
    let transport = titled(vec![route(
        Method::Post,
        "/api/v1/request",
        201,
        FILED_APPROVED,
    )]);
    let ctx = ctx_over(Arc::clone(&transport), "ask-broken-promise");

    let refused = asked(&ctx, "Ada", Kind::Movies, "603", &[]).await;

    assert_eq!(code(&refused).as_deref(), Some("WISH-8"));
    assert_eq!(filed(&transport).len(), 1);
}

#[tokio::test]
async fn a_member_the_request_service_does_not_know_yet_is_given_an_account_and_asks() {
    let transport = titled(vec![
        (
            Method::Get,
            "/api/v1/user/jellyfin/",
            vec![
                Answer::reply(404, "{}"),
                Answer::reply(200, crate::wishing::tests::WAITS),
            ],
        ),
        route(
            Method::Get,
            "/settings/main",
            200,
            r#"{"defaultPermissions":32,"defaultQuotas":{"movie":{"quotaLimit":3,"quotaDays":7}}}"#,
        ),
        route(Method::Post, "/user/import-from-jellyfin", 201, "[]"),
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
    assert!(asked_for(&transport, "/user/import-from-jellyfin"));
    assert_eq!(filed(&transport).len(), 1);
}

#[tokio::test]
async fn a_member_the_request_service_cannot_be_given_an_account_for_is_not_working() {
    for (accounts, imported, why) in [
        (vec![404], 201, "still unknown once linked"),
        (vec![404], 500, "linking refused"),
        (vec![404, 500], 201, "unreadable once linked"),
    ] {
        let account: Route = (
            Method::Get,
            "/api/v1/user/jellyfin/",
            accounts
                .into_iter()
                .map(|status| Answer::reply(status, "{}"))
                .collect(),
        );
        let transport = titled(vec![
            account,
            route(
                Method::Get,
                "/settings/main",
                200,
                r#"{"defaultPermissions":32,"defaultQuotas":{}}"#,
            ),
            route(Method::Post, "/user/import-from-jellyfin", imported, "[]"),
        ]);
        let ctx = ctx_over(Arc::clone(&transport), "ask-unlinkable");

        let refused = asked(&ctx, "Ada", Kind::Movies, "603", &[]).await;

        assert_eq!(code(&refused).as_deref(), Some("WISH-8"), "{why}");
        assert!(asked_for(&transport, "/user/import-from-jellyfin"), "{why}");
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
    let transport = titled(vec![
        route(Method::Get, "/api/v1/movie/604", 200, FILM_FIFTEEN),
        route(Method::Get, "/api/v1/movie/605", 404, "{}"),
    ]);
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
    for (running, kind, id, said, looked_up) in [
        ("radarr", Kind::Tv, "1399", "series", "/api/v1/tv/"),
        ("sonarr", Kind::Movies, "603", "films", "/api/v1/movie/"),
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
        assert!(!asked_for(&transport, looked_up), "{said}");
    }
}

#[tokio::test]
async fn a_film_already_here_is_refused_and_one_on_its_way_creates_nothing() {
    let here = titled(vec![route(
        Method::Get,
        "/api/v1/movie/603",
        200,
        FILM_HERE,
    )]);
    let coming = titled(vec![route(
        Method::Get,
        "/api/v1/movie/603",
        200,
        FILM_COMING,
    )]);

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
        vec![serde_json::json!({"mediaType":"tv","mediaId":1399,"seasons":[3]})]
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
    let requests = format!(
        r#"{{"pageInfo":{{"results":4}},"results":[
        {{"id":3,"status":2,"createdAt":"{}.000Z","type":"movie",
            "media":{{"status":5}},"requestedBy":{{"displayName":"Ada","jellyfinUserId":"a7f3"}}}},
        {{"id":4,"status":3,"createdAt":"{}.000Z","type":"movie",
            "media":{{"status":1}},"requestedBy":{{"displayName":"Ada","jellyfinUserId":"a7f3"}}}},
        {{"id":5,"status":2,"createdAt":"{}.000Z","type":"movie",
            "media":{{"status":5}},"requestedBy":{{"displayName":"Bea","jellyfinUserId":"b8e4"}}}},
        {{"id":6,"status":2,"createdAt":"{}.000Z","type":"movie",
            "media":{{"status":5}},"requestedBy":{{"displayName":"ada"}}}}]}}"#,
        stamp(-DAY + 10 * HOUR),
        stamp(-16 * DAY + 10 * HOUR),
        stamp(-15 * DAY + 10 * HOUR),
        stamp(8 * HOUR),
    );
    let spent = r#"{"movie":{"days":30,"limit":1,"used":1},"tv":{}}"#;
    let transport = titled(vec![
        route(Method::Get, "/quota", 200, spent),
        route(Method::Get, "/api/v1/request?", 200, &requests),
    ]);
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
    let spent = r#"{"movie":{"days":30,"limit":1,"used":1},"tv":{}}"#;
    let transport = titled(vec![route(Method::Get, "/quota", 200, spent)]);
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
        crate::test_support::SeedFs::keyed(Some("<Config><ApiKey>k</ApiKey></Config>"), None)
            .with_requests(lemonfiber_fixtures::support::SEERR_SETTINGS)
            .with_facts(full),
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
    let transport = titled(vec![route(
        Method::Get,
        "/api/v1/movie/606",
        200,
        FILM_COMING_OUT,
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
    let lookup = titled(vec![route(Method::Get, "/api/v1/movie/603", 500, "")]);
    let filing = titled(vec![route(Method::Post, "/api/v1/request", 500, "")]);
    let allowance = titled(vec![route(Method::Get, "/quota", 500, "")]);

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
