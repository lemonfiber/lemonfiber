use super::{against, among_versions, step, Jump, Standing};

#[test]
fn a_build_cut_ahead_of_a_release_precedes_it() {
    assert_eq!(among_versions("0.15.0-pre.1", "0.15.0"), Standing::Earlier);
    assert_eq!(among_versions("0.15.0", "0.15.0-pre.1"), Standing::Later);
    assert_eq!(
        among_versions("0.15.0-pre.1", "0.15.0-pre.1"),
        Standing::Same
    );
}

#[test]
fn two_builds_ahead_of_one_release_are_not_ordered_against_each_other() {
    assert_eq!(
        among_versions("0.15.0-pre.1", "0.15.0-pre.2"),
        Standing::Untellable
    );
}

#[test]
fn the_release_a_build_precedes_is_what_orders_it_against_everything_else() {
    assert_eq!(among_versions("0.15.0-pre.1", "0.14.0"), Standing::Later);
    assert_eq!(among_versions("0.15.0-pre.1", "0.16.0"), Standing::Earlier);
    assert_eq!(
        among_versions("not-a-version", "0.14.0"),
        Standing::Untellable
    );
}

#[test]
fn an_image_tag_keeps_the_reading_written_for_image_tags() {
    // `4.0.15-ls123` is the same upstream release packaged differently, and
    // `against` says so by refusing to order it rather than calling it earlier.
    assert_eq!(against("4.0.15-ls123", "4.0.15"), Standing::Untellable);
}

#[test]
fn the_same_tag_is_the_same_version() {
    assert_eq!(against("4.0.1", "4.0.1"), Standing::Same);
}

#[test]
fn a_lower_part_is_the_earlier_version() {
    assert_eq!(against("4.0.1", "4.0.2"), Standing::Earlier);
    assert_eq!(against("4.0.2", "4.0.1"), Standing::Later);
}

#[test]
fn a_leading_v_is_not_part_of_the_number() {
    assert_eq!(against("v3.3.0", "3.3.0"), Standing::Same);
    assert_eq!(against("v3.4.0", "v3.3.0"), Standing::Later);
}

#[test]
fn a_shorter_tag_is_the_earlier_where_what_it_has_agrees() {
    assert_eq!(against("4.0", "4.0.1"), Standing::Earlier);
    assert_eq!(against("4.0.1", "4.0"), Standing::Later);
    assert_eq!(against("4.0", "4.0.0"), Standing::Same);
}

#[test]
fn a_major_difference_outranks_every_part_after_it() {
    assert_eq!(against("10.0.0", "9.9.9"), Standing::Later);
}

#[test]
fn a_tag_that_is_not_a_run_of_numbers_is_not_ordered_at_all() {
    assert_eq!(against("latest", "4.0.1"), Standing::Untellable);
    assert_eq!(against("4.0.1", "nightly"), Standing::Untellable);
    assert_eq!(against("4.0.1-rc1", "4.0.1"), Standing::Untellable);
}

#[test]
fn two_tags_that_are_not_numbers_are_still_the_same_where_they_are_identical() {
    assert_eq!(against("latest", "latest"), Standing::Same);
}

#[test]
fn a_bare_v_reads_as_no_version_at_all() {
    assert_eq!(against("v", "4.0.1"), Standing::Untellable);
}

#[test]
fn a_first_number_that_moved_is_the_major_step() {
    assert_eq!(step("4.0.15", "5.0.0"), Jump::Major);
    assert_eq!(step("v5.0.0", "4.0.15"), Jump::Major);
}

#[test]
fn a_second_number_that_moved_alone_is_the_minor_step() {
    assert_eq!(step("4.0.15", "4.1.0"), Jump::Minor);
}

#[test]
fn a_step_behind_the_second_number_is_a_patch() {
    assert_eq!(step("4.0.15", "4.0.16"), Jump::Patch);
    assert_eq!(step("4.0.15", "4.0.15"), Jump::Patch);
}

#[test]
fn an_absent_part_reads_as_zero_rather_than_as_unknown() {
    assert_eq!(step("4.0", "4.1"), Jump::Minor);
    assert_eq!(step("4", "4.0.0"), Jump::Patch);
}

#[test]
fn a_tag_that_is_not_a_run_of_numbers_has_no_size_of_step_either() {
    assert_eq!(step("release-0.14.5", "0.14.6"), Jump::Untellable);
    assert_eq!(step("4.0.1", "V3.0.4"), Jump::Untellable);
}

#[test]
fn how_large_a_step_is_reads_the_same_way_however_it_is_carried() {
    let jump = step("4.0.15", "5.0.0");
    assert_eq!(jump, jump.clone());
    let written = serde_json::to_string(&jump).ok();
    assert_eq!(written.as_deref(), Some("\"major\""));
}
