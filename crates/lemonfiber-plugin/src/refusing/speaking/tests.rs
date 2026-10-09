use crate::refusing::tests::{names, said, INSTALLABLE};

/// The upstream the fixture's service stands in front of when it speaks.
const UPSTREAM: &str = r#"[[service]]
id          = "komga-server"
name        = "Komga server"
image       = "docker.io/gotson/komga"
digest      = "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"
tag         = "1.11.0"
criticality = "important"

"#;

/// The fixture's service, speaking `speaks` in front of an upstream of its own and
/// saying `rest`.
fn speaking(speaks: &str, rest: &str) -> String {
    INSTALLABLE
        .replace(
            "config_path = \"/config\"\n",
            &format!(
                "config_path = \"/config\"\nspeaks      = {speaks}\nfronts      = \"komga-server\"\n{rest}"
            ),
        )
        .replace("[[claim]]\n", &format!("{UPSTREAM}[[claim]]\n"))
        .replace("[[wiring]]\n", "[[wiring]]\nservice         = \"komga\"\n")
        .replace(
            "id      = \"komga.serves\"\n",
            "id      = \"komga.serves\"\nservice = \"komga\"\n",
        )
}

#[test]
fn an_adapter_names_one_other_service_of_its_plugin_that_speaks_nothing_as_its_upstream() {
    let fronting = |fronts: &str| {
        said(
            &speaking(r#"["media.serve@1"]"#, "listens     = 8080\n")
                .replace("fronts      = \"komga-server\"\n", fronts),
        )
    };
    for (fronts, says) in [
        ("", "an adapter has to name"),
        ("fronts      = \"komga\"\n", "names the adapter itself"),
        (
            "fronts      = \"kavita\"\n",
            "kavita, which this plugin does not declare",
        ),
    ] {
        let said = fronting(fronts);
        assert!(
            names(&said, &["service komga.fronts", says]),
            "{fronts}: {said:?}"
        );
    }
    let both = speaking(r#"["media.serve@1"]"#, "listens     = 8080\n").replace(
        "criticality = \"important\"\n\n[[claim]]",
        "criticality = \"important\"\nspeaks      = [\"media.serve@1\"]\nlistens     = 8081\nfronts      = \"komga\"\nprovides    = [\"komga:other\"]\n\n[[claim]]",
    );
    assert!(
        names(
            &said(&both),
            &["service komga.fronts", "speaks a contract itself"]
        ),
        "{:?}",
        said(&both)
    );
    let stray = INSTALLABLE.replace(
        "config_path = \"/config\"\n",
        "config_path = \"/config\"\nfronts      = \"komga-server\"\n",
    );
    assert!(
        names(
            &said(&stray),
            &["service komga.fronts", "only a service that speaks"]
        ),
        "{:?}",
        said(&stray)
    );
}

#[test]
fn an_adapter_speaking_contracts_this_build_speaks_is_refused_nothing() {
    let text = speaking(r#"["media.serve@1"]"#, "listens     = 8080\n");
    assert_eq!(said(&text), Vec::<String>::new());
}

#[test]
fn a_contract_of_a_capability_the_service_does_not_provide_is_refused_by_name() {
    let said = said(&speaking(
        r#"["media.serve@1", "identity.source@1"]"#,
        "listens     = 8080\n",
    ));
    assert!(
        names(
            &said,
            &["service komga.speaks", "does not provide identity.source"]
        ),
        "{said:?}"
    );
    assert_eq!(said.len(), 1, "{said:?}");
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
