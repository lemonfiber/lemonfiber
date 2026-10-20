use super::{Admitted, Gated, Target};

#[test]
fn a_route_is_answered_with_its_credential_by_fingerprint_and_never_its_value() {
    let gated = Gated {
        target: Target::Curator {
            media_type: "tv".to_owned(),
        },
        address: "http://curator:80".to_owned(),
        credential: "the-curators-own-key".to_owned(),
        tokens: vec!["ab".repeat(32)],
    };
    let admitted = Admitted::of(&gated);
    assert_eq!(admitted.target, gated.target);
    assert_eq!(admitted.address, gated.address);
    assert_eq!(admitted.tokens, gated.tokens);
    assert_eq!(
        admitted.fingerprint,
        crate::fingerprint("the-curators-own-key")
    );
    let answered = serde_json::to_string(&admitted).unwrap_or_default();
    assert!(!answered.contains("the-curators-own-key"), "{answered}");
}

#[test]
fn a_route_names_a_curator_by_the_media_type_it_files_and_the_media_server_by_nothing() {
    let curator = serde_json::to_value(Target::Curator {
        media_type: "music".to_owned(),
    })
    .unwrap_or_default();
    assert_eq!(
        curator
            .pointer("/curator/media_type")
            .and_then(serde_json::Value::as_str),
        Some("music")
    );
    assert_eq!(
        serde_json::to_value(Target::MediaServer).unwrap_or_default(),
        "media-server"
    );
}

#[test]
fn a_route_written_for_a_reader_withholds_its_credential() {
    let gated = Gated {
        target: Target::MediaServer,
        address: "http://media:80".to_owned(),
        credential: "the-media-servers-key".to_owned(),
        tokens: Vec::new(),
    };
    let written = format!("{gated:?}");
    assert!(!written.contains("the-media-servers-key"), "{written}");
    assert!(written.contains("http://media:80"), "{written}");
}
