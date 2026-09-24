use lemonfiber_core::changelog::Notes;

use super::super::fixtures::notes as fixture;
use super::notes;

/// What a version's notes read as, as one text.
fn read(running: &str) -> String {
    notes(&fixture(running), running).text()
}

#[test]
fn the_summary_leads_and_the_identifiers_are_nowhere_in_it() {
    let said = read("0.4.0");
    assert!(
        said.contains("• The panel shows the forwarded port — VPN verification"),
        "{said}"
    );
    assert!(
        !said.contains("C2-R4"),
        "an identifier reached a person's report: {said}"
    );
}

#[test]
fn a_requirement_withdrawn_since_is_named_as_withdrawn_rather_than_dropped() {
    // Both of the entry's requirements belong to the same feature and one of them
    // has been withdrawn since, so the feature is named twice and said once each
    // way — the change shipped, and what it served is gone.
    let said = read("0.4.0");
    assert!(
        said.contains("VPN verification, VPN verification (withdrawn)"),
        "{said}"
    );
}

#[test]
fn an_entry_whose_requirement_the_record_does_not_hold_keeps_its_line() {
    // A record written by an older generator can cite an identifier its own index
    // never gained. The change still shipped, so the summary stands on its own
    // rather than the line being dropped or carrying an empty dash.
    let said = read("0.4.0");
    assert!(
        said.contains("• One the record keeps no requirement for\n"),
        "{said}"
    );
    assert!(
        !said.contains("One the record keeps no requirement for —"),
        "{said}"
    );
}

#[test]
fn maintenance_is_counted_and_the_count_reads_as_english() {
    let said = read("0.4.0");
    assert!(
        said.contains("And 1 maintenance change nobody asked for."),
        "{said}"
    );
}

#[test]
fn the_release_says_what_it_delivered_and_when_it_went_out() {
    let said = read("0.4.0");
    assert!(
        said.contains("What 0.4.0 changed, released 2026-04-01"),
        "{said}"
    );
    assert!(said.contains("Seeing what is happening"), "{said}");
}

#[test]
fn a_patch_says_which_version_it_patched_and_why_it_was_taken_back() {
    let said = read("0.3.1");
    assert!(
        said.contains("What 0.3.1 changed, the patch for 0.3.0, released 2026-03-14"),
        "{said}"
    );
    assert!(
        said.contains("Withdrawn: the installer shipped a broken pin"),
        "{said}"
    );
}

#[test]
fn a_release_with_nothing_user_facing_says_so_rather_than_showing_nothing() {
    let said = read("0.3.0");
    assert!(
        said.contains("Internal only — nothing an operator would notice."),
        "{said}"
    );
    assert!(said.contains("And 1 maintenance change"), "{said}");
}

#[test]
fn a_withdrawn_release_is_named_in_the_listing_whichever_version_is_asking() {
    let said = read("0.4.0");
    assert!(
        said.contains("3 releases recorded, back to 0.3.0."),
        "{said}"
    );
    assert!(
        said.contains("0.3.1 was withdrawn: the installer shipped a broken pin"),
        "{said}"
    );
}

#[test]
fn a_version_with_no_release_of_its_own_is_told_so_and_still_sees_the_rest() {
    let said = read("0.5.0");
    assert!(
        said.contains("0.5.0 has not been released, so nothing is written about it yet."),
        "{said}"
    );
    assert!(said.contains("3 releases recorded"), "{said}");
}

#[test]
fn a_record_that_disagrees_with_this_build_is_not_offered_as_current() {
    // Asking as a version older than the newest the record holds: the record
    // claims a release this build cannot have shipped in.
    let said = read("0.3.0");
    assert!(said.contains("disagree, so what"), "{said}");
}

#[test]
fn the_narrow_reading_is_the_release_alone_and_not_the_whole_record() {
    let said = super::brought(&fixture("0.4.0")).text();
    assert!(
        said.contains("What 0.4.0 changed, released 2026-04-01"),
        "{said}"
    );
    assert!(!said.contains("releases recorded"), "{said}");
}

#[test]
fn the_narrow_reading_says_nothing_at_all_where_there_is_nothing_to_say() {
    // A build between tags has no release of its own, and a report about the
    // stack is not the place to explain the changelog's own state.
    assert_eq!(super::brought(&fixture("0.5.0")).text(), "");
    assert_eq!(super::brought(&Notes::unread()).text(), "");
}

#[test]
fn the_narrow_reading_still_refuses_to_offer_a_record_that_disagrees() {
    let said = super::brought(&fixture("0.3.0")).text();
    assert!(said.contains("disagree, so what"), "{said}");
    assert!(said.contains("Internal only"), "{said}");
}

#[test]
fn notes_written_elsewhere_read_as_words_rather_than_as_markup() {
    let said = super::flattened(concat!(
        "## [0.14.0](https://example.test/tag/v0.14.0) — released 2026-09-20\n",
        "\n",
        "> **Withdrawn.** the installer shipped a broken pin\n",
        "\n",
        "### New\n",
        "\n",
        "- The panel shows the forwarded port — [VPN verification · C2-R4](https://example.test/c2) (#42)\n",
        "- One nobody wrote a page for — ~~Gone · Z9-R1~~ (withdrawn)\n",
    ))
    .text();
    // The heading is dropped: whoever shows these has just named the version.
    assert!(!said.contains("0.14.0"), "{said}");
    assert!(
        said.contains("Withdrawn. the installer shipped a broken pin"),
        "{said}"
    );
    assert!(said.contains("New"), "{said}");
    assert!(
        said.contains("  • The panel shows the forwarded port — VPN verification · C2-R4"),
        "{said}"
    );
    assert!(
        !said.contains("https://"),
        "a URL reached a terminal: {said}"
    );
    assert!(!said.contains("(#42)"), "{said}");
    assert!(said.contains("Gone · Z9-R1 (withdrawn)"), "{said}");
}

#[test]
fn a_line_that_is_not_the_shape_this_knows_is_shown_as_it_was_written() {
    // Nothing here parses, so notes from a release older or newer than this
    // renderer still read as text rather than as an error.
    let said = super::flattened("a sentence [with an unclosed bracket\nand a plain one").text();
    assert!(
        said.contains("a sentence [with an unclosed bracket"),
        "{said}"
    );
    assert!(said.contains("and a plain one"), "{said}");
}

#[test]
fn a_bracket_that_is_not_a_link_is_left_where_it_was() {
    let said = super::flattened("- an array [0] and a [link](https://example.test)").text();
    assert!(said.contains("an array [0] and a link"), "{said}");
}

#[test]
fn something_that_merely_ends_in_brackets_keeps_them() {
    let said = super::flattened("- a sentence (about something)").text();
    assert!(said.contains("a sentence (about something)"), "{said}");
    let held = super::flattened("- a sentence (#not-a-number)").text();
    assert!(held.contains("(#not-a-number)"), "{held}");
}

#[test]
fn a_build_carrying_no_record_says_that_rather_than_showing_an_empty_changelog() {
    let said = notes(&Notes::unread(), "0.4.0").text();
    assert!(
        said.contains("This build carries no record of what any release changed."),
        "{said}"
    );
    assert!(!said.contains("recorded, back to"), "{said}");
}
