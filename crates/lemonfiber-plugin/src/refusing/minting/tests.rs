use crate::refusing::tests::{names, said, speaking, INSTALLABLE};

/// A secret lemonfiber mints for the plugin's upstream, `of` naming whose it is.
fn minted(of: &str) -> String {
    format!(
        "\n[[secret]]\nid     = \"upstream-key\"\nof     = \"{of}\"\nminted = true\nwhy    = \"The key \
         the adapter signs in with\"\n"
    )
}

/// The contracts an adapter that serves media speaks.
const SERVES: &str = r#"["media.serve@1"]"#;

/// The contracts an adapter a part is claimed through speaks.
const CLAIMS: &str = r#"["owner.claim@1"]"#;

/// The fixture's adapter handing its upstream `upstream`, with `secrets` declared.
fn handing(upstream: &str, secrets: &str) -> String {
    handing_as(SERVES, upstream, secrets)
}

/// The fixture's adapter speaking `speaks` and handing its upstream `upstream`, with `secrets`
/// declared.
fn handing_as(speaks: &str, upstream: &str, secrets: &str) -> String {
    speaking(
        speaks,
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

#[test]
fn a_secret_lemonfiber_mints_is_never_stood_in_for_when_recorded() {
    let said = said(
        &(INSTALLABLE.to_owned()
            + &minted("komga")
                .replace("minted = true", "minted = true\nrecorded_with = \"minted\"")),
    );
    assert!(
        names(
            &said,
            &["secret upstream-key.recorded_with", "operator alone holds"]
        ),
        "{said:?}"
    );
}

#[test]
fn a_secret_stood_in_for_by_a_value_holding_a_control_character_is_refused() {
    let entered =
        "\n[[secret]]\nid            = \"tunnel\"\nof            = \"komga\"\nwhy           = \
                   \"Reach the tunnel\"\nrecorded_with = \"a\\tb\"\n";
    let refused = said(&(INSTALLABLE.to_owned() + entered));
    assert!(
        names(
            &refused,
            &["secret tunnel.recorded_with", "control character"]
        ),
        "{refused:?}"
    );
    let made = entered.replace("\"a\\tb\"", "\"minted\"");
    let kept = said(&(INSTALLABLE.to_owned() + &made));
    assert!(
        !kept.iter().any(|one| one.contains("recorded_with")),
        "{kept:?}"
    );
}

/// The fixture's adapter speaking `speaks` and handing its upstream a claim code declared with
/// `extra`.
fn claiming(speaks: &str, extra: &str) -> Vec<String> {
    said(&handing_as(
        speaks,
        r#"{ claim_code = { secret = "claim-code" } }"#,
        &format!(
            "\n[[secret]]\nid  = \"claim-code\"\nof  = \"komga-server\"\nwhy = \"Claim it\"\n{extra}"
        ),
    ))
}

/// A claim code that lapses beside its stand-in.
const LAPSING: &str = "lapses_s = 240\nrecorded_with = \"claim-0000\"\n";

#[test]
fn a_code_claimed_through_the_adapter_that_lapses_beside_its_stand_in_is_refused_nothing() {
    let said = claiming(CLAIMS, LAPSING);
    assert!(!said.iter().any(|one| one.contains("lapses_s")), "{said:?}");
}

#[test]
fn a_code_that_lapses_is_refused_without_its_stand_in_at_no_seconds_minted_or_handed_nowhere() {
    for (speaks, extra, message) in [
        (CLAIMS, "lapses_s = 240\n", "without recorded_with"),
        (
            CLAIMS,
            "lapses_s = 0\nrecorded_with = \"claim-0000\"\n",
            "no seconds at all",
        ),
        (
            CLAIMS,
            "lapses_s = 240\nminted = true\n",
            "mints or carries",
        ),
        (SERVES, LAPSING, "no adapter speaking owner.claim@1"),
    ] {
        let said = claiming(speaks, extra);
        assert!(
            names(&said, &["secret claim-code.lapses_s", message]),
            "{extra}: {said:?}"
        );
    }
}
