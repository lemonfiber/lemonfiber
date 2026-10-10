//! How the series the media server holds are filed, and reading one afresh, answered over
//! `media.serve` from the media server's recorded answers.

use super::{reader, SIGNED_IN};
use lemonfiber_contract::capabilities::media::serve;
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::http::Method;
use lemonfiber_ports::service::{Failure, SeriesHeld, Upkeep};

const BROKEN: &str = "0a1b2c3d4e5f60718293a4b5c6d7e8f9";

const SERIES: &str = r#"{"Items":[
    {"Id":"0a1b2c3d4e5f60718293a4b5c6d7e8f9","Name":"The Expanse","Type":"Series",
     "IsFolder":true,"ChildCount":0,"RecursiveItemCount":0},
    {"Id":"9f8e7d6c5b4a39281706f5e4d3c2b1a0","Name":"Severance","Type":"Series",
     "IsFolder":true,"ChildCount":2,"RecursiveItemCount":19}
],"TotalRecordCount":2,"StartIndex":0}"#;

const FILED_UNDER: &str = r#"{"Items":[],"TotalRecordCount":12,"StartIndex":0}"#;

fn asked(fake: &Fake) -> Vec<(Method, String)> {
    fake.requests()
        .into_iter()
        .skip(1)
        .map(|request| (request.method, request.url))
        .collect()
}

#[tokio::test]
async fn a_series_whose_seasons_lost_their_key_answers_its_episodes_by_where_they_are_filed() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, SERIES),
        Answer::reply(200, FILED_UNDER),
    ]);
    let answered = serve::dispatch(&reader(&fake), "series_held", br#"{"most":50}"#).await;
    let held: Option<Vec<SeriesHeld>> = answered
        .ok()
        .and_then(|body| serde_json::from_slice(&body).ok());
    assert_eq!(
        held,
        Some(vec![
            SeriesHeld {
                id: BROKEN.to_owned(),
                title: "The Expanse".to_owned(),
                seasons: 0,
                episodes: 12,
            },
            SeriesHeld {
                id: "9f8e7d6c5b4a39281706f5e4d3c2b1a0".to_owned(),
                title: "Severance".to_owned(),
                seasons: 2,
                episodes: 19,
            },
        ])
    );
    assert_eq!(
        asked(&fake),
        vec![
            (
                Method::Get,
                "http://127.0.0.1:8096/Items?Recursive=true&IncludeItemTypes=Series\
                 &Fields=ChildCount%2CRecursiveItemCount&SortBy=SortName&Limit=50"
                    .to_owned()
            ),
            (
                Method::Get,
                format!(
                    "http://127.0.0.1:8096/Items?ParentId={BROKEN}&Recursive=true\
                     &IncludeItemTypes=Episode&IsMissing=false&Limit=0"
                )
            ),
        ]
    );
}

#[tokio::test]
async fn a_listing_without_the_seasons_counted_is_refused_rather_than_read_as_none() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(
            200,
            r#"{"Items":[{"Id":"a","Name":"Dark","Type":"Series"}]}"#,
        ),
    ]);
    assert!(matches!(
        reader(&fake).series_held(50).await,
        Err(Failure::Refused { ref detail, .. }) if detail.contains("ChildCount")
    ));
}

#[tokio::test]
async fn a_count_the_media_server_will_not_give_is_refused() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(200, SERIES),
        Answer::reply(200, r#"{"Items":[]}"#),
    ]);
    assert!(matches!(
        reader(&fake).series_held(50).await,
        Err(Failure::Refused { ref detail, .. }) if detail.contains("could not be counted")
    ));
}

#[tokio::test]
async fn a_series_with_seasons_missing_its_episode_count_is_read_as_holding_none() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, SIGNED_IN),
        Answer::reply(
            200,
            r#"{"Items":[{"Id":"a","Name":"Dark","Type":"Series","ChildCount":3}]}"#,
        ),
    ]);
    let held = reader(&fake).series_held(50).await.ok();
    assert_eq!(
        held.map(|held| held
            .iter()
            .map(|one| (one.seasons, one.episodes))
            .collect::<Vec<_>>()),
        Some(vec![(3, 0)])
    );
}

#[tokio::test]
async fn a_refresh_reads_the_series_and_everything_under_it_again_and_keeps_edits() {
    let fake = Fake::in_turn(vec![Answer::reply(200, SIGNED_IN), Answer::reply(204, "")]);
    let answered = serve::dispatch(
        &reader(&fake),
        "refresh",
        format!(r#"{{"id":"{BROKEN}"}}"#).as_bytes(),
    )
    .await;
    assert_eq!(answered.ok().as_deref(), Some(&b"null"[..]));
    assert_eq!(
        asked(&fake),
        vec![(
            Method::Post,
            format!(
                "http://127.0.0.1:8096/Items/{BROKEN}/Refresh?Recursive=true\
                 &MetadataRefreshMode=FullRefresh&ImageRefreshMode=Default\
                 &ReplaceAllMetadata=false&ReplaceAllImages=false"
            )
        )]
    );
}

#[tokio::test]
async fn a_refresh_naming_nothing_the_media_server_files_is_refused_before_it_is_sent() {
    let fake = Fake::silent();
    assert!(matches!(
        reader(&fake).refresh("../Library/Refresh").await,
        Err(Failure::Refused { .. })
    ));
    assert!(fake.requests().is_empty());
}

#[tokio::test]
async fn a_refresh_the_media_server_refuses_is_a_failure() {
    let fake = Fake::in_turn(vec![Answer::reply(200, SIGNED_IN), Answer::reply(404, "")]);
    assert!(reader(&fake).refresh(BROKEN).await.is_err());
}
