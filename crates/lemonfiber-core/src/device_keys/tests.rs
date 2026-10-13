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

#[test]
fn an_attestation_says_its_platform_and_nothing_it_carries() {
    let android = super::Attestation::Android {
        chain: vec!["certificate".to_owned()],
    };
    assert_eq!(format!("{android:?}"), "Attestation(android)");
    let ios = super::Attestation::Ios {
        object: "object".to_owned(),
    };
    assert_eq!(format!("{ios:?}"), "Attestation(ios)");
}

#[test]
fn an_ios_attestation_that_does_not_read_or_cover_the_key_vouches_for_nothing() {
    use lemonfiber_fixtures::attestations::APPLE;
    let key = super::testing::public(&super::testing::pair());
    let at = super::testing::at(APPLE.at);
    for object in ["not base64url!".to_owned(), super::encoded(APPLE.object)] {
        let attestation = super::Attestation::Ios { object };
        assert_eq!(
            attestation.verified(&key, &[], at),
            Err(super::Unattested::Unverified)
        );
    }
}

#[test]
fn a_challenge_handed_over_says_when_it_lapses_and_not_the_challenge() {
    let challenged = super::Challenged {
        challenge: "the-challenge".to_owned(),
        expires: "2026-10-13T00:01:00Z".to_owned(),
    };
    let said = format!("{challenged:?}");
    assert!(said.contains("2026-10-13T00:01:00Z"));
    assert!(!said.contains("the-challenge"));
}

#[test]
fn a_long_value_is_written_with_its_length_in_the_long_form() {
    let written = super::testing::tlv(&[0x04], &[0; 200]);
    assert_eq!(written.get(..3), Some([0x04, 0x81, 200].as_slice()));
    assert_eq!(written.len(), 203);
}
