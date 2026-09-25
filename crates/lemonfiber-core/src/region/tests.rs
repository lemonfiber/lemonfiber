use super::{carried, closing, opening, put, within, without};

const FILE: &str = "watch.{$DOMAIN} {\n\treverse_proxy jellyfin:8096\n}\n";

#[test]
fn a_region_put_in_is_found_holding_exactly_what_was_put() {
    let written = put(FILE, "plugin komga", "comics.x {\n}\n");

    assert_eq!(
        within(&written, "plugin komga").as_deref(),
        Some("comics.x {\n}\n")
    );
    assert!(written.starts_with(FILE), "{written}");
}

#[test]
fn taking_a_region_out_gives_back_the_file_it_was_put_into() {
    let written = put(FILE, "plugin komga", "comics.x {\n}\n");

    assert_eq!(without(&written, "plugin komga").as_deref(), Some(FILE));
}

#[test]
fn a_body_with_no_last_newline_and_a_file_with_none_still_make_whole_lines() {
    let written = put("no newline", "plugin komga", "one line");

    assert_eq!(
        within(&written, "plugin komga").as_deref(),
        Some("one line\n")
    );
    assert_eq!(
        without(&written, "plugin komga").as_deref(),
        Some("no newline\n")
    );
}

#[test]
fn an_empty_file_and_an_empty_body_are_still_a_region() {
    let written = put("", "plugin komga", "");

    assert_eq!(within(&written, "plugin komga").as_deref(), Some(""));
    assert_eq!(without(&written, "plugin komga").as_deref(), Some(""));
}

#[test]
fn putting_again_replaces_the_region_rather_than_adding_a_second() {
    let once = put(FILE, "plugin komga", "first\n");
    let twice = put(&once, "plugin komga", "second\n");

    assert_eq!(twice.matches(&opening("plugin komga")).count(), 1);
    assert_eq!(within(&twice, "plugin komga").as_deref(), Some("second\n"));
}

#[test]
fn two_owners_keep_two_regions_and_taking_one_leaves_the_other() {
    let both = put(&put(FILE, "plugin a", "a\n"), "plugin b", "b\n");
    let left = without(&both, "plugin a").unwrap_or_default();

    assert_eq!(within(&left, "plugin a"), None);
    assert_eq!(within(&left, "plugin b").as_deref(), Some("b\n"));
    assert_eq!(without(&left, "plugin b").as_deref(), Some(FILE));
}

/// A region whose closing marker is gone is not a region, so nothing is found to
/// take out and the lines stay exactly where they are.
#[test]
fn a_region_with_an_edited_marker_is_not_found() {
    let written = put(FILE, "plugin komga", "comics\n");
    let edited = written.replace(&closing("plugin komga"), "# the end");

    assert_eq!(within(&edited, "plugin komga"), None);
    assert_eq!(without(&edited, "plugin komga"), None);
}

#[test]
fn a_closing_marker_naming_somebody_else_does_not_close_the_region() {
    let written = put(FILE, "plugin komga", "comics\n");
    let edited = written.replace(&closing("plugin komga"), &closing("plugin plex"));

    assert_eq!(within(&edited, "plugin komga"), None);
}

#[test]
fn a_closing_marker_with_nothing_opened_is_left_alone() {
    let text = format!("{FILE}{}\n", closing("plugin komga"));

    assert_eq!(within(&text, "plugin komga"), None);
}

#[test]
fn a_file_with_no_region_has_none_to_take_out() {
    assert_eq!(without(FILE, "plugin komga"), None);
}

/// What a pass over the shipped file intends, carrying what is on disk, is what is
/// on disk, byte for byte — which is what keeps that pass from writing the shipped
/// copy over a region.
#[test]
fn the_shipped_file_carrying_the_regions_on_disk_is_the_file_on_disk() {
    let on_disk = put(&put(FILE, "plugin a", "a\n"), "plugin b", "b\n");

    assert_eq!(carried(&on_disk, FILE), on_disk);
}

#[test]
fn a_file_with_no_region_carries_nothing() {
    assert_eq!(carried(FILE, FILE), FILE);
}
