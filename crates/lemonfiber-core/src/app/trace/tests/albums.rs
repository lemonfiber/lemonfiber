use std::sync::Arc;

use lemonfiber_fixtures::http::Fake as Transport;

use super::*;
use crate::model::StuckReport;
use crate::test_support::curators::{told, MUSIC};

/// The album the music curator holds.
fn album() -> FoundItem {
    FoundItem {
        id: 7,
        title: "Kind of Blue".to_owned(),
        year: Some(1959),
        monitored: true,
    }
}

/// A transport where the music curator alone holds the album, grabbed and waiting in its
/// queue, the media server holds it, and what the music curator says is stuck is `stuck`.
fn album_grabbed(stuck: &[StuckItem]) -> Arc<Transport> {
    Transport::by_path(vec![
        (
            crate::test_support::media::serve("has_item"),
            answered(&true),
        ),
        (curating(MUSIC, "find_items"), answered(&[album()])),
        (curated("find_items"), answered(&Vec::<FoundItem>::new())),
        (curated("item_history"), happened(Outcome::Grabbed)),
        (
            curated("item_queue"),
            answered(&queued(Stage::Downloading, false)),
        ),
        (curated("item_parts"), answered(&Vec::<ItemPart>::new())),
        (curating(MUSIC, "stuck_items"), answered(&stuck)),
        (curated("stuck_items"), answered(&Vec::<StuckItem>::new())),
        (curated("wanting"), answered(&false)),
    ])
}

/// A context reaching every curator and the media server, its reads answered by `http`.
fn over(http: Arc<Transport>) -> Ctx {
    let fake = Fake::curator(monitored(), nothing_happened(), nothing_queued());
    let mut ctx = curators::reaching(unmoved(&fake).with_http(http), &curators::ALL);
    ctx.seams.engine = Arc::new(
        curators::running(&curators::ALL).alongside(crate::test_support::media::running()),
    );
    crate::test_support::media::moved_in(&ctx);
    ctx
}

#[tokio::test]
async fn an_album_is_followed_through_the_curator_of_music_with_the_album_as_the_item() {
    let http = album_grabbed(&[]);
    let ctx = over(Arc::clone(&http));

    let report = trace(&ctx, "kind of blue", None, false)
        .await
        .unwrap_or_default();

    assert!(report.matched, "{report:?}");
    let stages: Vec<(Stage, &str)> = report
        .stages
        .iter()
        .map(|stage| (stage.stage, stage.service.as_str()))
        .collect();
    assert_eq!(
        stages,
        [
            (Stage::Monitored, "Lidarr"),
            (Stage::Grabbed, "Lidarr"),
            (Stage::Downloading, "Lidarr"),
            (Stage::Available, "Jellyfin"),
        ]
    );
    assert_eq!(report.item, "Kind of Blue");
    let sent = http.requests();
    let asked: Vec<(TitleKind, i64)> = told::<TitleKind>(&sent, MUSIC, "item_history", "kind")
        .into_iter()
        .zip(told::<i64>(&sent, MUSIC, "item_history", "id"))
        .collect();
    assert_eq!(asked, [(TitleKind::Music, 7)]);
    assert_eq!(
        told::<TitleKind>(&sent, MUSIC, "find_items", "kind"),
        [TitleKind::Music]
    );
}

#[tokio::test]
async fn the_media_server_is_asked_for_the_album_in_its_music_library() {
    let http = album_grabbed(&[]);
    let ctx = over(Arc::clone(&http));

    let _ = trace(&ctx, "kind of blue", None, false).await;

    let kinds: Vec<serde_json::Value> =
        crate::test_support::media::served(&http.requests(), "has_item")
            .into_iter()
            .filter_map(|asked| asked.get("kind").cloned())
            .collect();
    assert_eq!(kinds, [serde_json::json!("music")]);
}

#[tokio::test]
async fn the_stuck_list_names_an_album_stuck_with_the_curator_of_music() {
    let http = album_grabbed(&[StuckItem {
        title: "Kind of Blue".to_owned(),
        stage: Stage::Downloading,
        queued: None,
        title_id: Some(7),
        release: None,
    }]);
    let ctx = over(Arc::clone(&http));

    let report: StuckReport = super::super::stuck(&ctx).await.unwrap_or_default();

    let listed: Vec<(&str, &str)> = report
        .items
        .iter()
        .map(|entry| (entry.title.as_str(), entry.service.as_str()))
        .collect();
    assert_eq!(listed, [("Kind of Blue", "Lidarr")]);
    assert!(!report.incomplete);
    assert_eq!(
        told::<TitleKind>(&http.requests(), MUSIC, "stuck_items", "kind"),
        [TitleKind::Music]
    );
}

#[tokio::test]
async fn a_book_is_followed_through_the_curator_of_books_with_the_book_as_the_item() {
    let http = Transport::by_path(vec![
        (
            crate::test_support::books::curating_here("find_items"),
            answered(&[FoundItem {
                id: 11,
                title: "Dune".to_owned(),
                year: Some(1965),
                monitored: true,
            }]),
        ),
        (curated("find_items"), answered(&Vec::<FoundItem>::new())),
        (curated("item_history"), happened(Outcome::Grabbed)),
        (curated("item_queue"), answered(&Vec::<QueueItem>::new())),
        (curated("item_parts"), answered(&Vec::<ItemPart>::new())),
        (curated("wanting"), answered(&false)),
    ]);
    let fake = Fake::curator(monitored(), nothing_happened(), nothing_queued());
    let mut ctx = crate::test_support::books::reaching(curators::reaching(
        unmoved(&fake).with_http(Arc::<Transport>::clone(&http)),
        &curators::ALL,
    ));
    ctx.seams.engine = Arc::new(
        curators::running(&curators::ALL).alongside(crate::test_support::books::running()),
    );

    let report = trace(&ctx, "dune", None, false).await.unwrap_or_default();

    assert_eq!(report.item, "Dune", "{report:?}");
    assert_eq!(report.furthest, Stage::Grabbed, "{report:?}");
    let kinds: Vec<serde_json::Value> = http
        .requests()
        .iter()
        .filter(|sent| {
            sent.url
                .contains(crate::test_support::books::curating_here("item_history"))
        })
        .filter_map(|sent| {
            let posted: serde_json::Value = serde_json::from_str(sent.body.as_deref()?).ok()?;
            posted.get("kind").cloned()
        })
        .collect();
    assert_eq!(kinds, [serde_json::json!("books")]);
}
