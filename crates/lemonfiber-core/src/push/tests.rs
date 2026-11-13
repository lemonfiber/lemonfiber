use super::{Listed, Record};

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
