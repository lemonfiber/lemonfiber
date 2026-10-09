use crate::refusing::tests::{names, said, INSTALLABLE};

/// The fixture's service, speaking `speaks` and saying `rest`.
fn speaking(speaks: &str, rest: &str) -> String {
    INSTALLABLE.replace(
        "config_path = \"/config\"\n",
        &format!("config_path = \"/config\"\nspeaks      = {speaks}\n{rest}"),
    )
}

#[test]
fn an_adapter_speaking_contracts_this_build_speaks_is_refused_nothing() {
    let text = speaking(
        r#"["media.serve@1", "identity.source@1"]"#,
        "listens     = 8080\n",
    );
    assert_eq!(said(&text), Vec::<String>::new());
}

#[test]
fn a_contract_this_build_does_not_speak_is_refused_by_name() {
    for unspoken in ["media.serve@2", "media.serve", "media.watch@1", ""] {
        let said = said(&speaking(
            &format!("[\"{unspoken}\"]"),
            "listens     = 8080\n",
        ));
        assert!(
            names(
                &said,
                &[
                    "service komga.speaks",
                    &format!("{unspoken} is not a contract")
                ]
            ),
            "{unspoken}: {said:?}"
        );
    }
}

#[test]
fn an_adapter_publishing_the_port_it_speaks_on_is_refused() {
    let said = said(&speaking(r#"["media.serve@1"]"#, "listens     = 25600\n"));
    assert!(
        names(&said, &["service komga.port", "loopback alone"]),
        "{said:?}"
    );
}

#[test]
fn a_contract_named_twice_is_refused() {
    let said = said(&speaking(
        r#"["media.serve@1", "media.serve@1"]"#,
        "listens     = 8080\n",
    ));
    assert!(
        names(
            &said,
            &["service komga.speaks", "media.serve@1 more than once"]
        ),
        "{said:?}"
    );
}

#[test]
fn an_adapter_that_says_neither_where_it_answers_nor_one_way_to_ask_it_is_refused() {
    let said = said(&speaking(
        r#"["media.serve@1"]"#,
        "api         = { kind = \"jellyfin\", key_source = \"generated\" }\n",
    ));
    assert!(
        names(&said, &["service komga.listens", "speaks a contract"]),
        "{said:?}"
    );
    assert!(
        names(&said, &["service komga.api", "asked one way"]),
        "{said:?}"
    );
}
