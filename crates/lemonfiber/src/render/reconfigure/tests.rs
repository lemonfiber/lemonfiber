use lemonfiber_core::reconfigure::{Active, Edited, Findings, LibraryPath, Opening};

use super::found;

#[test]
fn a_change_that_came_to_nothing_says_nothing() {
    // Not empty headings over empty lists: a report full of those about a
    // timezone teaches the operator to skip the one that matters.
    assert_eq!(found(&Findings::default()).text(), "");
}

#[test]
fn a_move_that_would_lose_the_library_marks_the_path_it_would_lose() {
    let findings = Findings {
        library: vec![LibraryPath {
            service: "Sonarr".to_owned(),
            path: "/data/media/tv".to_owned(),
            host: Some("/srv/new/media/tv".to_owned()),
            carried: false,
            because: "/srv/new/media/tv is not there".to_owned(),
        }],
        ..Findings::default()
    };
    let text = found(&findings).text();
    assert!(text.contains("lost  /data/media/tv"), "{text}");
    assert!(text.contains("is not there"), "{text}");
    assert!(findings.any());
}

#[test]
fn a_move_the_library_survives_says_where_each_folder_lands() {
    let findings = Findings {
        library: vec![LibraryPath {
            service: "Sonarr".to_owned(),
            path: "/data/media/tv".to_owned(),
            host: Some("/srv/new/media/tv".to_owned()),
            carried: true,
            because: "Sonarr keeps filing into /data/media/tv".to_owned(),
        }],
        ..Findings::default()
    };
    assert!(found(&findings).text().contains("kept  /data/media/tv"));
}

#[test]
fn a_reduction_lists_what_is_in_flight_what_stops_and_what_is_kept() {
    let findings = Findings {
        active: vec![Active {
            protocol: "torrent".to_owned(),
            name: "Ubuntu.iso".to_owned(),
            progress: 94,
        }],
        keeps: vec!["everything already downloaded, in /srv/downloads".to_owned()],
        stops: vec!["qbittorrent".to_owned()],
        ..Findings::default()
    };
    let text = found(&findings).text();
    assert!(text.contains("94% Ubuntu.iso (torrent)"), "{text}");
    assert!(text.contains("This stops: qbittorrent"), "{text}");
    assert!(text.contains("/srv/downloads"), "{text}");
}

#[test]
fn a_hand_edit_underneath_is_shown_from_both_sides() {
    let findings = Findings {
        edited: Some(Edited {
            wrote: "/srv/old".to_owned(),
            found: "/mnt/theirs".to_owned(),
            secret: false,
        }),
        ..Findings::default()
    };
    let text = found(&findings).text();
    assert!(text.contains("lemonfiber wrote  /srv/old"), "{text}");
    assert!(text.contains("the file holds    /mnt/theirs"), "{text}");
}

#[test]
fn adding_a_protocol_lists_what_it_asks_for() {
    let findings = Findings {
        opens: vec![Opening {
            what: "Usenet provider".to_owned(),
            because: "it holds the data your downloaders fetch".to_owned(),
            setting: None,
        }],
        ..Findings::default()
    };
    let text = found(&findings).text();
    assert!(text.contains("This opens:"), "{text}");
    assert!(text.contains("Usenet provider"), "{text}");
}
