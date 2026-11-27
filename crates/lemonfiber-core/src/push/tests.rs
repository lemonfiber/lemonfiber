use super::{fingerprint_of, host_of, Listed, Record};

/// What a listing shows of a record holds no key.
#[test]
fn a_listing_holds_no_key() {
    let record = Record {
        name: "pixel".to_owned(),
        endpoint: "https://push.example/abc".to_owned(),
        p256dh: "the-public-key".to_owned(),
        auth: "the-secret".to_owned(),
        registered: crate::instant::Instant::from_seconds(1_794_009_600),
        owed: 0,
        owner: None,
        wakes: Vec::new(),
        last: None,
        failures: Vec::new(),
        proven: None,
    };
    let shown = serde_json::to_string(&Listed::from(&record)).unwrap_or_default();
    assert!(shown.contains("pixel"), "{shown}");
    assert!(!shown.contains("the-secret"), "{shown}");
    assert!(!shown.contains("the-public-key"), "{shown}");
}

/// What a listing shows of an address is its host and a fingerprint, never the address.
#[test]
fn a_listing_names_the_address_by_its_host_and_a_fingerprint() {
    let record = Record {
        name: "pixel".to_owned(),
        endpoint: "https://push.example/abc".to_owned(),
        p256dh: "the-public-key".to_owned(),
        auth: "the-secret".to_owned(),
        registered: crate::instant::Instant::from_seconds(1_794_009_600),
        owed: 0,
        owner: None,
        wakes: Vec::new(),
        last: None,
        failures: Vec::new(),
        proven: None,
    };
    let listed = Listed::from(&record);
    assert_eq!(listed.host, "push.example");
    assert_eq!(listed.fingerprint, "f7a263f8786e9f95");
    let shown = serde_json::to_string(&listed).unwrap_or_default();
    assert!(!shown.contains("push.example/abc"), "{shown}");
    assert!(!shown.contains("https://"), "{shown}");
}

#[test]
fn the_host_is_the_host_alone() {
    assert_eq!(
        host_of("https://user:secret@up.example.org:8443/wake/abc?token=1"),
        "up.example.org"
    );
}

#[test]
fn a_fingerprint_is_sixteen_digits_of_the_whole_address() {
    let one = fingerprint_of("https://push.example/abc");
    assert_eq!(one.len(), 16);
    assert!(one
        .chars()
        .all(|digit| matches!(digit, '0'..='9' | 'a'..='f')));
    assert_ne!(one, fingerprint_of("https://push.example/abd"));
}
