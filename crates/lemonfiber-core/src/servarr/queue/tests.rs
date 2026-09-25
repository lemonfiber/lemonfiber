use super::{counted, HistoryRecord};

/// One history event of the given type, for the given episode.
fn event(event_type: &str, episode: i64) -> HistoryRecord {
    HistoryRecord {
        event_type: event_type.to_owned(),
        episode_id: Some(episode),
        movie_id: None,
    }
}

#[test]
fn the_same_item_grabbed_again_and_again_is_counted() {
    // Counted per item rather than per release: a loop commonly grabs a
    // different release each time for the same episode, and counting by name
    // would see three unrelated items and never find it.
    let history = [
        event("grabbed", 7),
        event("downloadFailed", 7),
        event("grabbed", 7),
        event("downloadFailed", 7),
        event("grabbed", 7),
    ];
    assert_eq!(counted(&history).get(&7), Some(&3));
}

#[test]
fn an_upgrade_is_not_a_loop() {
    // Newest first: grabbed once since the import, and imported before that.
    // An episode re-grabbed after it arrived is a better copy replacing a
    // worse one — the system working. Counting the older grabs too would flag
    // every upgrade on the machine.
    let history = [
        event("grabbed", 7),
        event("downloadFolderImported", 7),
        event("grabbed", 7),
        event("grabbed", 7),
    ];
    assert_eq!(counted(&history).get(&7), Some(&1));
}

#[test]
fn one_item_looping_says_nothing_about_another() {
    let history = [
        event("grabbed", 7),
        event("grabbed", 8),
        event("grabbed", 7),
        event("grabbed", 7),
    ];
    let counts = counted(&history);
    assert_eq!(counts.get(&7), Some(&3));
    assert_eq!(counts.get(&8), Some(&1));
}

#[test]
fn a_film_is_counted_the_same_way_as_an_episode() {
    // The film services file history by movie, and nothing here needs to know
    // which kind of service answered.
    let film = |event_type: &str| HistoryRecord {
        event_type: event_type.to_owned(),
        episode_id: None,
        movie_id: Some(11),
    };
    assert_eq!(
        counted(&[film("grabbed"), film("grabbed")]).get(&11),
        Some(&2)
    );
}

#[test]
fn an_event_about_nothing_identifiable_is_passed_over() {
    // History carries events with no item on them at all. They are not grabs of
    // something unnamed; they are events this has no business counting.
    let anonymous = HistoryRecord {
        event_type: "grabbed".to_owned(),
        episode_id: None,
        movie_id: None,
    };
    assert!(counted(&[anonymous]).is_empty());
}

#[test]
fn an_event_this_does_not_recognise_changes_nothing() {
    let history = [event("episodeFileRenamed", 7), event("grabbed", 7)];
    assert_eq!(counted(&history).get(&7), Some(&1));
}

#[test]
fn an_empty_history_counts_nothing_rather_than_guessing() {
    assert!(counted(&[]).is_empty());
}
