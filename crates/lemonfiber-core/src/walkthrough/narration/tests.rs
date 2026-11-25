use super::Step;
use super::{Line, Speed};

#[test]
fn a_line_says_what_its_step_says() {
    let line = Line::at(Step::Importing);
    assert_eq!(line.said, Step::Importing.said());
    assert!(line.detail.is_empty(), "nothing particular to add");
}

#[test]
fn a_search_is_narrated_with_both_numbers() {
    // Releases alone would hide that only one of five indexers answered, which is
    // exactly what an operator needs before concluding their stack works.
    assert_eq!(Line::searched(3, 47).detail, "3 indexers, 47 releases");
    assert_eq!(Line::searched(1, 1).detail, "1 indexer, 1 release");
    assert_eq!(Line::searched(0, 0).detail, "0 indexers, 0 releases");
}

#[test]
fn a_grab_names_the_client_and_the_protocol() {
    let line = Line::sent_to("SABnzbd", "usenet");
    assert_eq!(line.step, Step::Grabbing);
    assert_eq!(line.detail, "SABnzbd, via usenet");
}

#[test]
fn a_download_is_narrated_as_size_rate_and_estimate() {
    let speed = Speed {
        total: 2_100_000_000,
        left: 1_680_000_000,
        rate: 14_000_000,
    };
    assert_eq!(speed.detail(), "2.1 GB · 14 MB/s · ~2m");
}

#[test]
fn a_download_that_is_not_moving_is_not_given_an_estimate() {
    let stalled = Speed {
        total: 2_000_000_000,
        left: 2_000_000_000,
        rate: 0,
    };
    assert_eq!(stalled.remaining(), None);
    assert!(!stalled.detail().contains('~'), "no estimate from no rate");
}

#[test]
fn a_large_download_is_known_to_be_large() {
    assert!(Speed {
        total: 20_000_000_000,
        ..Speed::default()
    }
    .is_large());
    assert!(!Speed {
        total: 300_000_000,
        ..Speed::default()
    }
    .is_large());
    assert_eq!(Speed::default().line().step, Step::Downloading);
}
