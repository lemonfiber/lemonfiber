//! What a member plays, asked of the media server about that member's own account.

use super::{reader, SIGNED_IN};
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::http::Method;
use lemonfiber_ports::service::{HowFar, Medium, Screening};

/// The requests after the sign-in, as method and URL.
fn asked(fake: &Fake) -> Vec<(Method, String)> {
    fake.requests()
        .into_iter()
        .skip(1)
        .map(|request| (request.method, request.url))
        .collect()
}

/// The token the media server opens a device's session with: thirty-two hex digits,
/// built rather than written, so nothing reading the source takes it for a key.
fn session() -> String {
    "fe".repeat(16)
}

/// A device is signed in by code as the member: the code asked for as the device, the
/// administrator authorising it for the member, and the secret traded for the session.
#[tokio::test]
async fn a_device_is_signed_in_by_code_as_the_member_and_answered_its_token() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, r#"{"Code":"123456","Secret":"the-secret"}"#),
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, "true"),
        Answer::reply(200, format!(r#"{{"AccessToken":"{}"}}"#, session())),
    ]);
    let opened = reader(&fake).signed_in("a7f3", "a-phone-0123").await;
    assert_eq!(opened.ok().flatten(), Some(session()));
    let requests = fake.requests();
    let urls: Vec<(Method, String)> = requests
        .iter()
        .map(|request| (request.method, request.url.clone()))
        .collect();
    assert_eq!(
        urls,
        vec![
            (
                Method::Post,
                "http://127.0.0.1:8096/QuickConnect/Initiate".to_owned()
            ),
            (
                Method::Post,
                "http://127.0.0.1:8096/Users/AuthenticateByName".to_owned()
            ),
            (
                Method::Post,
                "http://127.0.0.1:8096/QuickConnect/Authorize?code=123456&userId=a7f3".to_owned()
            ),
            (
                Method::Post,
                "http://127.0.0.1:8096/Users/AuthenticateWithQuickConnect".to_owned()
            ),
        ]
    );
    assert_eq!(
        requests.last().and_then(|request| request.body.as_deref()),
        Some(r#"{"Secret":"the-secret"}"#)
    );
}

/// A server that signs no device in by code says so, rather than failing.
#[tokio::test]
async fn a_server_that_signs_no_device_in_by_code_is_answered_as_nothing() {
    let fake = Fake::in_turn(vec![Answer::reply(401, "")]);
    let opened = reader(&fake).signed_in("a7f3", "a-phone-0123").await;
    assert_eq!(opened.ok(), Some(None));
}

/// A code the administrator could not authorise, and a session with no token, are
/// failures rather than grants.
#[tokio::test]
async fn an_unauthorised_code_and_an_empty_session_are_failures() {
    let issued = r#"{"Code":"123456","Secret":"the-secret"}"#;
    let fake = Fake::in_turn(vec![
        Answer::reply(200, issued),
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, "false"),
    ]);
    assert!(reader(&fake)
        .signed_in("a7f3", "a-phone-0123")
        .await
        .is_err());
    let fake = Fake::in_turn(vec![
        Answer::reply(200, issued),
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, "true"),
        Answer::reply(200, r#"{"AccessToken":""}"#),
    ]);
    assert!(reader(&fake)
        .signed_in("a7f3", "a-phone-0123")
        .await
        .is_err());
}

/// A film is answered with what a title's page shows, as the member's account reads it.
#[tokio::test]
async fn a_film_is_answered_with_its_details_as_the_member_reads_it() {
    let film = r#"{"Id":"f1","Name":"Heat","Type":"Movie","ProductionYear":1995,
        "Overview":"A heist.","RunTimeTicks":102000000000,"Genres":["Crime"],
        "OfficialRating":"16","PremiereDate":"1995-12-15T00:00:00.0000000Z",
        "ImageTags":{"Primary":"p"},"IsFolder":false}"#;
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, film),
    ]);
    let title = reader(&fake).title(Some("a7f3"), "f1").await.ok().flatten();
    let read = title.map(|title| {
        (
            title.minutes,
            title.genres,
            title.certificate,
            title.released,
            title.held.holds.plays,
            title.seasons.len(),
        )
    });
    assert_eq!(
        read,
        Some((
            Some(170),
            vec!["Crime".to_owned()],
            Some("16".to_owned()),
            Some("1995-12-15".to_owned()),
            true,
            0
        ))
    );
    assert_eq!(
        asked(&fake),
        vec![(
            Method::Get,
            "http://127.0.0.1:8096/Items/f1?userId=a7f3".to_owned()
        )]
    );
}

/// A title the member's account may not see is absent, the server's 404 read as an answer.
#[tokio::test]
async fn a_title_outside_the_members_limits_is_absent() {
    let fake = Fake::in_turn(vec![Answer::reply(200, SIGNED_IN), Answer::reply(404, "")]);
    let title = reader(&fake).title(Some("a7f3"), "hidden").await;
    assert_eq!(title.ok(), Some(None));
}

/// A series carries its seasons, each with the episodes filed under it, in order.
#[tokio::test]
async fn a_series_carries_each_season_with_its_own_episodes() {
    let series = r#"{"Id":"s1","Name":"The Wire","Type":"Series","IsFolder":true}"#;
    let seasons = r#"{"Items":[
        {"Id":"x1","Name":"Season 1","Type":"Season","IndexNumber":1,"IsFolder":true},
        {"Id":"x2","Name":"Season 2","Type":"Season","IndexNumber":2,"IsFolder":true}]}"#;
    let episodes = r#"{"Items":[
        {"Id":"e1","Name":"The Target","Type":"Episode","IndexNumber":1,"SeasonId":"x1"},
        {"Id":"e2","Name":"The Detail","Type":"Episode","IndexNumber":2,"SeasonId":"x1"},
        {"Id":"e3","Name":"Ebb Tide","Type":"Episode","IndexNumber":1,"SeasonId":"x2"}]}"#;
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, series),
        Answer::reply(200, seasons),
        Answer::reply(200, episodes),
    ]);
    let title = reader(&fake).title(Some("a7f3"), "s1").await.ok().flatten();
    let grouped: Vec<(Option<u32>, Vec<String>)> = title
        .map(|title| title.seasons)
        .unwrap_or_default()
        .into_iter()
        .map(|season| {
            let ids = season.episodes.into_iter().map(|one| one.held.id).collect();
            (season.number, ids)
        })
        .collect();
    assert_eq!(
        grouped,
        vec![
            (Some(1), vec!["e1".to_owned(), "e2".to_owned()]),
            (Some(2), vec!["e3".to_owned()]),
        ]
    );
}

/// What a member was part-way through says how far, in seconds.
#[tokio::test]
async fn part_way_says_how_far_in_whole_seconds() {
    let resume = r#"{"Items":[{"Id":"e2","Name":"The Detail","Type":"Episode",
        "RunTimeTicks":36000000000,"UserData":{"PlaybackPositionTicks":12000000000}}]}"#;
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, resume),
    ]);
    let read: Vec<_> = reader(&fake)
        .part_way("a7f3", 12)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|one| (one.held.medium, one.position, one.length))
        .collect();
    assert_eq!(read, vec![(Medium::Episode, 1200, Some(3600))]);
    assert_eq!(
        asked(&fake),
        vec![(
            Method::Get,
            "http://127.0.0.1:8096/UserItems/Resume?userId=a7f3&Limit=12&MediaTypes=Video"
                .to_owned()
        )]
    );
}

/// Progress is the member's own, in the server's ticks, and a finish is a finish.
#[tokio::test]
async fn progress_is_recorded_as_the_members_own() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, "{}"),
        Answer::reply(200, "{}"),
    ]);
    let client = reader(&fake);
    let going = HowFar {
        position: 90,
        ended: false,
    };
    let done = HowFar {
        position: 5000,
        ended: true,
    };
    assert!(client.progressed("a7f3", "f1", &going).await.is_ok());
    assert!(client.progressed("a7f3", "f1", &done).await.is_ok());
    let bodies: Vec<_> = fake
        .requests()
        .into_iter()
        .skip(1)
        .map(|request| (request.url, request.body))
        .collect();
    let url = "http://127.0.0.1:8096/UserItems/f1/UserData?userId=a7f3".to_owned();
    assert_eq!(
        bodies,
        vec![
            (
                url.clone(),
                Some(r#"{"PlaybackPositionTicks":900000000,"Played":false}"#.to_owned())
            ),
            (
                url,
                Some(r#"{"PlaybackPositionTicks":0,"Played":true}"#.to_owned())
            ),
        ]
    );
}

/// Signing a device out names it, encoded, and a refusal is a failure.
#[tokio::test]
async fn a_device_is_signed_out_by_name_and_a_refusal_is_said() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(204, ""),
        Answer::reply(500, ""),
    ]);
    let client = reader(&fake);
    assert!(client.sign_out("phone one").await.is_ok());
    assert!(client.sign_out("phone one").await.is_err());
    assert_eq!(
        asked(&fake).first(),
        Some(&(
            Method::Delete,
            "http://127.0.0.1:8096/Devices?id=phone+one".to_owned()
        ))
    );
}

/// Each step of signing a device in fails as a failure where the server is gone or
/// answers something it cannot mean, rather than as a session or as nothing.
#[tokio::test]
async fn every_step_of_signing_a_device_in_fails_where_the_server_is_gone_or_garbled() {
    let issued = || Answer::reply(200, r#"{"Code":"123456","Secret":"the-code"}"#);
    let steps = vec![
        vec![Answer::Silent],
        vec![Answer::reply(200, "not json")],
        vec![issued(), Answer::reply(200, SIGNED_IN), Answer::Silent],
        vec![
            issued(),
            Answer::reply(200, SIGNED_IN),
            Answer::reply(200, "maybe"),
        ],
        vec![
            issued(),
            Answer::reply(200, SIGNED_IN),
            Answer::reply(200, "true"),
            Answer::Silent,
        ],
        vec![
            issued(),
            Answer::reply(200, SIGNED_IN),
            Answer::reply(200, "true"),
            Answer::reply(200, "not json"),
        ],
    ];
    for (at, answers) in steps.into_iter().enumerate() {
        let fake = Fake::in_turn(answers);
        let opened = reader(&fake).signed_in("a7f3", "a-phone-0123").await;
        assert!(opened.is_err(), "step {at} opened a session");
    }
}

/// A title, a series' seasons and its episodes each fail as a failure where the server
/// is gone or garbled, and so does recording progress.
#[tokio::test]
async fn a_title_its_seasons_and_progress_fail_where_the_server_is_gone_or_garbled() {
    let series = || {
        Answer::reply(
            200,
            r#"{"Id":"s1","Name":"Fargo","Type":"Series","IsFolder":true}"#,
        )
    };
    let seasons = || Answer::reply(200, r#"{"Items":[{"Id":"n1","Name":"Season 1"}]}"#);
    let signed = || Answer::reply(200, SIGNED_IN);
    let steps = vec![
        vec![signed(), Answer::Silent],
        vec![signed(), Answer::reply(200, "not json")],
        vec![signed(), series(), Answer::Silent],
        vec![signed(), series(), Answer::reply(200, "not json")],
        vec![signed(), series(), seasons(), Answer::Silent],
        vec![
            signed(),
            series(),
            seasons(),
            Answer::reply(200, "not json"),
        ],
    ];
    for (at, answers) in steps.into_iter().enumerate() {
        let fake = Fake::in_turn(answers);
        let title = reader(&fake).title(None, "s1").await;
        assert!(title.is_err(), "step {at} answered a title");
    }
    let fake = Fake::in_turn(vec![signed(), Answer::Silent]);
    let went = HowFar {
        position: 1,
        ended: false,
    };
    assert!(reader(&fake).progressed("a7f3", "f1", &went).await.is_err());
}
