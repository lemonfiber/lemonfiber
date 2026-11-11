use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use lemonfiber_contract::capabilities::library::curate;
use lemonfiber_contract::Contracted;
use lemonfiber_fixtures::http::Fake;

use super::judged;
use crate::app::healing::gathering::{Curator, Gathered, Item};
use crate::config::Healing;
use crate::model::{Act, Wedge};
use crate::ports::media::TitleKind;
use crate::ports::service::{Download, DownloadState, Downloading, TransferState};
use crate::queue::healing::Struck;

/// The curator every entry is queued with.
const CURATOR: &str = "sonarr";

/// The download client every item is held by.
const CLIENT: &str = "qbittorrent";

/// The category the curator files under.
const CATEGORY: &str = "tv";

/// When every pass runs.
const NOW: u64 = 1_000_000;

/// Healing on, acting on the second pass, leaving a download alone for an hour.
fn healing() -> Healing {
    Healing {
        on: true,
        strikes: 2,
        grace: Duration::from_secs(3600),
        ..Healing::default()
    }
}

/// The curator's entry for `release`, its queue entry `queued`, paired with the client's
/// `download_id`.
fn entry(release: &str, queued: Option<i64>, download_id: Option<&str>) -> Downloading {
    Downloading {
        title_id: 9,
        release: release.to_owned(),
        bytes: 100,
        left: 60,
        expected: None,
        state: DownloadState::Downloading,
        download_id: download_id.map(str::to_owned),
        queued,
        episode: None,
    }
}

/// The client's item `id`, under `category`, doing `state`, added at `added`.
fn item(id: &str, category: &str, state: TransferState, added: Option<u64>) -> Item {
    Item {
        client: CLIENT.to_owned(),
        download: Download {
            name: format!("{id}.mkv"),
            progress: 40,
            speed: Some(0),
            eta: None,
            remaining: Some(60),
            id: Some(id.to_owned()),
            category: Some(category.to_owned()),
            state: Some(state),
            added,
        },
    }
}

/// What a pass gathered: the curator holding `entries`, the client holding `items`.
fn gathered(entries: Vec<Downloading>, items: Vec<Item>) -> Gathered {
    let asked = curate::Adapter(Contracted::new(
        Fake::by_path(Vec::new()),
        "http://127.0.0.1:1",
        CURATOR,
        "key",
    ));
    Gathered {
        curators: vec![Curator {
            id: CURATOR.to_owned(),
            kind: TitleKind::Tv,
            asked: Arc::new(asked),
            entries,
            titles: BTreeMap::from([(9, "The Office".to_owned())]),
            held_back: None,
        }],
        items,
        categories: [CATEGORY.to_owned()].into(),
        ..Gathered::default()
    }
}

#[test]
fn an_entry_is_struck_under_the_clients_id_and_due_once_struck_out_and_past_its_grace() {
    let gathered = gathered(
        vec![entry("The.Office.S01E01", Some(41), Some("ABC"))],
        vec![item(
            "ABC",
            CATEGORY,
            TransferState::Stalled,
            Some(NOW - 7200),
        )],
    );

    let first = judged(&gathered, &BTreeMap::new(), &healing(), NOW);
    let second = judged(&gathered, &first.strikes, &healing(), NOW + 600);

    assert_eq!(
        first
            .strikes
            .get("abc")
            .map(|struck| (struck.wedge, struck.strikes)),
        Some((Wedge::Stalled, 1))
    );
    assert!(first.due(&healing(), NOW).is_empty());
    let due = second.due(&healing(), NOW + 600);
    assert_eq!(due.len(), 1);
    assert!(due
        .iter()
        .all(|found| found.client.as_deref() == Some(CLIENT)
            && found.added() == NOW - 7200
            && found.release() == "The.Office.S01E01"
            && found.next() == [Act::Remove, Act::Blocklist, Act::Search]));

    let young = gathered_young();
    let struck_out = BTreeMap::from([(
        "abc".to_owned(),
        Struck {
            wedge: Wedge::Stalled,
            strikes: 5,
            since: NOW,
        },
    )]);
    let fresh = judged(&young, &struck_out, &healing(), NOW);
    assert!(fresh.due(&healing(), NOW).is_empty());
    let wedged = fresh.wedged(&young, &healing(), NOW);
    assert_eq!(
        wedged.first().and_then(|one| one.in_grace_until),
        Some(NOW - 60 + 3600)
    );
}

/// A stalled download the client added a minute ago.
fn gathered_young() -> Gathered {
    gathered(
        vec![entry("The.Office.S01E02", Some(42), Some("ABC"))],
        vec![item(
            "ABC",
            CATEGORY,
            TransferState::Stalled,
            Some(NOW - 60),
        )],
    )
}

#[test]
fn an_entry_the_client_does_not_name_is_kept_under_its_curator_and_one_with_no_queue_entry_is_not_acted_on(
) {
    let mut warned = entry("The.Office.S01E03", Some(43), None);
    warned.state = DownloadState::Warning;
    let mut unqueued = entry("The.Office.S01E04", None, None);
    unqueued.state = DownloadState::Failed;
    let gathered = gathered(
        vec![warned, unqueued, entry("Fine", Some(44), None)],
        Vec::new(),
    );

    let healthy_before = BTreeMap::from([(
        "sonarr:44".to_owned(),
        Struck {
            wedge: Wedge::Stalled,
            strikes: 2,
            since: NOW - 600,
        },
    )]);

    let judged = judged(&gathered, &healthy_before, &healing(), NOW);

    let keys: Vec<&str> = judged.strikes.keys().map(String::as_str).collect();
    assert_eq!(keys, ["sonarr:43", "sonarr:The.Office.S01E04"]);
    let wedged = judged.wedged(&gathered, &healing(), NOW);
    assert!(wedged
        .iter()
        .all(|one| one.title.as_deref() == Some("The Office")
            && one.service == CURATOR
            && one.added_at == Some(NOW)));
    let next: Vec<Option<Vec<Act>>> = wedged.into_iter().map(|one| one.next).collect();
    assert_eq!(
        next,
        [
            Some(vec![Act::Remove, Act::Blocklist, Act::Search]),
            Some(Vec::new())
        ]
    );
    let off = Healing::default();
    assert!(judged
        .wedged(&gathered, &off, NOW)
        .iter()
        .all(|one| one.next.is_none()));
}

#[test]
fn an_item_under_a_curators_category_that_no_curator_holds_is_an_orphan_only_where_every_curator_answered(
) {
    let items = || {
        vec![
            item("PAIRED", CATEGORY, TransferState::Downloading, None),
            item("ORPHAN", CATEGORY, TransferState::Downloading, None),
            item("OWN", "operator", TransferState::Downloading, None),
        ]
    };
    let complete = gathered(vec![entry("Paired", Some(45), Some("PAIRED"))], items());
    let mut incomplete = gathered(Vec::new(), items());
    incomplete.incomplete = true;

    let found = judged(&complete, &BTreeMap::new(), &healing(), NOW);
    let unsure = judged(&incomplete, &BTreeMap::new(), &healing(), NOW);

    let keys: Vec<&str> = found.strikes.keys().map(String::as_str).collect();
    assert_eq!(keys, ["orphan"]);
    let wedged = found.wedged(&complete, &healing(), NOW);
    assert!(wedged.iter().all(|one| one.class == Wedge::Orphaned
        && one.service == CLIENT
        && one.title.is_none()
        && one.release == "ORPHAN.mkv"
        && one.next == Some(Vec::new())));
    assert!(unsure.found.is_empty());
}

#[test]
fn a_curator_naming_the_clients_id_in_another_case_is_paired_with_it_and_no_orphan_is_left() {
    let gathered = gathered(
        vec![entry("The.Office.S01E05", Some(46), Some("ABCDEF0123"))],
        vec![item(
            "abcdef0123",
            CATEGORY,
            TransferState::Stalled,
            Some(NOW - 7200),
        )],
    );
    let before = BTreeMap::from([(
        "abcdef0123".to_owned(),
        Struck {
            wedge: Wedge::Stalled,
            strikes: 1,
            since: NOW - 600,
        },
    )]);

    let judged = judged(&gathered, &before, &healing(), NOW);

    let keys: Vec<&str> = judged.strikes.keys().map(String::as_str).collect();
    assert_eq!(keys, ["abcdef0123"]);
    assert!(judged
        .found
        .iter()
        .all(|found| found.client.as_deref() == Some(CLIENT)
            && found.struck.strikes == 2
            && found.struck.wedge == Wedge::Stalled));
    assert!(judged
        .wedged(&gathered, &healing(), NOW)
        .iter()
        .all(|one| one.class != Wedge::Orphaned));
}
