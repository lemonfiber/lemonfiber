use std::time::Duration;

use super::{classify, in_grace_until, orphaned, strike, Struck};
use crate::model::Wedge;
use crate::ports::service::{Download, DownloadState, Downloading, TransferState};

/// A week.
const WEEK: Duration = Duration::from_secs(7 * crate::instant::DAY);

/// The moment the cases are judged at.
const NOW: u64 = 1_800_000_000;

/// A curator's entry in `state`, expected at `expected`.
fn entry(state: DownloadState, expected: Option<i64>) -> Downloading {
    Downloading {
        title_id: 1,
        release: "A.Film.2026".to_owned(),
        bytes: 100,
        left: 50,
        expected,
        state,
        download_id: Some("7a3f".to_owned()),
        queued: Some(4),
        episode: None,
    }
}

/// A client's item in `state`, with `eta` left.
fn item(state: Option<TransferState>, eta: Option<Duration>) -> Download {
    Download {
        name: "A.Film.2026".to_owned(),
        progress: 50,
        speed: Some(1),
        eta,
        remaining: Some(50),
        id: Some("7a3f".to_owned()),
        category: Some("movies".to_owned()),
        state,
        added: Some(NOW - 3_600),
    }
}

/// What the pair comes to, by its reason alone.
fn judged(entry: &Downloading, item: Option<&Download>) -> Option<Wedge> {
    classify(Some(entry), item, WEEK, NOW).map(|(wedge, _)| wedge)
}

#[test]
fn each_reason_is_told_apart_and_a_healthy_download_is_left_alone() {
    let moving = entry(DownloadState::Downloading, None);
    let cases = [
        (
            entry(DownloadState::Failed, None),
            None,
            Some(Wedge::Failed),
        ),
        (
            moving.clone(),
            Some(item(Some(TransferState::Error), None)),
            Some(Wedge::Failed),
        ),
        (
            moving.clone(),
            Some(item(Some(TransferState::Metadata), None)),
            Some(Wedge::StuckOnMetadata),
        ),
        (
            moving.clone(),
            Some(item(Some(TransferState::Stalled), None)),
            Some(Wedge::Stalled),
        ),
        (
            entry(DownloadState::Warning, None),
            None,
            Some(Wedge::Stalled),
        ),
        (
            entry(DownloadState::Warning, None),
            Some(item(Some(TransferState::Downloading), None)),
            None,
        ),
        (
            moving.clone(),
            Some(item(Some(TransferState::Downloading), Some(WEEK * 2))),
            Some(Wedge::Slow),
        ),
        (
            moving.clone(),
            Some(item(Some(TransferState::Queued), Some(WEEK / 2))),
            None,
        ),
        (
            entry(
                DownloadState::Paused,
                Some(i64::try_from(NOW).unwrap_or_default() + 30 * 86_400),
            ),
            Some(item(Some(TransferState::Paused), None)),
            Some(Wedge::Slow),
        ),
        (moving.clone(), None, None),
    ];
    for (entry, item, wedge) in cases {
        assert_eq!(judged(&entry, item.as_ref()), wedge, "{entry:?} {item:?}");
    }
    assert_eq!(
        classify(
            None,
            Some(&item(Some(TransferState::Stalled), None)),
            WEEK,
            NOW
        ),
        None
    );
}

#[test]
fn a_slow_download_says_how_long_it_would_take_against_what_is_allowed() {
    let said = classify(
        Some(&entry(DownloadState::Downloading, None)),
        Some(&item(None, Some(Duration::from_secs(10 * 86_400)))),
        WEEK,
        NOW,
    );
    assert_eq!(
        said.map(|(_, why)| why).as_deref(),
        Some("at its speed it would take 10 more days, past the 7 allowed")
    );
    assert_eq!(
        orphaned("tv"),
        (
            Wedge::Orphaned,
            "its client holds it under tv, and no curator's queue does".to_owned()
        )
    );
}

#[test]
fn a_strike_is_added_for_the_same_reason_and_restarted_for_another() {
    let first = strike(None, Wedge::Stalled, NOW);
    assert_eq!(
        first,
        Struck {
            wedge: Wedge::Stalled,
            strikes: 1,
            since: NOW
        }
    );
    let second = strike(Some(first), Wedge::Stalled, NOW + 600);
    assert_eq!((second.strikes, second.since), (2, NOW));
    let other = strike(Some(second), Wedge::Slow, NOW + 1_200);
    assert_eq!(
        (other.wedge, other.strikes, other.since),
        (Wedge::Slow, 1, NOW + 1_200)
    );
}

#[test]
fn a_download_is_left_alone_until_its_grace_is_spent() {
    let grace = Duration::from_secs(1_800);
    assert_eq!(in_grace_until(NOW - 600, grace, NOW), Some(NOW + 1_200));
    assert_eq!(in_grace_until(NOW - 1_800, grace, NOW), None);
}
