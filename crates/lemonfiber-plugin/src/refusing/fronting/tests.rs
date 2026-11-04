use crate::refusing::tests::{names, said, with};

const ADAPTER: &str = "komga";

const UPSTREAM: &str = "komga-server";

const UPSTREAM_SERVICE: &str = r#"[[service]]
id          = "komga-server"
name        = "Komga server"
image       = "docker.io/gotson/komga"
digest      = "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"
tag         = "1.11.0"
media_types = ["comics"]
criticality = "important"
"#;

const FILES: &str = "media_types = [\"comics\"]\n";

/// The fixture's service as an adapter speaking `media.serve` in front of an upstream of
/// its own, with `upstream` added to the upstream's table and `adapter` to the adapter's.
fn fronting(upstream: &str, adapter: &str) -> Vec<String> {
    said(&fronted(&format!("{UPSTREAM_SERVICE}{upstream}"), adapter))
}

/// The fixture's service as an adapter speaking `media.serve` in front of `upstream`, with
/// `adapter` added to the adapter's table.
fn fronted(upstream: &str, adapter: &str) -> String {
    with(&format!(
        "speaks      = [\"media.serve@1\"]\nlistens     = 8080\ncontract    = 8081\nfronts      = \"{UPSTREAM}\"\n{adapter}"
    ))
    .replace("[[claim]]\n", &format!("{upstream}\n[[claim]]\n"))
    .replace(
        "[[wiring]]\n",
        &format!("[[wiring]]\nservice         = \"{ADAPTER}\"\n"),
    )
    .replace(
        "id      = \"komga.serves\"\n",
        &format!("id      = \"komga.serves\"\nservice = \"{ADAPTER}\"\n"),
    )
}

#[test]
fn the_upstream_an_adapter_fronts_may_name_its_native_api() {
    let said = fronting("native = \"komga\"\n", "");
    assert!(said.is_empty(), "{said:?}");
}

#[test]
fn a_service_nothing_fronts_may_not_name_a_native_api() {
    let said = fronting("", "native = \"komga\"\n");
    assert!(
        names(&said, &[&format!("service {ADAPTER}.native"), "fronts"]),
        "{said:?}"
    );
}

#[test]
fn a_native_api_is_one_lowercase_word() {
    for named in ["Komga", "komga api", "9komga", "kom-ga", ""] {
        let said = fronting(&format!("native = \"{named}\"\n"), "");
        assert!(
            names(
                &said,
                &[&format!("service {UPSTREAM}.native"), "one lowercase word"]
            ),
            "{named}: {said:?}"
        );
    }
}

#[test]
fn a_one_word_native_api_with_digits_is_named_rightly() {
    let said = fronting("native = \"komga2\"\n", "");
    assert!(said.is_empty(), "{said:?}");
}

/// A case of `media.serve@1` every adapter speaking it is asked.
const CASE: &str = "media.serve@1/refuses-without-the-key";

#[test]
fn an_adapter_may_name_a_case_of_its_contract_that_reaches_a_host_outside_the_stack() {
    let said = fronting("", &format!("outside = {{ \"{CASE}\" = \"plex.tv\" }}\n"));
    assert!(said.is_empty(), "{said:?}");
}

#[test]
fn a_service_fronting_nothing_may_not_name_a_case_reaching_outside() {
    let said = fronting(&format!("outside = {{ \"{CASE}\" = \"plex.tv\" }}\n"), "");
    assert!(
        names(&said, &[&format!("service {UPSTREAM}.outside"), "fronts"]),
        "{said:?}"
    );
}

#[test]
fn a_case_reaching_outside_is_one_of_a_contract_the_adapter_speaks() {
    for case in [
        "owner.claim@1/claim-answers",
        "media.serve@1/no-such-case",
        "media.serve",
        "media.serve@1/",
    ] {
        let said = fronting("", &format!("outside = {{ \"{case}\" = \"plex.tv\" }}\n"));
        assert!(
            names(
                &said,
                &[
                    &format!("service {ADAPTER}.outside"),
                    case,
                    "case of a contract"
                ]
            ),
            "{case}: {said:?}"
        );
    }
}

#[test]
fn the_host_a_case_reaches_is_a_dns_name() {
    let long = format!("{}.tv", "a".repeat(64));
    for host in [
        "plex tv",
        "",
        "https://plex.tv",
        "-plex.tv",
        "plex-.tv",
        "plex..tv",
        "localhost",
        "plex.tv/claim",
        long.as_str(),
    ] {
        let said = fronting("", &format!("outside = {{ \"{CASE}\" = \"{host}\" }}\n"));
        assert!(
            names(&said, &[&format!("service {ADAPTER}.outside"), "DNS name"]),
            "{host}: {said:?}"
        );
    }
}

#[test]
fn an_upstream_filing_what_its_adapter_files_is_asked_each_of_its_cases() {
    let said = fronting("", "");
    assert!(said.is_empty(), "{said:?}");
}

#[test]
fn an_upstream_declaring_no_media_types_is_refused_naming_where_to_declare_them() {
    let said = said(&fronted(&UPSTREAM_SERVICE.replace(FILES, ""), ""));
    assert!(
        names(
            &said,
            &[
                &format!("service {UPSTREAM}.media_types"),
                "declares no media types",
                "media.serve@1",
                "declare media_types = [\"comics\"]",
            ]
        ),
        "{said:?}"
    );
}

#[test]
fn an_upstream_declaring_other_media_types_than_its_adapter_is_refused() {
    let said = said(&fronted(
        &UPSTREAM_SERVICE.replace("[\"comics\"]", "[\"books\"]"),
        "",
    ));
    assert!(
        names(
            &said,
            &[
                &format!("service {UPSTREAM}.media_types"),
                "declares [\"books\"]"
            ]
        ),
        "{said:?}"
    );
}

#[test]
fn an_upstream_of_an_adapter_declaring_no_media_types_is_refused_until_it_declares_its_own() {
    let said = said(&fronted(UPSTREAM_SERVICE, "").replace(FILES, ""));
    assert!(
        names(
            &said,
            &[
                &format!("service {UPSTREAM}.media_types"),
                "declares no media types",
                "declare the media types it files",
            ]
        ),
        "{said:?}"
    );
}
