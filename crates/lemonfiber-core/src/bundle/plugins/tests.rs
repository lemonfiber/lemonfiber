use super::{listed, withheld, NONE_INSTALLED, WITHHELD};
use crate::test_support::{a_placed, an_installed};

#[test]
fn each_plugin_is_named_with_its_version_origin_and_review_and_its_configuration_withheld() {
    let mut reviewed = an_installed("komga", vec![a_placed("komga", &[], None)]);
    reviewed.from = "https://github.com/example/plugin-komga".to_owned();
    reviewed.declared.reviewed = true;
    let unrecorded = an_installed("shelf", vec![a_placed("shelf", &[], None)]);

    let said = listed(&[reviewed, unrecorded]);

    let lines: Vec<&str> = said.lines().collect();
    assert_eq!(
        lines,
        [
            format!(
                "komga 1.0.0, from https://github.com/example/plugin-komga, reviewed; {WITHHELD}"
            ),
            format!(
                "shelf 1.0.0, from a source its record does not name, not reviewed; {WITHHELD}"
            ),
        ]
    );
}

#[test]
fn a_machine_with_no_plugin_says_so() {
    assert_eq!(listed(&[]), NONE_INSTALLED);
}

#[test]
fn a_plugins_settings_are_left_out_of_the_configuration_and_the_stacks_kept() {
    let kept = withheld("PUID=1000\nPLUGIN_KOMGA_KOMGA_API_KEY=abc\n  PLUGIN_SHELF_X=y\nTZ=UTC");
    assert_eq!(kept, "PUID=1000\nTZ=UTC");
}
