//! The music quality choice written to a service's profiles.

use crate::{lidarr, sonarr, PROFILES};
use lemonfiber_core::audio::Format;
use lemonfiber_core::ports::http::{Method, Request};
use lemonfiber_core::ports::service::RootFolder;
use lemonfiber_fixtures::http::{Answer, Fake};

#[tokio::test]
async fn a_lossless_choice_updates_each_addressable_profile() {
    let router = Fake::by_route(vec![
        (
            Method::Get,
            "/qualityprofile",
            Answer::reply(200, PROFILES.to_owned()),
        ),
        (
            Method::Put,
            "/qualityprofile",
            Answer::reply(200, String::new()),
        ),
    ]);
    let applied = lidarr(&router).apply_music_format(Format::Lossless).await;
    assert!(applied.is_ok());

    // Only the profile that carries an id is addressed — the stray and the id-less one
    // are passed over rather than sent nowhere.
    let puts: Vec<Request> = router
        .requests()
        .into_iter()
        .filter(|request| request.method == Method::Put)
        .collect();
    assert_eq!(puts.len(), 1);
    let put = puts.first();
    assert!(put.is_some_and(|request| request.url.ends_with("/api/v1/qualityprofile/2")));
    assert!(put
        .and_then(|request| request.body.as_deref())
        .is_some_and(|body| body.contains(r#""upgradeAllowed":true"#)));

    // A non-hi-res choice touches no custom format.
    assert!(!router
        .requests()
        .iter()
        .any(|request| request.url.contains("/customformat")));
}

#[tokio::test]
async fn a_compact_choice_updates_the_profile_without_touching_a_custom_format() {
    let router = Fake::by_route(vec![
        (
            Method::Get,
            "/qualityprofile",
            Answer::reply(200, PROFILES.to_owned()),
        ),
        (
            Method::Put,
            "/qualityprofile",
            Answer::reply(200, String::new()),
        ),
    ]);
    let applied = lidarr(&router).apply_music_format(Format::Compact).await;
    assert!(applied.is_ok());
    // Compact addresses the profile but, like every non-hi-res choice, leaves custom
    // formats alone.
    assert!(router
        .requests()
        .iter()
        .any(|request| request.method == Method::Put));
    assert!(!router
        .requests()
        .iter()
        .any(|request| request.url.contains("/customformat")));
}

#[tokio::test]
async fn a_hi_res_choice_creates_the_24_bit_format_and_prefers_it() {
    let router = Fake::by_route(vec![
        (
            Method::Get,
            "/customformat",
            Answer::reply(200, "[]".to_owned()),
        ),
        (
            Method::Post,
            "/customformat",
            Answer::reply(201, String::new()),
        ),
        (
            Method::Get,
            "/qualityprofile",
            Answer::reply(200, PROFILES.to_owned()),
        ),
        (
            Method::Put,
            "/qualityprofile",
            Answer::reply(200, String::new()),
        ),
    ]);
    let applied = lidarr(&router).apply_music_format(Format::HiRes).await;
    assert!(applied.is_ok());

    // The 24-bit format is created, as a release-title match.
    let posts: Vec<Request> = router
        .requests()
        .into_iter()
        .filter(|request| request.method == Method::Post && request.url.contains("/customformat"))
        .collect();
    assert_eq!(posts.len(), 1);
    assert!(posts
        .first()
        .and_then(|request| request.body.as_deref())
        .is_some_and(|body| body.contains("ReleaseTitleSpecification")));

    // The profile update prefers it, through a positive cutoff format score.
    let put = router
        .requests()
        .into_iter()
        .find(|request| request.method == Method::Put);
    assert!(put
        .and_then(|request| request.body)
        .is_some_and(|body| body.contains(r#""cutoffFormatScore":100"#)));
}

#[tokio::test]
async fn an_existing_24_bit_format_is_not_created_again() {
    let router = Fake::by_route(vec![
        (
            Method::Get,
            "/customformat",
            Answer::reply(200, r#"[{"id":9,"name":"lemonfiber: 24-bit"}]"#.to_owned()),
        ),
        (
            Method::Get,
            "/qualityprofile",
            Answer::reply(200, PROFILES.to_owned()),
        ),
        (
            Method::Put,
            "/qualityprofile",
            Answer::reply(200, String::new()),
        ),
    ]);
    let applied = lidarr(&router).apply_music_format(Format::HiRes).await;
    assert!(applied.is_ok());
    assert!(
        !router
            .requests()
            .iter()
            .any(|request| request.method == Method::Post),
        "the format already existed, so it was not created again"
    );
}

#[tokio::test]
async fn an_unreadable_profile_list_is_a_failure() {
    let router = Fake::by_route(vec![(
        Method::Get,
        "/qualityprofile",
        Answer::reply(200, "not json".to_owned()),
    )]);
    assert!(lidarr(&router)
        .apply_music_format(Format::Lossless)
        .await
        .is_err());
}

#[tokio::test]
async fn a_refused_profile_update_is_a_failure() {
    let router = Fake::by_route(vec![
        (
            Method::Get,
            "/qualityprofile",
            Answer::reply(200, PROFILES.to_owned()),
        ),
        (
            Method::Put,
            "/qualityprofile",
            Answer::reply(500, "boom".to_owned()),
        ),
    ]);
    assert!(lidarr(&router)
        .apply_music_format(Format::Lossless)
        .await
        .is_err());
}

#[tokio::test]
async fn an_unreadable_custom_format_list_is_a_failure() {
    let router = Fake::by_route(vec![(
        Method::Get,
        "/customformat",
        Answer::reply(200, "not json".to_owned()),
    )]);
    assert!(lidarr(&router)
        .apply_music_format(Format::HiRes)
        .await
        .is_err());
}

#[tokio::test]
async fn a_refused_custom_format_creation_is_a_failure() {
    let router = Fake::by_route(vec![
        (
            Method::Get,
            "/customformat",
            Answer::reply(200, "[]".to_owned()),
        ),
        (
            Method::Post,
            "/customformat",
            Answer::reply(500, "nope".to_owned()),
        ),
    ]);
    assert!(lidarr(&router)
        .apply_music_format(Format::HiRes)
        .await
        .is_err());
}

/// The \*arr that files by artist is given a root folder it will accept.
///
/// It refuses one described by path alone: it wants a name and the two profiles
/// anything found beneath the folder is fetched at. The ids are read from the service
/// rather than assumed, because they are numbered per installation.
#[tokio::test]
async fn a_music_root_folder_carries_the_name_and_profiles_that_service_requires() {
    let fake = Fake::by_path(vec![
        (
            "/metadataprofile",
            Answer::reply(200, r#"[{"id":7,"name":"Standard"}]"#),
        ),
        (
            "/qualityprofile",
            Answer::reply(200, r#"[{"id":4,"name":"Lossless"}]"#),
        ),
        ("/rootfolder", Answer::reply(201, "")),
    ]);
    let folder = RootFolder {
        path: "/data/media/music".to_owned(),
        media_type: "music".to_owned(),
    };
    assert!(lidarr(&fake).register_root_folder(&folder).await.is_ok());

    let body = fake
        .requests()
        .into_iter()
        .find(|request| request.url.contains("/rootfolder") && request.body.is_some())
        .and_then(|request| request.body)
        .unwrap_or_default();
    for expected in [
        "/data/media/music",
        "\"name\":\"music\"",
        "\"defaultQualityProfileId\":4",
        "\"defaultMetadataProfileId\":7",
    ] {
        assert!(body.contains(expected), "{expected} is missing from {body}");
    }
}

/// A service with no metadata profiles is given a path and nothing else.
///
/// Sending the music fields to one that does not file that way is not a field it
/// ignores — the extra profile ids name nothing it holds. Which service wants them is
/// asked rather than inferred from its name, so a service answering with no profiles
/// is one that neither has them nor wants them.
#[tokio::test]
async fn a_root_folder_for_a_service_without_metadata_profiles_carries_only_its_path() {
    let fake = Fake::by_path(vec![
        ("/metadataprofile", Answer::reply(404, "")),
        ("/rootfolder", Answer::reply(201, "")),
    ]);
    let folder = RootFolder {
        path: "/data/media/tv".to_owned(),
        media_type: "tv".to_owned(),
    };
    assert!(sonarr(&fake).register_root_folder(&folder).await.is_ok());

    let body = fake
        .requests()
        .into_iter()
        .find(|request| request.url.contains("/rootfolder") && request.body.is_some())
        .and_then(|request| request.body)
        .unwrap_or_default();
    assert!(body.contains("/data/media/tv"), "{body}");
    assert!(
        !body.contains("defaultMetadataProfileId") && !body.contains("defaultQualityProfileId"),
        "a service that files no music was sent the music fields: {body}"
    );
}
