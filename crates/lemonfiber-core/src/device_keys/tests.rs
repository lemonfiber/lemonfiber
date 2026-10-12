use super::{decoded, encoded, Backing, Kept, Record};

/// A device key named `name` with the fingerprint `fingerprint`.
fn record(name: &str, fingerprint: &str, revoked: Option<&str>) -> Record {
    Record {
        name: name.to_owned(),
        public_key: String::new(),
        fingerprint: fingerprint.to_owned(),
        backing: Backing::Tee,
        app: String::new(),
        registered: "2026-10-12T06:00:00Z".to_owned(),
        under: String::new(),
        used: None,
        revoked: revoked.map(str::to_owned),
    }
}

#[test]
fn a_name_finds_its_device_key_revoked_or_not_and_only_an_active_one_stands() {
    let kept = Kept {
        keys: vec![
            record("old", "aa", Some("2026-10-12T07:00:00Z")),
            record("new", "bb", None),
        ],
    };
    assert!(kept.named("old").is_some());
    assert!(kept.active("old").is_none());
    assert!(kept.active("new").is_some());
    assert_eq!(
        kept.holding("aa").map(|held| held.name.as_str()),
        Some("old")
    );
    assert!(kept.holding("cc").is_none());
}

#[test]
fn each_backing_is_written_as_the_listing_and_the_journal_write_it() {
    for backing in [Backing::Strongbox, Backing::Enclave, Backing::Tee] {
        assert_eq!(
            serde_json::to_value(backing).ok(),
            Some(serde_json::Value::String(backing.written().to_owned()))
        );
    }
}

#[test]
fn base64url_reads_back_padded_or_not() {
    let written = encoded(b"device");
    assert_eq!(decoded(&written), Some(b"device".to_vec()));
    assert_eq!(decoded(&format!("{written}==")), Some(b"device".to_vec()));
    assert_eq!(decoded("not base64url!"), None);
}

#[test]
fn a_record_whose_public_half_does_not_read_has_no_key() {
    assert!(record("broken", "aa", None).key().is_none());
}
