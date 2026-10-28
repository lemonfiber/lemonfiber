use crate::refusing::tests::{names, said, speaking, with, INSTALLABLE};

#[test]
fn an_adapter_names_one_other_service_of_its_plugin_that_speaks_nothing_as_its_upstream() {
    let fronting = |fronts: &str| {
        said(
            &speaking(r#"["media.serve@1"]"#, "listens     = 8080\n")
                .replace("fronts      = \"komga-server\"\n", fronts),
        )
    };
    assert_eq!(fronting(""), Vec::<String>::new(), "it is its own upstream");
    for (fronts, says) in [
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
fn every_way_the_compose_writer_or_the_proxy_reaches_a_contract_port_is_refused() {
    for (rest, field, says) in [
        (
            "listens     = 25600\ncontract    = 25600\n",
            "contract",
            "the port it publishes",
        ),
        ("listens     = 8080\ncontract    = 25600\n", "contract", "the proxy's route"),
        (
            "listens     = 8080\ncontract    = 8920\n[[service.publish]]\nport = 8920\n",
            "contract",
            "a further port it publishes",
        ),
        (
            "listens     = 8080\ncontract    = 9000\n[[service.publish]]\nport = 8920\nlistens = 9000\n",
            "contract",
            "a further port it publishes",
        ),
        ("listens     = 8080\ncontract    = 8080\n", "contract", "the port it publishes"),
    ] {
        let said = said(&speaking(r#"["media.serve@1"]"#, rest));
        assert!(
            names(
                &said,
                &[&format!("service komga.{field}"), says, "loopback alone"]
            ),
            "{rest}: {said:?}"
        );
    }
}

#[test]
fn a_contract_port_reached_by_nothing_but_loopback_is_refused_nothing() {
    let apart = speaking(
        r#"["media.serve@1"]"#,
        "listens     = 8080\ncontract    = 8081\n[[service.publish]]\nport = 8921\n",
    );
    assert_eq!(said(&apart), Vec::<String>::new());
}

#[test]
fn a_contract_port_declared_by_a_service_speaking_nothing_is_refused_by_name() {
    let said = said(&with("contract    = 8081\n"));
    assert!(
        names(&said, &["service komga.contract", "it speaks none"]),
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
fn an_adapter_that_says_nowhere_it_answers_is_refused() {
    let said = said(&speaking(r#"["media.serve@1"]"#, "").replace("contract    = 8081\n", ""));
    assert!(
        names(&said, &["service komga.listens", "speaks a contract"]),
        "{said:?}"
    );
}

#[test]
fn a_facet_is_spoken_without_being_provided() {
    let text = speaking(
        r#"["media.serve@1", "owner.claim@1"]"#,
        "listens     = 8080\n",
    );
    assert_eq!(said(&text), Vec::<String>::new());
}

#[test]
fn a_facet_named_among_what_a_service_provides_is_refused_by_name() {
    let text = speaking(
        r#"["media.serve@1", "owner.claim@1"]"#,
        "listens     = 8080\n",
    )
    .replace(
        "provides    = [\"media.serve\", \"komga:kobo-sync\"]",
        "provides    = [\"media.serve\", \"komga:kobo-sync\", \"owner.claim\"]",
    );
    let said = said(&text);
    assert!(
        names(&said, &["service komga.provides", "owner.claim, a facet"]),
        "{said:?}"
    );
}
