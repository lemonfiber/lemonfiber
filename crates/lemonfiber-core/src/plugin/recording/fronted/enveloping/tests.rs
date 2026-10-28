#[test]
fn the_envelope_is_the_bundled_media_servers_declaration_behind_stand_ins() {
    let declared = super::declared();
    assert!(
        declared
            .as_ref()
            .is_ok_and(|declared| !declared.bytes.is_empty()),
        "{:?}",
        declared.as_ref().err().map(|fault| &fault.why)
    );
    let Ok(declared) = declared else { return };
    let guarded = super::guarded(declared.clone());
    assert_eq!(guarded.declared, declared);
    assert_eq!(guarded.upstream, "http://recording-media-server:58082");
    assert_eq!(guarded.pass, "http://recording-pass:58081/pass");
    assert_eq!(guarded.adapter, None);
    let parties: Vec<(&str, u16, usize)> = super::parties()
        .iter()
        .map(|(host, stand_in)| (*host, stand_in.port, stand_in.answers.len()))
        .collect();
    assert_eq!(
        parties,
        vec![
            ("recording-media-server", 58_082, 0),
            ("recording-pass", 58_081, 0)
        ]
    );
}

#[test]
fn a_recording_that_is_not_a_declaration_says_why() {
    let case = "conformance/media.serve@1/declared-answers.json";
    let said = |files: &[(&str, &[u8])]| {
        super::read(Some(("jellyfin", files)), case)
            .err()
            .map(|fault| fault.why)
            .unwrap_or_default()
    };
    assert!(super::read(None, case)
        .is_err_and(|fault| fault.step == super::STEP && fault.why.contains("names no filler")));
    assert!(said(&[]).contains("jellyfin carries no"));
    assert!(said(&[(case, b"not json")]).contains("does not read"));
    assert!(said(&[(case, b"{}")]).contains("holds no answer"));
    assert!(said(&[(case, br#"{"response": {"json": {"bytes": 1}}}"#)])
        .contains("is not a declaration"));
}
