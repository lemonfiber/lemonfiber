use super::{
    is_secret, withheld, withheld_by, withheld_text, without_credentials, without_query, REDACTED,
};

/// A stand-in for a credential, assembled rather than written out so no value
/// that reads as one sits in this source.
fn a_credential() -> String {
    ["abcdef", "1234", "567890"].concat()
}

#[test]
fn a_login_in_front_of_a_host_is_withheld_even_where_the_address_carries_no_query() {
    // The gap this closes: the query rule reached the password only as a
    // side-effect of there being a query to strip, so an address with none went
    // through whole — printed on the terminal, in `--json`, and from the API.
    // Pinned whole rather than asserted absent: what has to hold is that the host
    // and the account survive, and "does not contain the password" is true of the
    // empty string.
    let secret = a_credential();
    assert_eq!(
        withheld(&format!(
            "upstream https://ana:{secret}@indexer.example/api refused"
        )),
        format!("upstream https://ana:{REDACTED}@indexer.example/api refused"),
    );
}

#[test]
fn a_port_after_a_host_is_not_a_password() {
    // The rule is a fact about URI syntax, not a guess: everything after the first
    // colon of a *userinfo* is a password, and an address with no userinfo has
    // none. Reading a port as one would delete the number an operator needs most.
    for line in [
        "the service at http://localhost:8989/api did not answer",
        "https://indexer.example:8443/ refused the connection",
    ] {
        assert_eq!(withheld(line), line, "a port was read as a password");
    }
}

#[test]
fn a_sentence_keeps_every_word_after_its_colon() {
    // What a check tells an operator is the service's own reason. Reading the
    // clause in front of the colon as the name of what follows replaced each of
    // these with a redaction — a diagnosis with the diagnosis taken out, on
    // every such error, every time.
    for line in [
        "Authentication failed: the server refused, try again later",
        "Grabbed Passengers: 2016.1080p.WEB-DL",
        "the indexer refused the key: your subscription has expired",
        "the indexer is rate-limiting this key (once a minute); try again shortly",
        "the download client recorded: Authentication failed, check the account",
        "The Passenger (2023) imported to /media/movies",
        "the disk is full: nothing was imported",
    ] {
        assert_eq!(withheld(line), line, "a sentence lost its words");
    }
}

#[test]
fn the_reason_an_indexer_gives_for_refusing_a_key_reaches_the_operator() {
    // The live one. `validate::reading` builds this from the indexer's own
    // `description`, and it is carried to a browser as a finding's detail — where
    // the detail *is* the message, so withholding it withholds the whole answer.
    let said = "your subscription has expired, renew at billing.example";
    let detail = format!("the indexer refused the key: {said}");
    assert!(withheld(&detail).contains(said), "{}", withheld(&detail));
}

#[test]
fn a_setting_line_still_loses_its_value_however_it_is_written() {
    let secret = a_credential();
    // Each case carries its name beside it rather than having one taken off the
    // line, because the line holds the credential and the name does not — and a
    // message argument is worked out only when the assertion fails, so anything
    // computed for one is a line no passing run ever reaches.
    for (name, line) in [
        ("INDEXER_APIKEY", format!("INDEXER_APIKEY={secret}")),
        ("USENET_PASS", format!("  USENET_PASS: {secret}")),
        (
            "SONARR_API_KEY",
            format!("      SONARR_API_KEY: {secret}:more"),
        ),
        (
            "WIREGUARD_PRIVATE_KEY",
            format!("WIREGUARD_PRIVATE_KEY={secret}=more"),
        ),
        (
            "PROVIDER_CREDENTIAL",
            format!("PROVIDER_CREDENTIAL={secret}"),
        ),
        // A name is a name whatever it is spelled with, so long as it is one word.
        (
            "homepage-var-jellyfin-key",
            format!("homepage-var-jellyfin-key={secret}"),
        ),
    ] {
        let shown = withheld(&line);
        assert!(
            shown.ends_with(REDACTED),
            "{name}: the value was not replaced by the marker"
        );
        assert!(
            !shown.contains(&secret),
            "{name}: the credential survived being withheld"
        );
    }
}

#[test]
fn a_field_written_into_a_sentence_still_loses_its_value() {
    // The shapes a service quotes its own configuration back in. Each names a
    // field, which is what separates them from a clause that ends on the same word.
    let secret = a_credential();
    // Each case says what the sentence around the credential must still read, so a
    // rule that took the credential by taking the line with it does not pass.
    for (line, survives) in [
        (
            format!("sonarr refused: api_key={secret}"),
            "sonarr refused",
        ),
        (
            format!("sonarr refused: api_key: {secret}"),
            "sonarr refused",
        ),
        (
            format!("sonarr refused: api_key:{secret}"),
            "sonarr refused",
        ),
        (
            format!("sonarr said: X-Api-Key: {secret} was rejected"),
            "was rejected",
        ),
        (format!("sonarr said: APIKEY: {secret}"), "sonarr said"),
        (
            format!("set PASSWORD={secret} in the environment file"),
            "in the environment file",
        ),
    ] {
        let shown = withheld(&line);
        assert!(
            !shown.contains(&secret),
            "{survives}: the credential survived being withheld"
        );
        assert!(
            shown.contains(REDACTED),
            "{survives}: nothing marked that a value had been taken"
        );
        assert!(
            shown.contains(survives),
            "{survives}: the sentence around the credential went with it"
        );
    }
}

#[test]
fn a_name_with_nothing_after_it_opens_a_block_and_keeps_its_shape() {
    assert_eq!(withheld("    AUTH_SETTINGS:"), "    AUTH_SETTINGS:");
    assert_eq!(withheld("services:"), "services:");
}

#[test]
fn a_line_with_no_separator_at_all_is_returned_as_it_came() {
    for line in ["", "  # a comment", "sonarr keeps restarting"] {
        assert_eq!(withheld(line), line);
    }
}

#[test]
fn an_ordinary_setting_keeps_its_value() {
    for line in [
        "    image: ghcr.io/example/sonarr:4.0",
        "      PUID: 1000",
        "DATA_ROOT=/srv/media",
    ] {
        assert_eq!(withheld(line), line);
    }
}

#[test]
fn a_marker_word_is_read_out_of_a_name_and_not_out_of_a_sentence() {
    for name in [
        "INDEXER_APIKEY",
        "USENET_PASS",
        "some_token",
        "NORDVPN_AUTH",
    ] {
        assert!(is_secret(name), "{name} holds a credential");
    }
    for name in ["DATA_ROOT", "TZ", "LEMONFIBER_USENET"] {
        assert!(!is_secret(name), "{name} does not");
    }
}

#[test]
fn every_line_of_a_detail_is_read_on_its_own() {
    let secret = a_credential();
    let detail = format!("the indexer refused the key: it expired\nINDEXER_APIKEY={secret}");
    let shown = withheld_text(&detail);
    assert!(
        shown.contains("it expired"),
        "the reason on the first line went with the credential on the second"
    );
    assert!(
        !shown.contains(&secret),
        "the credential on the second line survived being withheld"
    );
}

#[test]
fn a_key_that_is_not_the_first_parameter_of_an_address_does_not_ride_out_in_it() {
    // The live shape. `validate` builds `?t=search&apikey=…` to prove an indexer,
    // and the services around it quote the address they failed on into the lines
    // that become an error's detail. Read as a name, what stands in front of the
    // first equals sign of that address is `https://indexer.example/api?t` — no
    // marker in it, so it vouched for everything behind it, key included.
    let secret = a_credential();
    let line =
        format!("sonarr: GET https://indexer.example/api?t=search&q=foo&apikey={secret} failed");
    let shown = withheld(&line);

    assert!(
        !shown.contains(&secret),
        "the key behind the second parameter survived being withheld"
    );
    // And what the sentence was for survives: which service, which address, and
    // that the request failed.
    assert!(
        shown.contains("sonarr: GET"),
        "which service made the request went with the key"
    );
    assert!(
        shown.contains("https://indexer.example/api?"),
        "the address the request went to went with the key"
    );
    assert!(
        shown.ends_with("failed"),
        "that the request failed went with the key"
    );
}

#[test]
fn a_question_mark_with_no_parameters_after_it_is_somebody_asking_a_question() {
    // A query string is parameters; a question mark on its own is a log line.
    assert_eq!(
        withheld("sonarr said: is the indexer reachable? nothing answered"),
        "sonarr said: is the indexer reachable? nothing answered"
    );
}

#[test]
fn a_sentence_beginning_on_a_marker_word_keeps_the_rest_of_itself() {
    // Every service that refuses a credential answers 401, and the word most of
    // them write in front of the reason carries a marker. Read as the name of what
    // follows, that one word took the whole reason with it.
    for line in [
        "Unauthorized: the request was refused by the indexer",
        "Token: the session had already expired",
        "Password: the account is locked until tomorrow",
    ] {
        assert_eq!(withheld(line), line, "a sentence lost its words");
    }
}

#[test]
fn a_credential_under_a_word_of_a_name_is_still_withheld() {
    // The other side of the same rule, and why the sentence is judged on what
    // follows the name rather than on the name alone: a value is one word.
    let secret = a_credential();
    for (name, line) in [
        ("password", format!("password: {secret}")),
        ("api_key", format!("api_key: {secret}")),
        ("token", format!("token={secret}")),
    ] {
        let shown = withheld(&line);
        assert!(
            !shown.contains(&secret),
            "{name}: the credential survived being withheld"
        );
        assert!(
            shown.contains(REDACTED),
            "{name}: nothing marked that a value had been taken"
        );
    }
}

#[test]
fn text_that_has_already_been_withheld_comes_back_as_it_was() {
    // Laundered where a service's words are read and again where the report
    // carrying them is serialised. The marker is not a value: read as one it is
    // withheld in half, and the line grows a tail on every pass after the first.
    for line in [
        "sonarr refused: api_key=(set, not shown)",
        "sonarr refused: api_key: (set, not shown)",
        "INDEXER_APIKEY=(set, not shown)",
        "https://indexer.example/api?(set, not shown)",
    ] {
        assert_eq!(withheld(line), line, "the marker was read as a value");
    }
}

#[test]
fn a_name_a_list_does_not_vouch_for_loses_its_value_whatever_it_is_called() {
    // The other door. Where the text is a settings file there is a list to read
    // names against, and a name nobody has argued for is withheld — including one
    // no marker word names, which is the case a list exists for.
    let account = ["p", "1234567"].concat();
    let vouched = |name: &str| name == "DATA_ROOT";

    assert_eq!(
        withheld_by(&format!("OPENVPN_USER={account}"), &vouched),
        format!("OPENVPN_USER= {REDACTED}")
    );
    assert_eq!(
        withheld_by("DATA_ROOT=/srv/media", &vouched),
        "DATA_ROOT=/srv/media"
    );
}

#[test]
fn an_address_keeps_its_address_and_loses_its_query() {
    // Assembled rather than written out, and a placeholder rather than a plausible
    // key: what this fixture has to be is withheld, and a run of hex in source is a
    // secret scanner's finding for as long as the commit exists.
    let key = ["the", "indexer", "key"].join("-");
    assert_eq!(
        without_query("https://indexer.example/api"),
        "https://indexer.example/api"
    );
    let shown = without_query(&format!("https://indexer.example/api?apikey={key}"));
    assert!(
        shown.starts_with("https://indexer.example/api?"),
        "the address went with the query it carried"
    );
    assert!(
        !shown.contains(&key),
        "the key in the query survived into the shown address"
    );
}

#[test]
fn a_password_written_in_front_of_a_host_is_withheld_and_the_account_is_not() {
    // An indexer behind a proxy that asks for a login, written the one way a URL can
    // carry one. The host, the path and the account survive, because each of those is
    // what somebody checks when a login is refused.
    let secret = a_credential();
    let shown = without_credentials(&format!("https://operator:{secret}@indexer.example/api"));
    assert!(
        !shown.contains(&secret),
        "the password in front of the host survived into the shown address"
    );
    assert!(
        shown.starts_with("https://operator:"),
        "the account went with the password beside it"
    );
    assert!(
        shown.ends_with("@indexer.example/api"),
        "the host and the path went with the password in front of them"
    );
}

#[test]
fn a_password_goes_whether_or_not_the_address_also_carries_a_query() {
    // Both halves on one value, and the sentence around it kept: this is the shape a
    // service logs when the address it was configured with was refused.
    let secret = a_credential();
    let shown = withheld(&format!(
        "prowlarr refused https://operator:{secret}@indexer.example/api?t=caps&apikey={secret}"
    ));
    assert!(
        !shown.contains(&secret),
        "a credential carried in two places survived in one of them"
    );
    assert!(
        shown.starts_with("prowlarr refused https://operator:"),
        "the sentence and the account went with the password"
    );
    assert!(
        shown.contains("@indexer.example/api?"),
        "the host and the fact of a query went with what they carried"
    );
}

#[test]
fn an_address_that_carries_no_password_is_shown_exactly_as_it_was_written() {
    // The cost of firing wrongly is the whole reason this is narrow: a rule that took
    // a path apart because it held a colon would hide the value somebody came to read.
    for kept in [
        "https://indexer.example/api",
        // A username and no password: the syntax says a password is what stands after
        // the first colon, and there is none.
        "https://operator@indexer.example/api",
        // Colons and an at-sign, none of them in an authority.
        "https://indexer.example/api/v3:search/user@host",
        "/srv/media/tv",
        "Europe/Amsterdam",
        "127.0.0.1:8989",
        "",
    ] {
        assert_eq!(without_credentials(kept), kept, "{kept}");
    }
}
