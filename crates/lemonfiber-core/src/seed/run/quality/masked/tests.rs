use super::{masked, said, LARGEST};
use crate::error::withheld::REDACTED;
use crate::ports::service::Edited;

/// The curator's own key, as lemonfiber keeps it.
const KEY: &str = "4f0c2a9e81b34d7f";

/// A value somebody pasted under a key naming a credential.
const PASTED: &str = "s3cr3tvalue";

fn edit(diff: &str) -> Edited {
    Edited {
        path: "recyclarr.yml".to_owned(),
        diff: diff.to_owned(),
    }
}

/// What masking `diff` shows, holding `secrets`.
fn shown(diff: &str, secrets: &[&str]) -> String {
    let held: Vec<String> = secrets.iter().map(|one| (*one).to_owned()).collect();
    masked(edit(diff), &held).map_or_else(|why| why, |edit| edit.diff)
}

#[test]
fn the_curators_own_key_is_taken_out_under_any_name_and_percent_encoded() {
    let encoded = "k%2Fey%3D1";
    let diff = shown(
        &format!(
            "-  base_url: http://sonarr:8989/?apikey={KEY}\n-  note: {KEY}\n+  url: http://x/?a={encoded}\n+  quality_profiles: balanced\n"
        ),
        &[KEY, "k/ey=1"],
    );

    assert!(!diff.contains(KEY), "{diff}");
    assert!(!diff.contains(encoded), "{diff}");
    assert!(diff.contains(&format!("-  note: {REDACTED}")), "{diff}");
    assert!(diff.contains("+  quality_profiles: balanced"), "{diff}");
}

#[test]
fn every_spelling_of_a_credentials_name_has_its_value_masked() {
    for line in [
        format!("+ \"api_key\": {PASTED}"),
        format!("+ apikey: {PASTED}"),
        format!("+ Api-Key: {PASTED}"),
        format!("+ settings: {{api_key: {PASTED}}}"),
        format!("+ password: {PASTED}"),
        format!("+ bearer_value: {PASTED}"),
    ] {
        let diff = shown(&format!("{line}\n+ min: 25\n"), &[]);
        assert!(!diff.contains(PASTED), "{line} came out as {diff}");
        assert!(diff.contains("min: 25"), "{diff}");
    }
}

#[test]
fn a_value_reached_through_an_anchor_is_masked_where_the_anchor_is_set() {
    let diff = shown(
        &format!("+ shared: &k {PASTED}\n+ sonarr:\n+   api_key: *k\n+ min: 25\n"),
        &[],
    );

    assert!(!diff.contains(PASTED), "{diff}");
    assert!(diff.contains("min: 25"), "{diff}");
}

#[test]
fn a_block_or_quoted_value_under_a_credentials_name_is_masked_whole() {
    let block = shown(
        &format!("- api_key: |\n-   {PASTED}\n-   second{PASTED}\n- min: 0\n"),
        &[],
    );
    let quoted = shown(
        &format!("+ token: \"first\n+   {PASTED}\"\n+ min: 25\n"),
        &[],
    );

    for diff in [block, quoted] {
        assert!(!diff.contains(PASTED), "{diff}");
        assert!(diff.contains("min: "), "{diff}");
    }
}

#[test]
fn a_side_that_does_not_read_as_yaml_shows_no_value_it_sets() {
    let diff = shown(
        &format!("- : : [\n- note: {PASTED}\n- api_key = {PASTED}\n+ min: 25\n"),
        &[],
    );

    assert!(!diff.contains(PASTED), "{diff}");
    assert!(diff.contains(&format!("- note: {REDACTED}")), "{diff}");
    assert!(diff.contains("+ min: 25"), "{diff}");
}

#[test]
fn an_edit_too_long_or_named_by_no_path_is_refused() {
    let long = masked(edit(&"x".repeat(LARGEST + 1)), &[]);
    let unnamed = masked(
        Edited {
            path: "recyclarr\n.yml".to_owned(),
            diff: String::new(),
        },
        &[],
    );

    assert!(long.is_err_and(|why| why.contains("longer than")));
    assert!(unnamed.is_err_and(|why| why.contains("not a path")));
    assert_eq!(
        shown("-  min: 0\n+  min: 25\n", &[""]),
        "-  min: 0\n+  min: 25\n"
    );
}

#[test]
fn an_edit_prints_its_size_and_never_its_lines() {
    let printed = format!("{:?}", edit(&format!("-  api_key: {KEY}\n")));

    assert!(!printed.contains(KEY), "{printed}");
    assert!(printed.contains("bytes"), "{printed}");
}

#[test]
fn a_refusal_naming_a_kept_value_says_why_without_it() {
    let refused = said(
        &format!("the curator refused the key {KEY}"),
        &[KEY.to_owned()],
    );

    assert!(!refused.contains(KEY), "{refused}");
    assert!(
        refused.starts_with("the curator refused the key"),
        "{refused}"
    );
}

#[test]
fn a_tagged_listed_or_numbered_credential_is_masked_and_what_is_not_one_is_left() {
    let diff = shown(
        &format!(
            "+ api_key: !secret {PASTED}\n+ tokens:\n+   - first{PASTED}\n+ pin_password: 7314159\n+ enabled: true\n+ min: 25\n"
        ),
        &[],
    );

    assert!(!diff.contains(PASTED), "{diff}");
    assert!(!diff.contains("7314159"), "{diff}");
    assert!(diff.contains("+ enabled: true"), "{diff}");
    assert!(diff.contains("+ min: 25"), "{diff}");
}
