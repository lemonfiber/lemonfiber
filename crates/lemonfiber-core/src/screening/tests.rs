use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake as Transport};

use super::{a_device, an_item, lapsed, picture, viewed, Pictured, PICTURE_MOST, RASTER};
use crate::app::{Ctx, Outcome, Viewing, Whom};
use crate::error::codes::play;
use crate::ports::service::{
    EpisodeDetail, Holds, HowFar, Image, Item, ItemDetail, ItemProgress, Medium, Picture,
    SeasonDetail, Session, PLAYER,
};
use crate::test_support::{a_context, media, moved_household};

/// A title on the shelf, by an identifier shaped like the server's.
const FILM: &str = "0123456789abcdef0123456789abcdef";

/// A device, by an id shaped like the one a player keeps.
const PHONE: &str = "a-phone-0123";

/// The token the media server opens a device's session with: thirty-two hex digits,
/// built rather than written, so nothing reading the source takes it for a key.
fn session() -> String {
    "fe".repeat(16)
}

/// The media server, answering each operation by what it asks for, holding Ada.
fn server(routes: Vec<(&'static str, Answer)>) -> Arc<Transport> {
    let mut answered = routes;
    answered.push((
        media::identity("household"),
        media::answered(&[media::member("a7f3", "Ada", true)]),
    ));
    Transport::by_path(answered)
}

/// A context whose media server has moved into its plugin, kept apart from every other
/// case.
fn ctx_over(transport: Arc<Transport>, tag: &str) -> Ctx {
    moved_household(&format!("screening-{tag}"), transport)
}

/// What the media server was asked, as each operation and what it was asked with.
fn asked(transport: &Transport) -> Vec<(String, serde_json::Value)> {
    transport
        .requests()
        .into_iter()
        .map(|request| {
            let operation = request
                .url
                .rsplit('/')
                .next()
                .unwrap_or_default()
                .to_owned();
            let with = request
                .body
                .as_deref()
                .and_then(|body| serde_json::from_str(body).ok())
                .unwrap_or_default();
            (operation, with)
        })
        .collect()
}

/// A title called `title` under `id`, holding no picture and nothing to play.
fn called(id: &str, title: &str, medium: Medium) -> Item {
    Item {
        id: id.to_owned(),
        title: title.to_owned(),
        year: None,
        medium,
        holds: Holds::default(),
    }
}

/// `item` as the media server details it, with `seasons`.
fn detailed(item: Item, seasons: Vec<SeasonDetail>) -> ItemDetail {
    ItemDetail {
        item,
        overview: None,
        minutes: None,
        genres: Vec::new(),
        certificate: None,
        released: None,
        seasons,
    }
}

/// The film on the shelf, as the media server details it for a member.
fn on_the_shelf() -> ItemDetail {
    detailed(called(FILM, "Heat", Medium::Film), Vec::new())
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
    let transport = server(vec![(
        media::serve("title"),
        media::answered(&on_the_shelf()),
    )]);
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
    assert!(asked(&transport).contains(&(
        "title".to_owned(),
        serde_json::json!({ "member": "a7f3", "id": FILM })
    )));
}

#[tokio::test]
async fn a_title_off_the_shelf_and_a_stranger_are_refused_by_name() {
    let transport = server(vec![(
        media::serve("title"),
        media::answered(&None::<ItemDetail>),
    )]);
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
    let resume = vec![ItemProgress {
        item: called("e2", "The Detail", Medium::Episode),
        position: 1200,
        length: Some(3600),
    }];
    let ctx = ctx_over(
        server(vec![(media::serve("part_way"), media::answered(&resume))]),
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
        server(vec![(media::serve("part_way"), media::unavailable())]),
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

/// The media server, signing a device in and answering `token` for its session.
fn signing_in(token: Option<&str>) -> Arc<Transport> {
    server(vec![(media::serve("signed_in"), media::answered(&token))])
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
    let transport = signing_in(Some(&session));
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
        "signed_in".to_owned(),
        serde_json::json!({ "member": "a7f3", "device": PHONE })
    )));
}

#[tokio::test]
async fn a_rehearsed_grant_opens_nothing_and_answers_no_token() {
    let transport = signing_in(Some(&session()));
    let mut rehearsing = ctx_over(Arc::clone(&transport), "grant-rehearsed");
    rehearsing.dry_run = true;
    let rehearsed = viewed(&rehearsing, for_the_phone()).await;
    assert!(matches!(rehearsed, Ok(Outcome::Granted(report))
        if !report.granted && report.rehearsed && report.token.is_none()));
    assert!(!asked(&transport)
        .iter()
        .any(|(operation, _)| operation == "signed_in"));
}

#[tokio::test]
async fn a_device_named_by_something_else_is_refused_before_anything_is_asked() {
    let transport = signing_in(None);
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
    let ctx = ctx_over(signing_in(None), "grant-off");
    let refused = viewed(&ctx, for_the_phone()).await;
    assert_eq!(code(refused).as_deref(), Some("PLAY-8"));
}

#[tokio::test]
async fn a_server_that_will_not_sign_the_device_in_is_unanswered() {
    let refusing = ctx_over(
        server(vec![(media::serve("signed_in"), media::unavailable())]),
        "grant-unanswered",
    );
    let refused = viewed(&refusing, for_the_phone()).await;
    assert_eq!(code(refused).as_deref(), Some("PLAY-5"));
}

#[tokio::test]
async fn progress_is_handed_to_the_server_and_a_silent_server_is_said() {
    let transport = server(vec![(media::serve("progressed"), media::taken())]);
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
        "progressed".to_owned(),
        serde_json::json!({
            "member": "a7f3",
            "id": FILM,
            "how_far": { "position": 90, "ended": false }
        })
    )));

    let ctx = ctx_over(
        server(vec![(media::serve("progressed"), media::unavailable())]),
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
    let device = |id: &str, name: &str, client: &str| Session {
        device_id: id.to_owned(),
        device: name.to_owned(),
        client: client.to_owned(),
        last_seen: None,
    };
    let sessions = vec![
        device("phone", "Phone", PLAYER),
        device("web-1", "lemonfiber", "lemonfiber"),
        device("tv", "TV", "Jellyfin Android TV"),
    ];
    let transport = server(vec![
        (media::identity("sessions"), media::answered(&sessions)),
        (media::serve("sign_out"), media::taken()),
    ]);
    let ctx = ctx_over(Arc::clone(&transport), "lapsed");
    let kept = crate::app::targets::beside_env(&ctx, "grants.json");
    if let Some(kept) = &kept {
        let _ = std::fs::write(kept, r#"{"a7f3":"2020-01-01"}"#);
    }
    lapsed(&ctx).await;
    let signed_out: Vec<serde_json::Value> = asked(&transport)
        .into_iter()
        .filter(|(operation, _)| operation == "sign_out")
        .map(|(_, with)| with)
        .collect();
    assert_eq!(signed_out, vec![serde_json::json!({ "device": "phone" })]);
    let left = kept.and_then(|kept| std::fs::read_to_string(kept).ok());
    assert_eq!(left.as_deref().map(str::trim), Some("{}"));
}

/// A lapsed grant whose devices the server will not list is kept, so the next run tries
/// again rather than forgetting a session that may still be open.
#[tokio::test]
async fn a_lapsed_grant_the_server_will_not_list_is_kept() {
    let transport = server(vec![(media::identity("sessions"), media::unavailable())]);
    let ctx = ctx_over(Arc::clone(&transport), "lapsed-unlisted");
    let kept = crate::app::targets::beside_env(&ctx, "grants.json");
    let lapsed_on = r#"{"a7f3":"2020-01-01"}"#;
    if let Some(kept) = &kept {
        let _ = std::fs::write(kept, lapsed_on);
    }
    lapsed(&ctx).await;
    assert!(!asked(&transport)
        .iter()
        .any(|(operation, _)| operation == "sign_out"));
    let left = kept.and_then(|kept| std::fs::read_to_string(kept).ok());
    assert_eq!(left.as_deref(), Some(lapsed_on));
}

/// A title asked for a stranger, a title the server will not say, and a part-way list
/// for a stranger are each refused by name.
#[tokio::test]
async fn a_stranger_and_a_silent_title_are_refused_by_name() {
    let ctx = ctx_over(
        server(vec![(media::serve("title"), media::unavailable())]),
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
    let transport = server(vec![(media::serve("progressed"), media::taken())]);
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
        .any(|(operation, _)| operation == "progressed"));
    super::spoke(&ctx, "a7f3");
    let kept = crate::app::targets::beside_env(&ctx, "grants.json");
    assert!(kept.is_some_and(|kept| !kept.exists()));
}

/// A household the server will not list is said as the server not answering.
#[tokio::test]
async fn a_household_the_server_will_not_list_is_unanswered() {
    let transport = Transport::by_path(vec![(media::identity("household"), media::unavailable())]);
    let ctx = ctx_over(transport, "household-silent");
    let refused = viewed(&ctx, for_the_phone()).await;
    assert_eq!(code(refused).as_deref(), Some("PLAY-5"));
}

/// A series is answered with its seasons, and every episode in them is located at the
/// door like the series itself.
#[tokio::test]
async fn every_episode_of_a_series_is_located_like_the_series() {
    let series = detailed(
        called(FILM, "Fargo", Medium::Series),
        vec![SeasonDetail {
            id: "s1".to_owned(),
            name: "Season 1".to_owned(),
            number: Some(1),
            episodes: vec![EpisodeDetail {
                item: called("e1", "Pilot", Medium::Episode),
                number: Some(1),
                overview: None,
                minutes: None,
            }],
        }],
    );
    let ctx = ctx_over(
        server(vec![(
            media::serve("title"),
            media::answered(&Some(series)),
        )]),
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

/// A picture of `media_type` holding `bytes`, as the media server answers one.
fn a_picture(media_type: &str, bytes: &[u8]) -> Answer {
    media::answered(&Some(Image {
        media_type: media_type.to_owned(),
        bytes: bytes.to_vec(),
    }))
}

/// A backdrop's bytes, opening as a JPEG does.
const BACKDROP: &[u8] = b"\xFF\xD8\xFF\xE0the-backdrop";

/// A poster's bytes, opening as a PNG does.
const POSTER: &[u8] = b"\x89PNG\r\n\x1a\nthe-poster";

/// A backdrop asked for as Ada, with the media server answering `answer`, and the code it
/// was refused with.
async fn pictured(tag: &str, answer: Answer) -> Result<Pictured, String> {
    let transport = server(vec![(media::serve("picture"), answer)]);
    let ctx = ctx_over(transport, &format!("picture-{tag}"));
    picture(
        &ctx,
        &Whom::Named("ada".to_owned()),
        FILM,
        Picture::Backdrop,
    )
    .await
    .map_err(|problem| problem.code.to_string())
}

#[tokio::test]
async fn a_picture_is_asked_as_the_member_and_passed_on_as_a_raster() {
    let transport = server(vec![(
        media::serve("picture"),
        a_picture("IMAGE/JPEG; q=1", BACKDROP),
    )]);
    let ctx = ctx_over(Arc::clone(&transport), "picture");
    let shown = picture(
        &ctx,
        &Whom::Named("ada".to_owned()),
        FILM,
        Picture::Backdrop,
    )
    .await;
    assert_eq!(
        shown.ok(),
        Some(Pictured {
            media_type: "image/jpeg",
            bytes: BACKDROP.to_vec(),
        })
    );
    assert!(asked(&transport).contains(&(
        "picture".to_owned(),
        serde_json::json!({ "member": "a7f3", "id": FILM, "which": "backdrop" })
    )));
}

#[tokio::test]
async fn a_picture_off_the_shelf_missing_or_not_a_raster_is_absent() {
    for (tag, answer) in [
        ("none", media::answered(&None::<Image>)),
        ("vector", a_picture("image/svg+xml", b"<svg/>")),
        (
            "vector-as-raster",
            a_picture("image/png", b"<svg onload=\"alert(1)\"/>"),
        ),
        ("jpeg-as-png", a_picture("image/png", BACKDROP)),
        ("untyped", a_picture("", b"x")),
        (
            "oversized",
            a_picture("image/png", &vec![b'x'; PICTURE_MOST + 1]),
        ),
    ] {
        assert_eq!(
            pictured(tag, answer).await.err().as_deref(),
            Some("PLAY-9"),
            "{tag}"
        );
    }
}

#[tokio::test]
async fn a_picture_the_server_will_not_answer_is_unanswered_and_a_bad_id_is_never_asked() {
    for (tag, answer) in [
        ("silent", Answer::Silent),
        ("failing", media::unavailable()),
    ] {
        assert_eq!(
            pictured(tag, answer).await.err().as_deref(),
            Some("PLAY-5"),
            "{tag}"
        );
    }
    let transport = server(Vec::new());
    let ctx = ctx_over(Arc::clone(&transport), "picture-not-an-item");
    let refused = picture(&ctx, &Whom::Defaults, "../System", Picture::Poster).await;
    assert_eq!(
        refused
            .err()
            .map(|problem| problem.code.to_string())
            .as_deref(),
        Some("PLAY-1")
    );
    assert!(transport.requests().is_empty());
}

#[tokio::test]
async fn the_households_defaults_are_shown_a_picture_without_naming_a_member() {
    let transport = server(vec![(
        media::serve("picture"),
        a_picture("image/png", POSTER),
    )]);
    let ctx = ctx_over(Arc::clone(&transport), "picture-defaults");
    let shown = picture(&ctx, &Whom::Defaults, FILM, Picture::Poster).await;
    assert_eq!(
        shown.ok(),
        Some(Pictured {
            media_type: "image/png",
            bytes: POSTER.to_vec(),
        })
    );
    assert_eq!(
        asked(&transport),
        vec![(
            "picture".to_owned(),
            serde_json::json!({ "member": null, "id": FILM, "which": "poster" })
        )]
    );
}

#[test]
fn each_raster_opens_only_as_its_own_image_and_never_as_a_drawing() {
    let opening: [(&str, &[u8]); 7] = [
        ("image/jpeg", b"\xFF\xD8\xFF\xDB"),
        ("image/png", b"\x89PNG\r\n\x1a\n"),
        ("image/webp", b"RIFF\0\0\0\0WEBPVP8 "),
        ("image/gif", b"GIF87a"),
        ("image/gif", b"GIF89a"),
        ("image/avif", b"\0\0\0\x1cftypavif"),
        ("image/avif", b"\0\0\0\x1cftypavis"),
    ];
    for raster in RASTER {
        for (media_type, bytes) in opening {
            assert_eq!(
                (raster.opens)(bytes),
                raster.media_type == media_type,
                "{} {bytes:?}",
                raster.media_type
            );
        }
        assert!(!(raster.opens)(
            b"<svg xmlns=\"http://www.w3.org/2000/svg\" onload=\"alert(1)\"/>"
        ));
    }
}
