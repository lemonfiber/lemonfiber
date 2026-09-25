use super::{checksum, decide, diff, shown, Decision, Materialised};
use crate::config::store::REDACTED;

#[test]
fn a_recorded_checksum_is_read_back_and_an_unrecorded_one_is_none() {
    let mut record = Materialised::new();
    record.record("compose.yaml", 42);
    assert_eq!(record.checksum("compose.yaml"), Some(42));
    assert_eq!(record.checksum("stack.toml"), None);
}

#[test]
fn the_checksum_follows_the_content() {
    assert_eq!(checksum(b"services:\n"), checksum(b"services:\n"));
    assert_ne!(checksum(b"services:\n"), checksum(b"services: edited\n"));
}

#[test]
fn a_record_round_trips_through_its_serialised_form() {
    let mut record = Materialised::new();
    record.record("compose.yaml", 7);
    record.record("stack.toml", 9);
    let json = serde_json::to_string(&record).unwrap_or_default();
    let restored: Materialised = serde_json::from_str(&json).unwrap_or_default();
    assert_eq!(restored, record);
}

#[test]
fn the_decision_reads_every_row_of_the_comparison() {
    // Not on disk: written.
    assert_eq!(decide(None, None, 1), Decision::Write);
    // Already at what lemonfiber would write: left.
    assert_eq!(decide(Some(0), Some(1), 1), Decision::Fresh);
    // Still lemonfiber's last value while its embedded content upgraded: written.
    assert_eq!(decide(Some(1), Some(1), 2), Decision::Write);
    // Changed from what lemonfiber wrote: the operator's edit, preserved.
    assert_eq!(decide(Some(1), Some(2), 3), Decision::Preserve);
    // No record to judge against: a differing file is preserved, not overwritten.
    assert_eq!(decide(None, Some(2), 1), Decision::Preserve);
}

#[test]
fn a_diff_shows_only_the_changed_lines() {
    let yours = "a\nMINE\nc\n";
    let ours = "a\nOURS\nc\n";
    assert_eq!(diff(yours, ours), "- MINE\n+ OURS\n");
}

#[test]
fn a_drifted_credential_is_named_and_neither_value_is_shown() {
    // A diff reaches a terminal, its scrollback and any bug report pasted out of
    // it, so a key that reaches a diff is a key that has to be rotated.
    let yours = "services:\n  sonarr:\n    environment:\n      SONARR_API_KEY: theirs\n";
    let ours = "services:\n  sonarr:\n    environment:\n      SONARR_API_KEY: ours\n";
    let shown = diff(yours, ours);

    assert!(
        shown.contains("SONARR_API_KEY"),
        "the operator learns which drifted"
    );
    assert!(
        !shown.contains("theirs"),
        "their value is not shown: {shown}"
    );
    assert!(
        !shown.contains("ours"),
        "and neither is lemonfiber's: {shown}"
    );
    assert_eq!(shown.matches(REDACTED).count(), 2, "both sides withheld");
}

#[test]
fn a_credential_is_withheld_however_it_is_written() {
    // Both separators, and a value containing the other one — splitting on the later
    // separator would leave half the value in the name and print it.
    for line in [
        "      SONARR_API_KEY: a:b",
        "SONARR_API_KEY=a=b",
        "  ADMIN_PASSWORD: p:ss=word",
        "PROVIDER_CREDENTIAL=x",
    ] {
        let withheld = shown(line);
        assert!(withheld.ends_with(REDACTED), "{line} -> {withheld}");
        assert!(!withheld.contains("word"), "{line} -> {withheld}");
        assert!(!withheld.contains("a:b"), "{line} -> {withheld}");
    }
}

#[test]
fn a_credential_no_marker_word_names_is_withheld_from_a_diff() {
    // The case a marker list cannot answer and the allow-list can: a VPN provider
    // issues an account number, it is half of a paid login, and nothing in the name
    // says so. `/api/config` withholds it; the diff of the same file printed it, in
    // the same run.
    let account = ["p", "1234567"].concat();
    let shown = diff(
        &format!("OPENVPN_USER={account}\n"),
        "OPENVPN_USER=somebody\n",
    );

    assert!(
        !shown.contains(&account),
        "the account number survived into the diff"
    );
    assert!(
        shown.contains("OPENVPN_USER"),
        "the operator does not learn which setting drifted"
    );
    assert_eq!(shown.matches(REDACTED).count(), 2, "both sides withheld");
}

#[test]
fn an_address_in_a_diff_keeps_its_address_and_loses_its_query() {
    // Read the way `/api/config` reads it, which is the point of one list: the
    // address is what the operator is checking, and the key some indexers hand out
    // inside it is not.
    let key = ["the", "indexer", "key"].join("-");
    let shown = diff(
        &format!("INDEXER_URL=https://indexer.example/api?apikey={key}\n"),
        "INDEXER_URL=https://indexer.example/api\n",
    );

    assert!(
        !shown.contains(&key),
        "the key in the query survived into the diff"
    );
    assert!(
        shown.contains("https://indexer.example/api"),
        "the address went with the key riding in its query"
    );
}

#[test]
fn an_ordinary_line_is_shown_exactly_as_it_is() {
    // Withholding is for credentials; a diff that redacted the ordinary lines would
    // be a diff that says nothing.
    for line in [
        "    image: ghcr.io/example/sonarr:4.0",
        "services:",
        "      PUID: 1000",
        "  # a comment",
        "",
    ] {
        assert_eq!(shown(line), line, "{line}");
    }
}

#[test]
fn a_key_that_opens_a_block_keeps_its_shape() {
    // `environment:` with nothing after it is a YAML key opening a block, not a
    // setting with a value — blanking it would corrupt what the operator is reading.
    assert_eq!(shown("    AUTH_SETTINGS:"), "    AUTH_SETTINGS:");
}

#[test]
fn a_diff_of_added_and_removed_lines_leaves_the_matching_ends_out() {
    // Lines only in one side, with a matching head and tail dropped.
    assert_eq!(diff("keep\ngone\ntail\n", "keep\ntail\n"), "- gone\n");
    assert_eq!(diff("keep\ntail\n", "keep\nnew\ntail\n"), "+ new\n");
    // Identical content has no diff.
    assert_eq!(diff("same\n", "same\n"), "");
}
