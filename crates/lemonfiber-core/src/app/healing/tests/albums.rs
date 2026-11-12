use super::*;
use crate::test_support::curators::MUSIC;

/// The music curator's queue entry for the wedged album.
const ALBUM_QUEUED: i64 = 51;

/// The music curator's id for the album.
const ALBUM: i64 = 7;

/// The curators answering as a wedged album asks: the music one's clients answering, its
/// album's download not progressing, and its drop and search answered.
fn album_wedged() -> Arc<Fake> {
    let probes = vec![ClientProbe {
        id: "1".to_owned(),
        reachable: true,
        detail: None,
    }];
    let downloading = vec![Downloading {
        title_id: ALBUM,
        release: "Miles.Davis-Kind.of.Blue-FLAC".to_owned(),
        bytes: 100,
        left: 60,
        expected: None,
        state: DownloadState::Warning,
        download_id: None,
        queued: Some(ALBUM_QUEUED),
        episode: None,
    }];
    let titles = vec![CuratedTitle {
        reference: 1,
        id: Some(ALBUM),
        title: "Kind of Blue".to_owned(),
        year: Some(1959),
        monitored: true,
        files: 0,
        seasons: Vec::new(),
    }];
    Fake::by_path(vec![
        (curating(MUSIC, "test_download_clients"), answered(&probes)),
        (curating(MUSIC, "downloading"), answered(&downloading)),
        (curating(MUSIC, "titles"), answered(&titles)),
        (curating(MUSIC, "unqueue"), answered(&DROPPED)),
        (curating(MUSIC, "search"), answered(&())),
        (
            curated("test_download_clients"),
            answered(&Vec::<ClientProbe>::new()),
        ),
        (curated("downloading"), answered(&Vec::<Downloading>::new())),
        ("", refusing()),
    ])
}

#[tokio::test]
async fn an_album_wedged_on_enough_passes_is_named_dropped_searched_again_and_journalled_as_music()
{
    let http = album_wedged();
    let ctx = ctx("healing-album", &ACTING, &http);

    let (first, _) = healed(&ctx).await;
    assert!(
        first
            .items
            .iter()
            .any(|item| item.title.as_deref() == Some("Kind of Blue")
                && item.service == MUSIC.plugin
                && item.strikes == 1),
        "{first:?}"
    );

    let (_, said) = healed(&ctx).await;

    let sent = http.requests();
    assert_eq!(
        told::<Media>(&sent, MUSIC, "downloading", "kind").first(),
        Some(&Media::Music)
    );
    assert_eq!(
        told::<Media>(&sent, MUSIC, "titles", "kind").first(),
        Some(&Media::Music)
    );
    assert_eq!(told::<i64>(&sent, MUSIC, "unqueue", "id"), [ALBUM_QUEUED]);
    assert_eq!(
        told::<Media>(&sent, MUSIC, "unqueue", "kind"),
        [Media::Music]
    );
    assert_eq!(told::<i64>(&sent, MUSIC, "search", "id"), [ALBUM]);
    assert_eq!(
        told::<Media>(&sent, MUSIC, "search", "kind"),
        [Media::Music]
    );
    assert!(
        said.iter()
            .any(|one| one.starts_with("healed at lidarr Kind of Blue")),
        "{said:?}"
    );
    let kept: Vec<Kind> = journal(&ctx)
        .into_iter()
        .map(|change| change.kind)
        .collect();
    assert_eq!(
        kept,
        [Kind::Unqueued {
            kind: Media::Music,
            title: ALBUM,
            release: DROPPED.to_owned(),
        }]
    );
}

#[tokio::test]
async fn a_curator_whose_adapter_is_not_running_is_left_out_of_the_pass() {
    let http = album_wedged();
    let mut ctx = ctx("healing-album-alone", &ACTING, &http);
    ctx.seams.engine = Arc::new(curators::running(&[MUSIC]));

    let (read, _) = healed(&ctx).await;

    assert!(
        read.items.iter().all(|item| item.service == MUSIC.plugin),
        "{read:?}"
    );
    assert!(!read.items.is_empty(), "{read:?}");
    assert!(told::<Media>(&http.requests(), TELEVISION, "downloading", "kind").is_empty());
}
