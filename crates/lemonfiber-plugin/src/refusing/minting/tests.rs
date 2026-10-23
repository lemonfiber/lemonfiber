use crate::refusing::tests::{names, said, speaking, INSTALLABLE};

/// A secret lemonfiber mints for the plugin's upstream, `of` naming whose it is.
fn minted(of: &str) -> String {
    format!(
        "\n[[secret]]\nid     = \"upstream-key\"\nof     = \"{of}\"\nminted = true\nwhy    = \"The key \
         the adapter signs in with\"\n"
    )
}

/// The fixture's adapter handing its upstream `upstream`, with `secrets` declared.
fn handing(upstream: &str, secrets: &str) -> String {
    speaking(
        r#"["media.serve@1"]"#,
        &format!("listens     = 8080\nupstream    = {upstream}\n"),
    ) + secrets
}

#[test]
fn a_minted_secret_of_the_plugins_own_upstream_handed_to_its_adapter_is_refused_nothing() {
    let said = said(&handing(
        r#"{ api_key = { secret = "upstream-key" } }"#,
        &minted("komga-server"),
    ));
    assert!(
        !said
            .iter()
            .any(|one| one.contains("secret") || one.contains("upstream")),
        "{said:?}"
    );
}

#[test]
fn a_minted_secret_is_one_of_the_plugins_own_services() {
    let said = said(&(INSTALLABLE.to_owned() + &minted("sonarr")));
    assert!(
        names(&said, &["secret upstream-key.of", "sonarr", "own services"]),
        "{said:?}"
    );
}

#[test]
fn a_minted_secret_has_no_second_source_in_a_recipes_capture() {
    let recipe = r#"
[[recipe]]
id    = "first"
title = "Read the key"
why   = "Read the key"

[[recipe.step]]
id      = "read"
call    = { method = "GET", to = "komga", path = "/key" }
expect  = { status = 200 }
capture = [{ name = "upstream-key", from = "json.key", origin = "stack-service" }]
"#;
    let said = said(&(INSTALLABLE.to_owned() + &minted("komga") + recipe));
    assert!(
        names(&said, &["secret upstream-key", "also captured"]),
        "{said:?}"
    );
}

#[test]
fn only_an_adapter_hands_an_upstream_its_credentials() {
    let said = said(
        &(INSTALLABLE.replace(
            "config_path = \"/config\"\n",
            "config_path = \"/config\"\nupstream    = { api_key = { secret = \"upstream-key\" } }\n",
        ) + &minted("komga")),
    );
    assert!(
        names(&said, &["service komga.upstream", "speaks a contract"]),
        "{said:?}"
    );
}

#[test]
fn an_upstream_field_is_written_as_a_field_and_names_one_of_the_plugins_own_secrets() {
    for (upstream, secrets, says) in [
        (
            r#"{ ApiKey = { secret = "upstream-key" } }"#,
            minted("komga-server"),
            "service komga.upstream.ApiKey",
        ),
        (
            r#"{ api_key = { secret = "nowhere" } }"#,
            minted("komga-server"),
            "no [[secret]] of this manifest declares",
        ),
        (
            r#"{ api_key = { secret = "curator" } }"#,
            "\n[[secret]]\nid  = \"curator\"\nof  = \"sonarr\"\nwhy = \"Ask the curator\"\n"
                .to_owned(),
            "a credential that crosses",
        ),
    ] {
        let said = said(&handing(upstream, &secrets));
        assert!(names(&said, &[says]), "{upstream}: {said:?}");
    }
}
