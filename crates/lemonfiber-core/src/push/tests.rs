use super::{Listed, Record};

/// What a listing shows of a record holds no key.
#[test]
fn a_listing_holds_no_key() {
    let record = Record {
        name: "pixel".to_owned(),
        endpoint: "https://push.example/abc".to_owned(),
        p256dh: "the-public-key".to_owned(),
        auth: "the-secret".to_owned(),
        registered: "2026-11-07T00:00:00Z".to_owned(),
        owed: 0,
        owner: None,
    };
    let shown = serde_json::to_string(&Listed::from(&record)).unwrap_or_default();
    assert!(shown.contains("pixel"), "{shown}");
    assert!(!shown.contains("the-secret"), "{shown}");
    assert!(!shown.contains("the-public-key"), "{shown}");
}
