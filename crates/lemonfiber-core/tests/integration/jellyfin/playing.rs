//! What the media server is playing now.

use super::{reader, SIGNED_IN};
use lemonfiber_core::ports::service::{Medium, Playback};
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::service::Household;

/// Three sessions: ana watching an episode, bo paused on a film, and a device of
/// ana's signed in and playing nothing.
const SESSIONS: &str = r#"[
    {"UserId":"a7f3","UserName":"ana","DeviceName":"Living room TV",
     "NowPlayingItem":{"Name":"Pilot","SeriesName":"A Series","ParentIndexNumber":1,
                       "IndexNumber":2,"Type":"Episode"},
     "PlayState":{"IsPaused":false}},
    {"UserId":"b2c9","UserName":"bo","DeviceName":"Phone",
     "NowPlayingItem":{"Name":"A Film","Type":"Movie"},
     "PlayState":{"IsPaused":true}},
    {"UserId":"a7f3","UserName":"ana","DeviceName":"Tablet"}
]"#;

/// Ana's episode, as the port carries it.
fn anas() -> Playback {
    Playback {
        member_id: "a7f3".to_owned(),
        member: "ana".to_owned(),
        title: "Pilot".to_owned(),
        series: Some("A Series".to_owned()),
        season: Some(1),
        episode: Some(2),
        medium: Medium::Series,
        paused: false,
        device: "Living room TV".to_owned(),
    }
}

/// Bo's film, as the port carries it.
fn bos() -> Playback {
    Playback {
        member_id: "b2c9".to_owned(),
        member: "bo".to_owned(),
        title: "A Film".to_owned(),
        series: None,
        season: None,
        episode: None,
        medium: Medium::Film,
        paused: true,
        device: "Phone".to_owned(),
    }
}

/// Every session playing something is listed, each as who, what and where; a device
/// signed in and playing nothing is not somebody watching.
#[tokio::test]
async fn every_session_playing_something_is_listed() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, SESSIONS),
    ]);
    let playing = reader(&fake).playing(None).await;
    assert_eq!(playing.ok(), Some(vec![anas(), bos()]));
    let url = fake
        .requests()
        .last()
        .map(|request| request.url.clone())
        .unwrap_or_default();
    assert!(
        url.ends_with("/Sessions"),
        "sessions were not asked for: {url}"
    );
}

/// Naming an account lists only what that account is playing.
#[tokio::test]
async fn naming_an_account_lists_only_what_it_is_playing() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, SESSIONS),
    ]);
    assert_eq!(
        reader(&fake).playing(Some("b2c9")).await.ok(),
        Some(vec![bos()])
    );
}

/// An account playing nothing, or not known to the server, lists nothing.
#[tokio::test]
async fn an_account_playing_nothing_lists_nothing() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, SESSIONS),
    ]);
    assert_eq!(
        reader(&fake).playing(Some("c0d4")).await.ok(),
        Some(Vec::new())
    );
}

/// A series itself and anything else the server plays keep their kind.
#[tokio::test]
async fn what_is_neither_a_film_nor_an_episode_is_other() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(
            200,
            r#"[{"UserId":"a7f3","UserName":"ana","DeviceName":"Speaker",
                 "NowPlayingItem":{"Name":"A Song","Type":"Audio"}},
                {"UserId":"a7f3","UserName":"ana","DeviceName":"TV",
                 "NowPlayingItem":{"Name":"A Series","Type":"Series"}}]"#,
        ),
    ]);
    let playing = reader(&fake).playing(None).await.unwrap_or_default();
    let kinds: Vec<Medium> = playing.iter().map(|one| one.medium).collect();
    assert_eq!(kinds, vec![Medium::Other, Medium::Series]);
    assert!(playing.iter().all(|one| !one.paused), "{playing:?}");
}

/// A server that will not answer is a failure, never a house where nobody watches.
#[tokio::test]
async fn a_server_that_will_not_answer_is_a_failure() {
    let fake = Fake::in_turn(vec![Answer::reply(200, SIGNED_IN), Answer::reply(500, "")]);
    assert!(reader(&fake).playing(None).await.is_err());
}
