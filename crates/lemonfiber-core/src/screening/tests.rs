use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake as Transport};

use super::{a_device, an_item, lapsed, viewed};
use crate::app::{Ctx, Outcome, Viewing, Whom};
use crate::error::codes::play;
use crate::ports::http::Method;
use crate::ports::service::HowFar;
use crate::test_support::{a_context, a_password, SeedFs};

/// A Servarr config carrying a readable key, so the stack resolves its targets.
const KEYED: &str = "<Config><ApiKey>the-key</ApiKey></Config>";

/// A title on the shelf, by an identifier shaped like the server's.
const FILM: &str = "0123456789abcdef0123456789abcdef";

/// A device, by an id shaped like the one a player keeps.
const PHONE: &str = "a-phone-0123";

/// The token the media server opens a device's session with: thirty-two hex digits,
/// built rather than written, so nothing reading the source takes it for a key.
fn session() -> String {
    "fe".repeat(16)
}

/// The media server's answer to a device trading its secret, opening `token`.
fn opened(token: &str) -> String {
    format!(r#"{{"AccessToken":"{token}"}}"#)
}

/// The accounts the media server holds.
const ACCOUNTS: &str = r#"[{"Id":"a7f3","Name":"Ada","HasPassword":true,
    "Policy":{"EnableAllFolders":true}}]"#;

/// The media server, answering each call by what it asks for.
fn server(routes: Vec<(&'static str, Answer)>) -> Arc<Transport> {
    let mut answered = vec![(
        "/Users/AuthenticateByName",
        Answer::reply(200, r#"{"AccessToken":"token"}"#),
    )];
    answered.extend(routes);
    answered.push(("/Users", Answer::reply(200, ACCOUNTS)));
    answered.push(("", Answer::reply(200, "[]")));
    Transport::by_path(answered)
}

/// A context whose media server can be reached, kept apart from every other case.
fn ctx_over(transport: Arc<Transport>, tag: &str) -> Ctx {
    let dir = lemonfiber_fixtures::scratch::Scratch::named(&format!("screening-{tag}")).kept();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    let mut context = a_context()
        .build()
        .with_filesystem(Arc::new(SeedFs::keyed(Some(KEYED), None)))
        .with_http(transport);
    context.settings.env_file = Some(dir.join(".env"));
    let _ = crate::app::targets::record_secret(
        &context,
        crate::config::JELLYFIN_ADMIN_PASSWORD_KEY,
        &a_password(),
    );
    context
}

/// What the media server was asked, after signing in, as method and path.
fn asked(transport: &Transport) -> Vec<(Method, String)> {
    transport
        .requests()
        .into_iter()
        .filter(|request| !request.url.ends_with("/Users/AuthenticateByName"))
        .map(|request| {
            let path = request
                .url
                .split_once("://")
                .and_then(|(_, rest)| rest.find('/').map(|at| rest[at..].to_owned()))
                .unwrap_or_default();
            (request.method, path)
        })
        .collect()
}

fn code(outcome: Result<Outcome, Box<crate::error::Problem>>) -> Option<String> {
    outcome.err().map(|problem| problem.code.to_string())
}

#[test]
fn an_item_is_thirty_two_hex_digits_with_or_without_its_dashes() {
    assert!(an_item(FILM));
    assert!(an_item("01234567-89ab-cdef-0123-456789abcdef"));
    for not in [
        "",
        "0123456789abcdef0123456789abcde",
        "0123456789abcdef0123456789abcdeg",
        "0123456-789ab-cdef-0123-456789abcdef",
        "../Users/0123456789abcdef0123456789",
        "0123456789abcdef0123456789abcdef?x=1",
    ] {
        assert!(!an_item(not), "{not}");
    }
}

#[tokio::test]
async fn a_title_named_by_something_else_is_refused_before_anything_is_asked() {
    let transport = server(Vec::new());
    let ctx = ctx_over(Arc::clone(&transport), "not-an-item");
    let refused = viewed(
        &ctx,
        Viewing::Title {
            member: Whom::Named("Ada".to_owned()),
            id: "../Users".to_owned(),
        },
    )
    .await;
    assert_eq!(code(refused).as_deref(), Some("PLAY-1"));
    assert!(transport.requests().is_empty());
}

#[tokio::test]
async fn a_title_is_read_as_the_member_and_says_why_it_is_not_located() {
    let film = r#"{"Id":"0123456789abcdef0123456789abcdef","Name":"Heat","Type":"Movie",
        "IsFolder":false}"#;
    let transport = server(vec![("/Items/", Answer::reply(200, film))]);
    let ctx = ctx_over(Arc::clone(&transport), "title");
    let read = viewed(
        &ctx,
        Viewing::Title {
            member: Whom::Named("ada".to_owned()),
            id: FILM.to_owned(),
        },
    )
    .await;
    let report = match read {
        Ok(Outcome::Title(report)) => Some(report),
        _ => None,
    };
    assert_eq!(
        report
            .as_ref()
            .map(|report| (report.member.as_str(), report.id.as_str())),
        Some(("Ada", "a7f3"))
    );
    let at = report
        .and_then(|report| report.title)
        .map(|title| title.held.at)
        .unwrap_or_default();
    assert!(at.unlocated.is_some() && at.stream_from.is_none(), "{at:?}");
    assert!(asked(&transport).contains(&(Method::Get, format!("/Items/{FILM}?userId=a7f3"))));
}

#[tokio::test]
async fn a_title_off_the_shelf_and_a_stranger_are_refused_by_name() {
    let transport = server(vec![("/Items/", Answer::reply(404, ""))]);
    let ctx = ctx_over(transport, "off-the-shelf");
    let off = viewed(
        &ctx,
        Viewing::Title {
            member: Whom::Defaults,
            id: FILM.to_owned(),
        },
    )
    .await;
    assert_eq!(code(off).as_deref(), Some("PLAY-2"));
    let stranger = viewed(
        &ctx,
        Viewing::Grant {
            member: "Zed".to_owned(),
            device: PHONE.to_owned(),
        },
    )
    .await;
    assert_eq!(code(stranger).as_deref(), Some("PLAY-7"));
}

#[tokio::test]
async fn nobody_and_nothing_to_play_from_are_refused() {
    let ctx = ctx_over(server(Vec::new()), "nobody");
    let nobody = viewed(
        &ctx,
        Viewing::PartWay {
            member: Whom::Defaults,
            most: 3,
        },
    )
    .await;
    assert_eq!(code(nobody).as_deref(), Some("PLAY-6"));
    let blank = viewed(
        &ctx,
        Viewing::Watched {
            member: " ".to_owned(),
            id: FILM.to_owned(),
            how_far: HowFar {
                position: 1,
                ended: false,
            },
        },
    )
    .await;
    assert_eq!(code(blank).as_deref(), Some("PLAY-6"));
    let unset = a_context().build();
    let nothing = viewed(
        &unset,
        Viewing::Grant {
            member: "Ada".to_owned(),
            device: PHONE.to_owned(),
        },
    )
    .await;
    assert_eq!(
        code(nothing).as_deref(),
        Some(play::NOTHING_TO_PLAY_FROM.to_string().as_str())
    );
}

#[tokio::test]
async fn part_way_is_located_and_a_server_that_will_not_say_is_unread() {
    let resume = r#"{"Items":[{"Id":"e2","Name":"The Detail","Type":"Episode",
        "RunTimeTicks":36000000000,"UserData":{"PlaybackPositionTicks":12000000000}}]}"#;
    let ctx = ctx_over(
        server(vec![("/UserItems/Resume", Answer::reply(200, resume))]),
        "part-way",
    );
    let read = viewed(
        &ctx,
        Viewing::PartWay {
            member: Whom::Named("a7f3".to_owned()),
            most: 5,
        },
    )
    .await;
    let report = match read {
        Ok(Outcome::PartWay(report)) => Some(report),
        _ => None,
    };
    assert!(report.as_ref().is_some_and(|report| report.available));
    let first = report.as_ref().and_then(|report| report.part_way.first());
    assert_eq!(first.map(|one| one.position), Some(1200));
    assert!(first.is_some_and(|one| one.held.at.unlocated.is_some()));

    let ctx = ctx_over(
        server(vec![("/UserItems/Resume", Answer::reply(500, ""))]),
        "part-way-unread",
    );
    let read = viewed(
        &ctx,
        Viewing::PartWay {
            member: Whom::Named("a7f3".to_owned()),
            most: 5,
        },
    )
    .await;
    assert!(matches!(read, Ok(Outcome::PartWay(report))
        if !report.available && report.findings.len() == 1));
}

/// The media server, signing a device in by code as it does when it is asked.
fn signing_in(opened: String) -> Arc<Transport> {
    server(vec![
        (
            "/QuickConnect/Initiate",
            Answer::reply(200, r#"{"Code":"123456","Secret":"the-secret"}"#),
        ),
        ("/QuickConnect/Authorize", Answer::reply(200, "true")),
        (
            "/Users/AuthenticateWithQuickConnect",
            Answer::reply(200, opened),
        ),
    ])
}

/// A grant for Ada's phone.
fn for_the_phone() -> Viewing {
    Viewing::Grant {
        member: "Ada".to_owned(),
        device: PHONE.to_owned(),
    }
}

#[test]
fn a_device_is_eight_to_sixty_four_letters_digits_and_dashes() {
    assert!(a_device(PHONE));
    assert!(a_device(&"a".repeat(64)));
    assert!(!a_device("short"));
    assert!(!a_device(&"a".repeat(65)));
    assert!(!a_device("a-phone\", DeviceId=\"x"));
    assert!(!a_device("a phone 0123"));
}

#[tokio::test]
async fn a_grant_opens_the_device_session_as_the_member_and_answers_its_token() {
    let session = session();
    let transport = signing_in(opened(&session));
    let ctx = ctx_over(Arc::clone(&transport), "grant");
    let granted = viewed(&ctx, for_the_phone()).await;
    let report = match granted {
        Ok(Outcome::Granted(report)) => Some(report),
        _ => None,
    };
    assert_eq!(
        report.as_ref().map(|report| (
            report.granted,
            report.token.as_deref(),
            report.lasts_until.len()
        )),
        Some((true, Some(session.as_str()), 10))
    );
    assert!(!format!("{report:?}").contains(&session));
    assert!(asked(&transport).contains(&(
        Method::Post,
        "/QuickConnect/Authorize?code=123456&userId=a7f3".to_owned()
    )));
    let as_the_phone = transport
        .requests()
        .into_iter()
        .filter(|request| {
            request.url.ends_with("/QuickConnect/Initiate")
                || request.url.ends_with("/Users/AuthenticateWithQuickConnect")
        })
        .filter(|request| {
            request.headers.iter().any(|(name, value)| {
                name == "Authorization"
                    && value.contains(r#"Client="lemonfiber-player""#)
                    && value.contains(r#"DeviceId="a-phone-0123""#)
                    && !value.contains("Token=")
            })
        })
        .count();
    assert_eq!(as_the_phone, 2);
}

#[tokio::test]
async fn a_rehearsed_grant_opens_nothing_and_answers_no_token() {
    let transport = signing_in(opened(&session()));
    let mut rehearsing = ctx_over(Arc::clone(&transport), "grant-rehearsed");
    rehearsing.dry_run = true;
    let rehearsed = viewed(&rehearsing, for_the_phone()).await;
    assert!(matches!(rehearsed, Ok(Outcome::Granted(report))
        if !report.granted && report.rehearsed && report.token.is_none()));
    assert!(!asked(&transport)
        .iter()
        .any(|(method, _)| *method == Method::Post));
}

#[tokio::test]
async fn a_device_named_by_something_else_is_refused_before_anything_is_asked() {
    let transport = signing_in("{}".to_owned());
    let ctx = ctx_over(Arc::clone(&transport), "grant-not-a-device");
    let refused = viewed(
        &ctx,
        Viewing::Grant {
            member: "Ada".to_owned(),
            device: "a\"".to_owned(),
        },
    )
    .await;
    assert_eq!(code(refused).as_deref(), Some("PLAY-3"));
    assert!(transport.requests().is_empty());
}

#[tokio::test]
async fn a_server_that_signs_no_device_in_by_code_is_said() {
    let ctx = ctx_over(
        server(vec![("/QuickConnect/Initiate", Answer::reply(401, ""))]),
        "grant-off",
    );
    let refused = viewed(&ctx, for_the_phone()).await;
    assert_eq!(code(refused).as_deref(), Some("PLAY-8"));
}

#[tokio::test]
async fn a_code_left_unauthorised_or_a_session_without_a_token_is_unanswered() {
    let unauthorised = ctx_over(
        server(vec![
            (
                "/QuickConnect/Initiate",
                Answer::reply(200, r#"{"Code":"123456","Secret":"the-secret"}"#),
            ),
            ("/QuickConnect/Authorize", Answer::reply(200, "false")),
        ]),
        "grant-unauthorised",
    );
    let refused = viewed(&unauthorised, for_the_phone()).await;
    assert_eq!(code(refused).as_deref(), Some("PLAY-5"));
    let empty = ctx_over(signing_in(opened("")), "grant-empty");
    let refused = viewed(&empty, for_the_phone()).await;
    assert_eq!(code(refused).as_deref(), Some("PLAY-5"));
}

#[tokio::test]
async fn progress_is_handed_to_the_server_and_a_silent_server_is_said() {
    let transport = server(vec![("/UserItems/", Answer::reply(200, "{}"))]);
    let ctx = ctx_over(Arc::clone(&transport), "watched");
    let went = HowFar {
        position: 90,
        ended: false,
    };
    let recorded = viewed(
        &ctx,
        Viewing::Watched {
            member: "Ada".to_owned(),
            id: FILM.to_owned(),
            how_far: went,
        },
    )
    .await;
    assert!(matches!(recorded, Ok(Outcome::Watched(report)) if report.position == 90));
    assert!(asked(&transport).contains(&(
        Method::Post,
        format!("/UserItems/{FILM}/UserData?userId=a7f3")
    )));

    let ctx = ctx_over(
        server(vec![("/UserItems/", Answer::reply(500, ""))]),
        "watched-silent",
    );
    let silent = viewed(
        &ctx,
        Viewing::Watched {
            member: "Ada".to_owned(),
            id: FILM.to_owned(),
            how_far: went,
        },
    )
    .await;
    assert_eq!(code(silent).as_deref(), Some("PLAY-5"));
}

#[tokio::test]
async fn a_lapsed_grant_signs_out_only_the_devices_it_granted() {
    let sessions = r#"[
        {"UserId":"a7f3","DeviceId":"phone","DeviceName":"Phone","Client":"lemonfiber-player"},
        {"UserId":"a7f3","DeviceId":"web-1","DeviceName":"lemonfiber","Client":"lemonfiber"},
        {"UserId":"a7f3","DeviceId":"tv","DeviceName":"TV","Client":"Jellyfin Android TV"}]"#;
    let transport = server(vec![
        ("/Sessions", Answer::reply(200, sessions)),
        ("/Devices", Answer::reply(204, "")),
    ]);
    let ctx = ctx_over(Arc::clone(&transport), "lapsed");
    let kept = crate::app::targets::beside_env(&ctx, "grants.json");
    if let Some(kept) = &kept {
        let _ = std::fs::write(kept, r#"{"a7f3":"2020-01-01"}"#);
    }
    lapsed(&ctx).await;
    let deleted: Vec<String> = asked(&transport)
        .into_iter()
        .filter(|(method, _)| *method == Method::Delete)
        .map(|(_, path)| path)
        .collect();
    assert_eq!(deleted, vec!["/Devices?id=phone".to_owned()]);
    let left = kept.and_then(|kept| std::fs::read_to_string(kept).ok());
    assert_eq!(left.as_deref().map(str::trim), Some("{}"));
}

/// A lapsed grant whose devices the server will not list is kept, so the next run tries
/// again rather than forgetting a session that may still be open.
#[tokio::test]
async fn a_lapsed_grant_the_server_will_not_list_is_kept() {
    let transport = server(vec![("/Sessions", Answer::reply(500, ""))]);
    let ctx = ctx_over(Arc::clone(&transport), "lapsed-unlisted");
    let kept = crate::app::targets::beside_env(&ctx, "grants.json");
    let lapsed_on = r#"{"a7f3":"2020-01-01"}"#;
    if let Some(kept) = &kept {
        let _ = std::fs::write(kept, lapsed_on);
    }
    lapsed(&ctx).await;
    assert!(!asked(&transport)
        .iter()
        .any(|(method, _)| *method == Method::Delete));
    let left = kept.and_then(|kept| std::fs::read_to_string(kept).ok());
    assert_eq!(left.as_deref(), Some(lapsed_on));
}

/// A title asked for a stranger, a title the server will not say, and a part-way list
/// for a stranger are each refused by name.
#[tokio::test]
async fn a_stranger_and_a_silent_title_are_refused_by_name() {
    let ctx = ctx_over(
        server(vec![("/Items/", Answer::reply(500, ""))]),
        "title-silent",
    );
    let for_whom = |member: Whom| Viewing::Title {
        member,
        id: FILM.to_owned(),
    };
    let stranger = viewed(&ctx, for_whom(Whom::Named("Zed".to_owned()))).await;
    assert_eq!(code(stranger).as_deref(), Some("PLAY-7"));
    let silent = viewed(&ctx, for_whom(Whom::Defaults)).await;
    assert_eq!(code(silent).as_deref(), Some("PLAY-5"));
    let part_way = viewed(
        &ctx,
        Viewing::PartWay {
            member: Whom::Named("Zed".to_owned()),
            most: 3,
        },
    )
    .await;
    assert_eq!(code(part_way).as_deref(), Some("PLAY-7"));
}

/// Progress about something that is not a title is refused before anything is asked,
/// and a title or progress with no media server says there is nothing to play from.
#[tokio::test]
async fn progress_for_something_else_or_with_no_server_is_refused() {
    let transport = server(Vec::new());
    let ctx = ctx_over(Arc::clone(&transport), "watched-not-an-item");
    let went = HowFar {
        position: 1,
        ended: false,
    };
    let progress = |id: &str| Viewing::Watched {
        member: "Ada".to_owned(),
        id: id.to_owned(),
        how_far: went,
    };
    let refused = viewed(&ctx, progress("../Users")).await;
    assert_eq!(code(refused).as_deref(), Some("PLAY-1"));
    assert!(transport.requests().is_empty());

    let unset = a_context().build();
    let nothing = play::NOTHING_TO_PLAY_FROM.to_string();
    let no_progress = viewed(&unset, progress(FILM)).await;
    assert_eq!(code(no_progress).as_deref(), Some(nothing.as_str()));
    let no_title = viewed(
        &unset,
        Viewing::Title {
            member: Whom::Defaults,
            id: FILM.to_owned(),
        },
    )
    .await;
    assert_eq!(code(no_title).as_deref(), Some(nothing.as_str()));
}

/// A stack nothing can read is said as that, rather than as there being no server.
#[tokio::test]
async fn a_stack_that_cannot_be_read_is_said_rather_than_taken_for_no_server() {
    let ctx = a_context().over(crate::test_support::nowhere()).build();
    let read = viewed(
        &ctx,
        Viewing::Title {
            member: Whom::Defaults,
            id: FILM.to_owned(),
        },
    )
    .await;
    let said = code(read);
    assert!(
        said.is_some(),
        "a stack nothing could read said what it holds"
    );
    assert_ne!(said, Some(play::NOTHING_TO_PLAY_FROM.to_string()));
}

/// A rehearsal records no progress and renews no grant.
#[tokio::test]
async fn a_rehearsal_records_no_progress_and_renews_no_grant() {
    let transport = server(vec![("/UserItems/", Answer::reply(200, "{}"))]);
    let mut ctx = ctx_over(Arc::clone(&transport), "watched-rehearsed");
    ctx.dry_run = true;
    let rehearsed = viewed(
        &ctx,
        Viewing::Watched {
            member: "Ada".to_owned(),
            id: FILM.to_owned(),
            how_far: HowFar {
                position: 30,
                ended: true,
            },
        },
    )
    .await;
    assert!(matches!(rehearsed, Ok(Outcome::Watched(report)) if report.rehearsed && report.ended));
    assert!(!asked(&transport)
        .iter()
        .any(|(method, _)| *method == Method::Post));
    super::spoke(&ctx, "a7f3");
    let kept = crate::app::targets::beside_env(&ctx, "grants.json");
    assert!(kept.is_some_and(|kept| !kept.exists()));
}

/// A household the server will not list is said as the server not answering.
#[tokio::test]
async fn a_household_the_server_will_not_list_is_unanswered() {
    let transport = Transport::by_path(vec![
        (
            "/Users/AuthenticateByName",
            Answer::reply(200, r#"{"AccessToken":"token"}"#),
        ),
        ("/Users", Answer::reply(500, "")),
    ]);
    let ctx = ctx_over(transport, "household-silent");
    let refused = viewed(&ctx, for_the_phone()).await;
    assert_eq!(code(refused).as_deref(), Some("PLAY-5"));
}

/// A series is answered with its seasons, and every episode in them is located at the
/// door like the series itself.
#[tokio::test]
async fn every_episode_of_a_series_is_located_like_the_series() {
    let series = r#"{"Id":"0123456789abcdef0123456789abcdef","Name":"Fargo","Type":"Series",
        "IsFolder":true}"#;
    let seasons = r#"{"Items":[{"Id":"s1","Name":"Season 1","Type":"Season","IndexNumber":1}]}"#;
    let episodes = r#"{"Items":[{"Id":"e1","Name":"Pilot","Type":"Episode","IndexNumber":1,
        "SeasonId":"s1"}]}"#;
    let ctx = ctx_over(
        server(vec![
            ("/Seasons", Answer::reply(200, seasons)),
            ("/Episodes", Answer::reply(200, episodes)),
            ("/Items/", Answer::reply(200, series)),
        ]),
        "series",
    );
    let read = viewed(
        &ctx,
        Viewing::Title {
            member: Whom::Defaults,
            id: FILM.to_owned(),
        },
    )
    .await;
    let title = match read {
        Ok(Outcome::Title(report)) => report.title,
        _ => None,
    };
    let episodes: Vec<_> = title
        .iter()
        .flat_map(|title| &title.seasons)
        .flat_map(|season| &season.episodes)
        .collect();
    assert_eq!(episodes.len(), 1);
    assert!(episodes
        .iter()
        .all(|episode| episode.held.at.unlocated.is_some()));
}
