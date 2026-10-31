use super::super::Undeclarable;
use super::with;

#[test]
fn a_declaration_names_the_headers_its_upstream_also_reads_a_token_from() {
    let plex = with(|declared| declared.token.headers = vec!["X-Plex-Token".to_owned()]);
    assert_eq!(plex.token.headers, ["X-Plex-Token"]);
    assert!(plex.checked().is_ok());
    let unnamed: serde_json::Value = serde_json::to_value(with(|_| {})).unwrap_or_default();
    assert!(unnamed.pointer("/token/headers").is_none(), "{unnamed}");
}

#[test]
fn a_token_header_named_with_anything_but_letters_digits_and_dashes_is_refused() {
    let longest = "h".repeat(64);
    assert!(
        with(|declared| declared.token.headers = vec![longest.clone()])
            .checked()
            .is_ok()
    );
    let longer = "h".repeat(65);
    for name in [
        "",
        "X_Plex_Token",
        "X-Plex-Token:",
        "X Plex",
        longer.as_str(),
    ] {
        assert_eq!(
            with(|declared| declared.token.headers = vec![name.to_owned()]).checked(),
            Err(Undeclarable::Unspelled {
                name: name.to_owned()
            }),
            "{name}"
        );
    }
}
