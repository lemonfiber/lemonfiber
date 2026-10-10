use crate::refusing::tests::{names, said, INSTALLABLE};

const ADAPTER: &str = "komga";

const UPSTREAM: &str = "komga-server";

const UPSTREAM_SERVICE: &str = r#"[[service]]
id          = "komga-server"
name        = "Komga server"
image       = "docker.io/gotson/komga"
digest      = "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"
tag         = "1.11.0"
criticality = "important"
"#;

/// The fixture's service with `added` after its configuration directory.
fn with(added: &str) -> String {
    INSTALLABLE.replace(
        "config_path = \"/config\"\n",
        &format!("config_path = \"/config\"\n{added}"),
    )
}

/// The fixture's service as an adapter speaking `media.serve` in front of an upstream of
/// its own, with `upstream` added to the upstream's table and `adapter` to the adapter's.
fn fronting(upstream: &str, adapter: &str) -> Vec<String> {
    said(
        &with(&format!(
            "speaks      = [\"media.serve@1\"]\nlistens     = 8080\nfronts      = \"{UPSTREAM}\"\n{adapter}"
        ))
        .replace(
            "[[claim]]\n",
            &format!("{UPSTREAM_SERVICE}{upstream}\n[[claim]]\n"),
        )
        .replace(
            "[[wiring]]\n",
            &format!("[[wiring]]\nservice         = \"{ADAPTER}\"\n"),
        )
        .replace(
            "id      = \"komga.serves\"\n",
            &format!("id      = \"komga.serves\"\nservice = \"{ADAPTER}\"\n"),
        ),
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
