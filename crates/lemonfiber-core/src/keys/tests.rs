use super::{
    digest, names_a_key, shaped, Kept, Minter, Purpose, Record, Scope, Secret, Unkept, Used,
    LONGEST_NAME, PREFIX,
};
use lemonfiber_fixtures::ports::Chance;

/// A source that answers with thirty-two of the same byte.
fn bytes(byte: u8) -> Chance {
    Chance::exactly(Some(vec![byte; 32]))
}

/// A key minted from a source answering with `byte`.
fn a_key(name: &str, byte: u8) -> (Record, Secret) {
    let secret = Secret::mint(&bytes(byte)).unwrap_or_else(|| Secret(String::new()));
    let record = Record::minted(
        name,
        Scope::Read,
        Purpose::HomeAssistant,
        &secret,
        "2026-10-05T06:00:00".to_owned(),
        Minter::Operator,
    );
    (record, secret)
}

#[test]
fn a_secret_carries_the_prefix_and_thirty_two_bytes_of_hex() {
    let secret = Secret::mint(&bytes(0xab));
    assert!(secret
        .as_ref()
        .is_some_and(|secret| secret.as_str() == format!("{PREFIX}{}", "ab".repeat(32))));
    assert!(secret.is_some_and(|secret| shaped(secret.as_str())));
}

#[test]
fn a_source_that_answers_short_mints_no_secret_rather_than_a_narrow_one() {
    assert!(Secret::mint(&Chance::exactly(Some(vec![1; 31]))).is_none());
    assert!(Secret::mint(&Chance::exactly(None)).is_none());
}

#[test]
fn a_secret_never_reaches_anything_that_prints_it() {
    let (record, secret) = a_key("printed", 7);
    assert_eq!(format!("{secret:?}"), "Secret(withheld)");
    let printed = format!("{record:?}");
    assert!(!printed.contains(secret.as_str()), "{printed}");
    assert!(!printed.contains(&secret.digest()), "{printed}");
    assert!(printed.contains("printed"), "{printed}");
}

#[test]
fn what_is_kept_is_the_digest_and_never_the_secret() {
    let (record, secret) = a_key("kept", 9);
    let written = serde_json::to_string(&Kept { keys: vec![record] }).unwrap_or_default();
    assert!(!written.contains(secret.as_str()));
    assert!(!written.contains(&secret.as_str()[PREFIX.len()..]));
    assert!(written.contains(&digest(secret.as_str())));
}

#[test]
fn only_the_prefix_and_the_whole_width_in_lower_case_hex_is_shaped_like_a_key() {
    let good = format!("{PREFIX}{}", "0f".repeat(32));
    assert!(shaped(&good));
    assert!(!shaped(&good[..good.len() - 1]));
    assert!(!shaped(&format!("{good}0")));
    assert!(!shaped(&good.to_uppercase()));
    assert!(!shaped(&"0f".repeat(32)));
    assert!(!shaped(&format!("{PREFIX}{}", "zz".repeat(32))));
}

#[test]
fn a_key_is_found_by_its_secret_and_by_nothing_else() {
    let (first, first_secret) = a_key("first", 1);
    let (second, second_secret) = a_key("second", 2);
    let kept = Kept {
        keys: vec![first, second],
    };
    assert_eq!(
        kept.holding(second_secret.as_str())
            .map(|record| record.name.as_str()),
        Some("second")
    );
    assert_eq!(
        kept.holding(first_secret.as_str())
            .map(|record| record.name.as_str()),
        Some("first")
    );
    let (_, stranger) = a_key("stranger", 3);
    assert!(kept.holding(stranger.as_str()).is_none());
    // The digest itself is not a secret anybody can present.
    assert!(kept.holding(&first_secret.digest()).is_none());
}

#[test]
fn a_revoked_key_is_still_found_so_it_can_be_refused_as_revoked() {
    let (mut record, secret) = a_key("gone", 4);
    record.revoked = Some("2026-10-05T07:00:00".to_owned());
    let kept = Kept { keys: vec![record] };
    assert!(kept
        .holding(secret.as_str())
        .is_some_and(Record::is_revoked));
}

#[test]
fn a_name_is_what_an_address_an_alert_and_the_journal_carry_without_escaping() {
    for name in ["home-assistant", "ha", "mcp.desk_2", "7"] {
        assert!(names_a_key(name), "{name}");
    }
    for name in ["", "-ha", "Home", "home assistant", "ha/../x", "é", ".x"] {
        assert!(!names_a_key(name), "{name}");
    }
    assert!(names_a_key(&"a".repeat(LONGEST_NAME)));
    assert!(!names_a_key(&"a".repeat(LONGEST_NAME + 1)));
}

#[test]
fn keys_survive_being_written_and_read_back() {
    let dir = std::env::temp_dir().join(format!("lemonfiber-keys-{}", std::process::id()));
    let path = dir.join("keys.json");
    let _ = std::fs::remove_file(&path);
    assert!(Kept::at(&path).is_ok_and(|kept| kept == Kept::default()));
    let (record, secret) = a_key("survives", 5);
    let kept = Kept {
        keys: vec![record.clone()],
    };
    assert!(kept.keep(&path).is_ok());
    let read = Kept::at(&path);
    assert!(read
        .as_ref()
        .is_ok_and(|read| read.holding(secret.as_str()) == Some(&record)));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_record_that_does_not_read_as_keys_is_refused_rather_than_read_as_none() {
    let dir = std::env::temp_dir().join(format!("lemonfiber-keys-damaged-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("keys.json");
    assert!(std::fs::write(&path, "not keys").is_ok());
    assert!(matches!(Kept::at(&path), Err(Unkept::Unreadable(at)) if at == path));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_record_that_cannot_be_opened_is_refused_rather_than_read_as_none() {
    let dir = std::env::temp_dir().join(format!("lemonfiber-keys-unopened-{}", std::process::id()));
    // A directory where the file belongs: there, and not a file anything can read.
    let path = dir.join("keys.json");
    let _ = std::fs::create_dir_all(&path);
    assert!(matches!(Kept::at(&path), Err(Unkept::Unreadable(at)) if at == path));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_last_use_that_cannot_be_read_is_none_recorded() {
    let path = std::env::temp_dir().join("lemonfiber-keys-used-nowhere/keys-used.json");
    assert_eq!(Used::at(&path), Used::default());
}
