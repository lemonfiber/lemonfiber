use super::{held, masked, said, LARGEST};
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
fn the_curators_own_key_is_taken_out_wherever_the_sync_left_it() {
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
    }
}

#[test]
fn a_side_reaching_a_value_through_an_anchor_shows_names_alone() {
    let diff = shown(
        &format!("+ shared: &k {PASTED}\n+ sonarr:\n+   api_key: *k\n+ min: 25\n"),
        &[],
    );

    assert!(!diff.contains(PASTED), "{diff}");
    assert!(diff.contains(&format!("+ min: {REDACTED}")), "{diff}");
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
fn a_tagged_listed_or_numbered_credential_is_masked() {
    let diff = shown(
        &format!(
            "+ api_key: !secret {PASTED}\n+ tokens:\n+   - first{PASTED}\n+ pin_password: 7314159\n+ enabled: true\n+ min: 25\n"
        ),
        &[],
    );

    assert!(!diff.contains(PASTED), "{diff}");
    assert!(!diff.contains("7314159"), "{diff}");
}

#[test]
fn a_secret_store_that_cannot_be_read_or_is_not_there_refuses_rather_than_hides_nothing() {
    let scratch = lemonfiber_fixtures::scratch::Scratch::new("masked-unread-store");
    let mut ctx = crate::test_support::a_context().build();
    ctx.settings.env_file = Some(scratch.path().join(".env"));
    let _ = std::fs::write(
        crate::settings::secrets_at(&ctx).unwrap_or_default(),
        "not a store",
    );

    assert!(held(&ctx).is_err());
    ctx.settings.env_file = None;
    assert!(held(&ctx).is_err());
}

#[test]
fn a_side_that_cannot_be_accounted_for_shows_no_value_it_sets_listed_or_continued() {
    for (diff, why) in [
        (
            format!("+ : : [\n+ tokens:\n+ - {PASTED}\n+ - also{PASTED}\n"),
            "a bare list item",
        ),
        (
            format!("-     {PASTED}\n-   min: 0\n"),
            "a block scalar's body, its key outside the hunk",
        ),
        (format!("-   {PASTED}\n"), "a lone continuation"),
        (format!("+ ? token\n+ : {PASTED}\n"), "an explicit key"),
        (
            format!("+ shared: &k {PASTED}\n+ api_key: *k\n"),
            "an anchor",
        ),
        (
            format!("+ \"\\x74oken\": {PASTED}\n"),
            "a key spelled in escapes",
        ),
        (
            format!("+ settings: {{url: x,\n+   token: {PASTED}}}\n"),
            "a flow mapping over lines",
        ),
    ] {
        let diff = shown(&diff, &[]);
        assert!(!diff.contains(PASTED), "{why} came out as {diff}");
    }
}

#[test]
fn a_credential_spelled_apart_from_its_value_is_masked_by_where_it_stands() {
    for line in [
        "+ settings: {url: x, token: \"\\x73\\x65cret\"}".to_owned(),
        "+ token: 0x1F2E3D".to_owned(),
        "+ pin_password: 1_000_000".to_owned(),
        "+ token: first\n+   folded-on".to_owned(),
        "+ api_key: >\n+   folded\n+   on".to_owned(),
    ] {
        let diff = shown(&format!("{line}\n+ min: 25\n"), &[]);
        for leaked in [
            "x73",
            "0x1F2E3D",
            "1_000_000",
            "folded-on",
            "folded",
            "on\n",
        ] {
            assert!(
                !diff.contains(leaked) || !line.contains(leaked),
                "{line} came out as {diff}"
            );
        }
    }
}

#[test]
fn a_decoy_never_makes_up_for_a_key_spelled_in_escapes() {
    let hidden = "+ \"pass\\x77ord\": \"\\x68\\x75nter2\"\n";
    for decoy in [
        "+ # token: x\n",
        "+ note: \"token: here\"\n",
        "+ description: |\n+   password: x\n",
    ] {
        let diff = shown(&format!("{decoy}{hidden}+ min: 25\n"), &[]);
        assert!(!diff.contains("hunter2"), "{decoy} came out as {diff}");
        assert!(!diff.contains("x75nter2"), "{decoy} came out as {diff}");
        assert!(!diff.contains("\\x77"), "{decoy} came out as {diff}");
    }
}

#[test]
fn an_anchor_merge_or_tag_on_the_way_to_a_credential_shows_no_value() {
    for diff in [
        format!("+ x: &a {PASTED}\n+ token: *a\n"),
        "+ x: &a \"\\x73\\x33cretvalue\"\n+ token: *a\n".to_owned(),
        format!("+ base: &a\n+   api_key: {PASTED}\n+ sonarr:\n+   <<: *a\n"),
        format!("+ api_key: !!binary {PASTED}\n"),
        format!("+ x: !!str {PASTED}\n"),
    ] {
        let shown = shown(&diff, &[]);
        assert!(!shown.contains(PASTED), "{diff} came out as {shown}");
        assert!(!shown.contains("x33cret"), "{diff} came out as {shown}");
    }
}

#[test]
fn a_plain_side_is_read_line_by_line_and_shows_what_names_no_credential() {
    let diff = shown(
        &format!(
            "# kept\n- sonarr:\n-   - base_url: http://sonarr:8989\n-     api_key: {PASTED}\n-     tokens:\n-     - one{PASTED}\n-     min: 0\n+ min: 25\n"
        ),
        &[],
    );

    assert!(!diff.contains(PASTED), "{diff}");
    assert!(diff.contains(&format!("# {REDACTED}")), "{diff}");
    assert!(
        diff.contains("-   - base_url: http://sonarr:8989"),
        "{diff}"
    );
    assert!(diff.contains("-     min: 0"), "{diff}");
    assert!(diff.contains("+ min: 25"), "{diff}");
}
