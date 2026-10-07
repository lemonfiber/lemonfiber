//! Where a call goes, and where a value comes from, decided by reading.

use crate::refusing::refusals;
use crate::schema::tests::WHOLE;
use crate::schema::Manifest;

/// What a manifest declaring only this recipe is refused for about its recipe, each as
/// it is said. The rest of the fixture is held to everything else elsewhere.
fn refused(recipe: &str) -> Vec<String> {
    let before = WHOLE.split("[[recipe]]").next().unwrap_or_default();
    let text = format!(
        "{before}{recipe}\n[[secret]]\nid  = \"api-key\"\nof  = \"komga\"\nwhy = \"Held\"\n\n\
         [requires]\ncapabilities = [\"doctor.contribute\", \"recipe.run\"]\n"
    );
    Manifest::from_toml(&text).map_or_else(
        |refused| vec![refused.to_string()],
        |manifest| {
            refusals(&manifest, &["storage.hardlinks"])
                .iter()
                .map(ToString::to_string)
                .filter(|said| said.starts_with("recipe "))
                .collect()
        },
    )
}

/// Whether any refusal says every one of these words.
fn says(refused: &[String], words: &[&str]) -> bool {
    refused
        .iter()
        .any(|one| words.iter().all(|word| one.contains(word)))
}

/// A recipe carrying `value` from an input of `origin`, with `extra` on the input, to
/// `to` in a header.
fn carrying(origin: &str, extra: &str, to: &str) -> String {
    format!(
        r#"[[recipe]]
id    = "carry"
title = "Carry one value"
why   = "To see where it may go"

[[recipe.input]]
name   = "held"
origin = "{origin}"
{extra}

[[recipe.step]]
id   = "call"
call = {{ method = "GET", to = "{to}", path = "/x", headers = {{ X-Key = "{{{{held}}}}" }} }}

[[recipe.pair]]
value = "held"
to    = "{to}"
"#
    )
}

#[test]
fn a_credential_goes_back_to_the_service_whose_credential_it_is() {
    let said = refused(&carrying("credential-store", r#"of = "sonarr""#, "sonarr"));
    assert!(said.is_empty(), "{said:?}");
}

#[test]
fn a_credential_sent_to_the_plugins_own_service_is_refused() {
    let said = refused(&carrying("credential-store", r#"of = "sonarr""#, "komga"));
    assert!(says(&said, &["held", "sonarr", "komga"]), "{said:?}");
}

#[test]
fn a_credential_sent_to_another_service_of_the_stack_is_refused() {
    let said = refused(&carrying("credential-store", r#"of = "sonarr""#, "radarr"));
    assert!(says(&said, &["held", "sonarr", "radarr"]), "{said:?}");
}

#[test]
fn a_credential_sent_to_a_host_outside_is_refused() {
    let said = refused(&carrying(
        "credential-store",
        r#"of = "sonarr""#,
        "metadata.example.org",
    ));
    assert!(
        says(&said, &["held", "sonarr", "metadata.example.org"]),
        "{said:?}"
    );
}

/// A pair alone is enough to be refused: the credential need not be substituted for its
/// pair to permit something that may not happen.
#[test]
fn a_pair_carrying_a_credential_elsewhere_is_refused_though_nothing_uses_it() {
    let recipe = r#"[[recipe]]
id    = "carry"
title = "Carry one value"
why   = "To see where it may go"

[[recipe.input]]
name   = "held"
origin = "credential-store"
of     = "sonarr"

[[recipe.pair]]
value = "held"
to    = "metadata.example.org"
"#;
    let said = refused(recipe);
    assert!(says(&said, &["pair", "held", "sonarr"]), "{said:?}");
}

#[test]
fn an_operator_value_may_go_wherever_a_pair_permits() {
    let said = refused(&carrying(
        "operator",
        r#"ask = "The claim code plex.tv shows you""#,
        "metadata.example.org",
    ));
    assert!(said.is_empty(), "{said:?}");
}

#[test]
fn an_input_says_what_its_origin_needs_and_nothing_else() {
    let nameless = refused(&carrying("credential-store", "", "sonarr"));
    assert!(says(&nameless, &["names no service"]), "{nameless:?}");
    let asking = refused(&carrying(
        "credential-store",
        "of = \"sonarr\"\nask = \"Your key\"",
        "sonarr",
    ));
    assert!(
        says(&asking, &["ask", "credential store supplies"]),
        "{asking:?}"
    );
    let keyless = refused(&carrying(
        "credential-store",
        r#"of = "flaresolverr""#,
        "sonarr",
    ));
    assert!(
        says(
            &keyless,
            &[
                "of",
                "flaresolverr is no service of the stack's whose credential"
            ]
        ),
        "{keyless:?}"
    );
    let unknown = refused(&carrying("credential-store", r#"of = "nowhere""#, "sonarr"));
    assert!(
        says(
            &unknown,
            &[
                "of",
                "nowhere is no service of the stack's whose credential"
            ]
        ),
        "{unknown:?}"
    );
    let silent = refused(&carrying("operator", "", "sonarr"));
    assert!(says(&silent, &["says nothing in `ask`"]), "{silent:?}");
    let empty = refused(&carrying("operator", r#"ask = """#, "sonarr"));
    assert!(says(&empty, &["says nothing in `ask`"]), "{empty:?}");
    let whose = refused(&carrying(
        "operator",
        "ask = \"Your key\"\nof = \"sonarr\"",
        "sonarr",
    ));
    assert!(says(&whose, &["of", "operator supplies"]), "{whose:?}");
    let captured = refused(&carrying("stack-service", "", "sonarr"));
    assert!(
        says(
            &captured,
            &["stack-service", "credential store or the operator"]
        ),
        "{captured:?}"
    );
}

/// lemonfiber holds no credential for a service a plugin brings, so an input naming one
/// is refused whether or not that service names one of lemonfiber's adapters.
#[test]
fn a_credential_of_the_plugins_own_service_is_refused_adapter_or_not() {
    let held = carrying("credential-store", r#"of = "komga""#, "komga");
    let without = refused(&held);
    assert!(
        says(
            &without,
            &["of", "komga is one of this plugin's own services"]
        ),
        "{without:?}"
    );

    let adapted = WHOLE.replace(
        "provides    = [",
        "api         = { kind = \"seerr\", key_source = \"generated\" }\n\
         listens     = 25600\nprovides    = [",
    );
    let before = adapted.split("[[recipe]]").next().unwrap_or_default();
    let text = format!(
        "{before}{held}\n[requires]\ncapabilities = [\"doctor.contribute\", \"recipe.run\"]\n"
    );
    let with: Vec<String> = Manifest::from_toml(&text).map_or_else(
        |refused| vec![refused.to_string()],
        |manifest| {
            refusals(&manifest, &["storage.hardlinks"])
                .iter()
                .map(ToString::to_string)
                .filter(|said| said.starts_with("recipe "))
                .collect()
        },
    );
    assert!(
        says(&with, &["komga is one of this plugin's own services"]),
        "{with:?}"
    );
}

/// Only what the operator types can be typed in secret.
#[test]
fn a_secret_is_what_the_operator_types_and_nothing_the_store_supplies() {
    let typed = refused(&carrying(
        "operator",
        "ask = \"The claim code\"\nsecret = true",
        "sonarr",
    ));
    assert!(!says(&typed, &["secret"]), "{typed:?}");
    let stored = refused(&carrying(
        "credential-store",
        "of = \"sonarr\"\nsecret = true",
        "sonarr",
    ));
    assert!(
        says(&stored, &["input held.secret", "nobody types"]),
        "{stored:?}"
    );
    let written = refused(&carrying(
        "operator",
        "ask = \"The claim code\"\nsecret = \"yes\"",
        "sonarr",
    ));
    assert!(
        written.first().is_some_and(|one| one.contains("secret")),
        "a secret written as anything but true or false is refused by the reader: {written:?}"
    );
}

/// A call is held to where it goes whether or not it carries anything.
#[test]
fn a_call_carrying_nothing_is_still_held_to_where_it_goes() {
    for (to, why) in [
        ("elsewhere", "neither a service"),
        ("recyclarr", "publishes no port"),
    ] {
        let recipe = format!(
            r#"[[recipe]]
id    = "call"
title = "Call one place"
why   = "To see where it may go"

[[recipe.step]]
id   = "call"
call = {{ method = "GET", to = "{to}", path = "/x" }}
"#
        );
        let said = refused(&recipe);
        assert!(
            says(&said, &["step call.call.to", to, why]),
            "{to}: {said:?}"
        );
    }
}

#[test]
fn a_destination_is_a_service_here_with_a_port_or_a_host_of_two_labels() {
    let dotless = refused(&carrying("operator", r#"ask = "A value""#, "elsewhere"));
    assert!(
        says(&dotless, &["elsewhere", "neither a service"]),
        "{dotless:?}"
    );
    let portless = refused(&carrying("operator", r#"ask = "A value""#, "recyclarr"));
    assert!(
        says(&portless, &["recyclarr", "publishes no port"]),
        "{portless:?}"
    );
}

#[test]
fn a_capture_is_written_as_whoever_answered_it() {
    let recipe = r#"[[recipe]]
id    = "read"
title = "Read one value"
why   = "To see where it came from"

[[recipe.step]]
id      = "outside"
call    = { method = "GET", to = "metadata.example.org", path = "/x" }
capture = [{ name = "api-key", from = "token", origin = "stack-service" }]
"#;
    let said = refused(recipe);
    assert!(
        says(&said, &["api-key", "stack-service", "external-response"]),
        "{said:?}"
    );
}

/// A call carries a credential elsewhere though no pair asks it to: the substitution is
/// refused for it, at the place in the call it is written.
#[test]
fn a_credential_substituted_into_a_call_elsewhere_is_refused_where_it_is_written() {
    let recipe = r#"[[recipe]]
id    = "carry"
title = "Carry one value"
why   = "To see where it may go"

[[recipe.input]]
name   = "held"
origin = "credential-store"
of     = "sonarr"

[[recipe.step]]
id   = "call"
call = { method = "GET", to = "komga", path = "/x?key={{held}}", body = "{{held}}", headers = { X-Key = "{{held}}" } }
"#;
    let said = refused(recipe);
    for place in ["call.path", "call.body", "call.headers.X-Key"] {
        assert!(
            says(&said, &[place, "held", "sonarr", "komga", "goes back only"]),
            "{place}: {said:?}"
        );
    }
}

/// A recipe presenting sonarr's credential to sonarr, capturing what it answers with,
/// trading that once more at sonarr, and carrying both on to `to` in a query value, the
/// body and a header, beside a capture traded for nothing held, with pairs declaring each.
fn trading(to: &str) -> String {
    format!(
        r#"[[recipe]]
id    = "trade"
title = "Trade a credential"
why   = "To see where what it buys may go"

[[recipe.input]]
name   = "held"
origin = "credential-store"
of     = "sonarr"

[[recipe.input]]
name   = "typed"
origin = "operator"
ask    = "A value"

[[recipe.step]]
id      = "sign-in"
call    = {{ method = "POST", to = "sonarr", path = "/login", body = "{{{{held}}}}" }}
capture = [{{ name = "token", from = "token", origin = "stack-service" }}]

[[recipe.step]]
id      = "session"
call    = {{ method = "POST", to = "sonarr", path = "/session", headers = {{ X-Token = "{{{{token}}}}" }} }}
capture = [{{ name = "session", from = "id", origin = "stack-service" }}]

[[recipe.step]]
id      = "plain"
call    = {{ method = "POST", to = "radarr", path = "/plain", body = "{{{{typed}}}}" }}
capture = [{{ name = "free", from = "id", origin = "stack-service" }}]

[[recipe.step]]
id   = "carry"
call = {{ method = "POST", to = "{to}", path = "/x?t={{{{token}}}}", body = "{{{{session}}}}", headers = {{ X-Free = "{{{{free}}}}", X-Token = "{{{{token}}}}" }} }}

[[recipe.pair]]
value = "held"
to    = "sonarr"

[[recipe.pair]]
value = "token"
to    = "sonarr"

[[recipe.pair]]
value = "typed"
to    = "radarr"

[[recipe.pair]]
value = "token"
to    = "{to}"

[[recipe.pair]]
value = "session"
to    = "{to}"

[[recipe.pair]]
value = "free"
to    = "{to}"
"#
    )
}

/// What a credential is traded for is held as the credential is, through as many trades
/// as the recipe makes, at every place a call carries it and at every pair.
#[test]
fn what_a_credential_is_traded_for_goes_back_only_to_its_service() {
    let said = refused(&trading("radarr"));
    for (place, value) in [
        ("step carry.call.path", "token"),
        ("step carry.call.body", "session"),
        ("step carry.call.headers.X-Token", "token"),
        ("pair #4.to", "token"),
        ("pair #5.to", "session"),
    ] {
        assert!(
            says(
                &said,
                &[
                    place,
                    value,
                    "radarr",
                    "captured from a call that carried",
                    "sonarr"
                ]
            ),
            "{place}: {said:?}"
        );
    }
    assert!(
        !said.iter().any(|one| one.contains("carries free")),
        "a capture from a call carrying nothing held is free to go where its pairs say: {said:?}"
    );
}

/// Taken back to the service whose credential it is, what it was traded for is not
/// refused, and a host outside is refused as another service is.
#[test]
fn a_traded_value_may_go_back_to_its_service_and_nowhere_outside() {
    assert!(!refused(&trading("sonarr"))
        .iter()
        .any(|one| one.contains("traded for")),);
    let outside = refused(&trading("api.example.org"));
    assert!(
        says(
            &outside,
            &[
                "step carry.call.path",
                "token",
                "api.example.org",
                "traded for"
            ]
        ) && !outside.iter().any(|one| one.contains("carries free")),
        "{outside:?}"
    );
}
