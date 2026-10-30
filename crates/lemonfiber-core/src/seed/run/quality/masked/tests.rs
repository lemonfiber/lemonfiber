use super::{held, masked, said, LARGEST};
use crate::error::withheld::REDACTED;
use crate::ports::service::Edited;

/// The curator's own key, as lemonfiber keeps it.
const KEY: &str = "4f0c2a9e81b34d7f";

/// A value somebody pasted under a key naming a credential.
const PASTED: &str = "s3cr3tvalue";

/// What putting the profiles back writes, the same in every case.
const PRESET: &str = "min: 25\n";

fn edit(on_disk: &str, preset: &str) -> Edited {
    Edited {
        path: "recyclarr.yml".to_owned(),
        on_disk: on_disk.to_owned(),
        preset: preset.to_owned(),
    }
}

/// The diff shown of `on_disk` against [`PRESET`], holding `secrets`.
fn shown(on_disk: &str, secrets: &[&str]) -> String {
    let held: Vec<String> = secrets.iter().map(|one| (*one).to_owned()).collect();
    masked(edit(on_disk, PRESET), &held).map_or_else(|why| why, |edit| edit.diff)
}

#[test]
fn the_curators_own_key_is_taken_out_wherever_the_sync_left_it() {
    let encoded = "k%2Fey%3D1";
    let diff = shown(
        &format!(
            "base_url: http://sonarr:8989/?apikey={KEY}\nnote: {KEY}\nurl: http://x/?a={encoded}\nquality_profiles: balanced\n"
        ),
        &[KEY, "k/ey=1"],
    );

    assert!(!diff.contains(KEY), "{diff}");
    assert!(!diff.contains(encoded), "{diff}");
    assert!(diff.contains("- note: "), "{diff}");
}

#[test]
fn a_plain_file_shows_what_names_no_credential_and_the_diff_is_the_cores_own() {
    let diff = shown(
        &format!(
            "# kept\nsonarr:\n  - base_url: http://sonarr:8989\n    api_key: {PASTED}\n    tokens:\n    - one{PASTED}\n    min: 0\n"
        ),
        &[],
    );

    assert!(!diff.contains(PASTED), "{diff}");
    assert!(diff.contains(&format!("- # {REDACTED}")), "{diff}");
    assert!(
        diff.contains("-   - base_url: http://sonarr:8989"),
        "{diff}"
    );
    assert!(diff.contains("-     min: 0"), "{diff}");
    assert!(diff.contains("+ min: 25"), "{diff}");
}

#[test]
fn every_spelling_of_a_credentials_name_has_its_value_masked() {
    for line in [
        format!("\"api_key\": {PASTED}"),
        format!("apikey: {PASTED}"),
        format!("Api-Key: {PASTED}"),
        format!("settings: {{api_key: {PASTED}}}"),
        format!("settings: {{url: x, token: {PASTED}}}"),
        format!("password: {PASTED}"),
        format!("bearer_value: {PASTED}"),
    ] {
        let diff = shown(&format!("{line}\n"), &[]);
        assert!(!diff.contains(PASTED), "{line} came out as {diff}");
    }
}

#[test]
fn a_credential_spelled_apart_from_its_value_is_masked() {
    for line in [
        "settings: {url: x, token: \"\\x73\\x65cret\"}",
        "token: 0x1F2E3D",
        "pin_password: 1_000_000",
        "token: first\n  folded-on",
        "api_key: >\n  folded\n  over",
        "? token\n: explicitvalue",
    ] {
        let diff = shown(&format!("{line}\n"), &[]);
        for leaked in [
            "x73",
            "0x1F2E3D",
            "1_000_000",
            "folded",
            "over",
            "explicitvalue",
        ] {
            assert!(
                !diff.contains(leaked) || !line.contains(leaked),
                "{line} came out as {diff}"
            );
        }
    }
}

#[test]
fn an_anchor_merge_or_tag_on_the_way_to_a_credential_shows_no_value() {
    for on_disk in [
        format!("x: &a {PASTED}\ntoken: *a\n"),
        "x: &a \"\\x73\\x33cretvalue\"\ntoken: *a\n".to_owned(),
        format!("base: &a\n  api_key: {PASTED}\nsonarr:\n  <<: *a\n"),
        format!("api_key: !!binary {PASTED}\n"),
        format!("x: !!str {PASTED}\n"),
    ] {
        let diff = shown(&on_disk, &[]);
        assert!(!diff.contains(PASTED), "{on_disk} came out as {diff}");
        assert!(!diff.contains("x33cret"), "{on_disk} came out as {diff}");
    }
}

#[test]
fn a_decoy_never_makes_up_for_a_key_spelled_in_escapes() {
    let hidden = "\"pass\\x77ord\": \"\\x68\\x75nter2\"\n";
    for decoy in [
        "# token: x\n",
        "note: \"token: here\"\n",
        "description: |\n  password: x\n",
    ] {
        let diff = shown(&format!("{decoy}{hidden}"), &[]);
        assert!(!diff.contains("hunter2"), "{decoy} came out as {diff}");
        assert!(!diff.contains("x75nter2"), "{decoy} came out as {diff}");
    }
}

#[test]
fn whatever_stands_beneath_a_credentials_key_shows_nothing() {
    for on_disk in [
        format!("tokens:\n- {PASTED}\n- other{PASTED}\nmin: 0\n"),
        format!("auth:\n  value: {PASTED}\nmin: 0\n"),
        format!("auth:\n  - {PASTED}\nmin: 0\n"),
        format!("api_key: |\n  {PASTED}\n  second{PASTED}\nmin: 0\n"),
        format!("token: \"first\n  {PASTED}\"\nmin: 0\n"),
        format!("tokens:\n  - first{PASTED}\n"),
    ] {
        let diff = shown(&on_disk, &[]);
        assert!(!diff.contains(PASTED), "{on_disk} came out as {diff}");
    }
    assert!(shown(&format!("tokens:\n- {PASTED}\nmin: 0\n"), &[]).contains("- min: 0"));
}

#[test]
fn a_file_that_cannot_be_read_line_by_line_shows_no_value_it_sets() {
    let diff = shown(
        &format!(": : [\nnote: {PASTED}\napi_key = {PASTED}\n- {PASTED}\n  {PASTED}\n"),
        &[],
    );

    assert!(!diff.contains(PASTED), "{diff}");
    assert!(diff.contains(&format!("- note: {REDACTED}")), "{diff}");
}

#[test]
fn a_login_or_a_credential_inside_an_address_never_shows() {
    for line in [
        "url: http://admin:hunter2@sonarr:8989".to_owned(),
        "base: https://user:pw0rdvalue@host/path".to_owned(),
        format!("url: http://sonarr:8989/?apikey={PASTED}"),
        format!("url: http://sonarr:8989/a;password={PASTED}"),
        format!("url: http://sonarr:8989/?api%4Bey={PASTED}"),
    ] {
        let diff = shown(&format!("{line}\n"), &[]);
        for leaked in ["hunter2", "pw0rdvalue", PASTED] {
            assert!(!diff.contains(leaked), "{line} came out as {diff}");
        }
    }
}

#[test]
fn a_tagged_listed_or_numbered_credential_is_masked() {
    let diff = shown(
        &format!("api_key: !secret {PASTED}\ntokens:\n  - first{PASTED}\npin_password: 7314159\n"),
        &[],
    );

    assert!(!diff.contains(PASTED), "{diff}");
    assert!(!diff.contains("7314159"), "{diff}");
}

#[test]
fn the_preset_is_masked_as_the_file_on_disk_is() {
    let held = [KEY.to_owned()];
    let diff = masked(
        edit(
            "min: 0\n",
            &format!("min: 25\napi_key: {PASTED}\nnote: {KEY}\n"),
        ),
        &held,
    )
    .map(|edit| edit.diff)
    .unwrap_or_default();

    assert!(!diff.contains(PASTED), "{diff}");
    assert!(!diff.contains(KEY), "{diff}");
    assert!(diff.contains("+ min: 25"), "{diff}");
}

#[test]
fn a_file_too_long_or_an_edit_named_by_no_path_is_refused() {
    let long = "x".repeat(LARGEST + 1);
    let too_long_on_disk = masked(edit(&long, PRESET), &[]);
    let too_long_preset = masked(edit(PRESET, &long), &[]);
    let unnamed = masked(
        Edited {
            path: "recyclarr\n.yml".to_owned(),
            on_disk: String::new(),
            preset: String::new(),
        },
        &[],
    );

    assert!(too_long_on_disk.is_err_and(|why| why.contains("longer than")));
    assert!(too_long_preset.is_err_and(|why| why.contains("longer than")));
    assert!(unnamed.is_err_and(|why| why.contains("not a path")));
    assert_eq!(shown("min: 0\n", &[""]), "- min: 0\n+ min: 25\n");
}

#[test]
fn an_edit_prints_its_sizes_and_never_its_lines() {
    let printed = format!("{:?}", edit(&format!("api_key: {KEY}\n"), PRESET));

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
