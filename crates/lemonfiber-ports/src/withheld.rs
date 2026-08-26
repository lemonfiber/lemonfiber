//! Keeping a credential out of anything an operator is shown.
//!
//! A service that fails while authenticating says so with the credential in hand, and that
//! sentence becomes an error's detail, a condition, and a notification. So the withholding
//! happens here, at the one place every such sentence passes through, rather than at the
//! dozens of call sites that produce them — a rule you have to remember at each of them is
//! a rule that will be forgotten at one.
//!
//! Beside the error model rather than beside the configuration file it was written for:
//! the model is what carries a service's own words outward, and it cannot reach up into a
//! layer above it to have them cleaned.
//!
//! Two shapes arrive here and they are told apart by shape. A settings line is a name, a
//! separator and a value, and its name is a name: one word, no spaces, with a value after
//! it rather than a clause. A sentence has a colon in it because English does. Reading the
//! front of a sentence as the name of what follows is how "the indexer refused the key:
//! your subscription has expired" became "the indexer refused the key: (set, not shown)" —
//! a diagnosis with its diagnosis removed, on every such error, every time — and how
//! "Unauthorized: the request was refused" lost its reason to its own first word.
//!
//! So the two directions are balanced differently on purpose. Where there is a name to
//! read, the name decides and withholding is the default — that is the settings surface,
//! and the allow-list in `lemonfiber-core`'s `config::display` is what answers it, through
//! [`withheld_by`]. Where there is no name, there is no list to consult and every rule is a
//! guess about somebody else's words: a guess that fires wrongly destroys the one sentence
//! the operator needed, so [`withheld`] fires only the narrow rules that read as
//! configuration wherever they appear.
//!
//! What a caller chooses is therefore the list, never the splitting: one line cannot be
//! read as a setting here and as a sentence at `/api/config`. A rule each call site
//! decides for itself is a rule most of them will decide differently.

const SECRET_MARKERS: &[&str] = &[
    "KEY",
    "PASS",
    "SECRET",
    "TOKEN",
    "PRIVATE",
    "CREDENTIAL",
    "AUTH",
];

/// What is shown in place of a secret.
pub const REDACTED: &str = "(set, not shown)";

/// Whether a setting's value must be withheld when configuration is displayed.
#[must_use]
pub fn is_secret(key: &str) -> bool {
    let upper = key.to_ascii_uppercase();
    SECRET_MARKERS.iter().any(|marker| upper.contains(marker))
}

/// Whether a run of text is written the way a setting is named.
///
/// One word: no whitespace, and nothing in it but the characters names are built from,
/// so `SONARR_API_KEY`, `api_key` and `X-Api-Key` are names and `the indexer refused
/// the key` is not. This is the whole of what separates the two shapes that arrive
/// here, and it is deliberately a question about the *front* of the line rather than
/// about the words in it: a sentence is not made into a name by containing one.
fn reads_as_name(text: &str) -> bool {
    !text.is_empty()
        && text
            .chars()
            .all(|character| character.is_alphanumeric() || matches!(character, '_' | '-' | '.'))
}

/// Whether a word reads as the name of a field rather than as a word in a sentence.
///
/// The question only prose has to ask. A word in a sentence is letters, capitalised at
/// most at its front — `key`, `Passengers`, `Authentication`. A field's name is spelled
/// like nothing anybody writes in a sentence: it carries a separator (`api_key`,
/// `X-Api-Key`), it shouts (`APIKEY`), or it changes case mid-word (`apiKey`).
///
/// Without this, every clause that happens to end on one of the marker words takes the
/// rest of its sentence with it, and the marker words are ordinary English — a check
/// that says "the indexer refused the key" says it about the key.
fn names_a_field(word: &str) -> bool {
    !word
        .char_indices()
        .all(|(at, character)| character.is_alphabetic() && (at == 0 || character.is_lowercase()))
}

/// A value with anything after its question mark withheld.
///
/// The shape that catches people out, and the one a name rule cannot see: an indexer's
/// address is worth showing and the key riding in its query string is not, and the two
/// arrive as one value. Which parameter holds it is not a question answerable from here
/// — a parameter's name belongs to whoever wrote the service, and `apikey` was only ever
/// caught by the accident of its holding `KEY`, while the `r=` a Newznab-family indexer
/// authenticates by and a `sid=` session were not caught at all.
///
/// So the query goes wholesale wherever the value turns up: on the settings surface, in
/// the URL a transport failure keeps, and inside a sentence that quotes one. A query
/// nobody reads is a smaller loss than a key everybody can.
#[must_use]
pub fn without_query(value: &str) -> String {
    match value.split_once('?') {
        None => value.to_owned(),
        Some((address, _)) => format!("{address}?{REDACTED}"),
    }
}

/// A value with both of the things a URL can carry a credential in withheld.
///
/// The query is one, and the other is the password a URL may carry in front of its host.
/// Nothing in this stack hands one out — an indexer of the Torznab and Newznab families
/// authenticates by a query parameter, which is why the key is a setting of its own — but
/// an operator whose indexer sits behind a proxy that asks for a login can write one, and
/// the client this stack sends with will use it.
///
/// What separates it from the query is that no guessing is involved. The rule above takes
/// a query wholesale precisely because a parameter's name belongs to whoever wrote the
/// service; here the URI syntax itself says that everything after the first colon of a
/// userinfo is a password, and that no application should render one as clear text. So
/// this is a fact about the shape rather than a guess about somebody's vocabulary.
///
/// The username stays. It names the account, an operator whose login is refused needs to
/// see which one, and it is not what the syntax calls a password.
#[must_use]
pub fn without_credentials(value: &str) -> String {
    without_query(&without_password(value))
}

/// The same value with the password half of any userinfo in it withheld, or the value
/// itself where there is none to withhold.
fn without_password(value: &str) -> String {
    password_withheld(value).unwrap_or_else(|| value.to_owned())
}

/// The rebuilt value, where this one is a URL carrying a password before its host.
///
/// Read out of the authority alone — what stands between `://` and the first `/`, `?` or
/// `#` — so a colon in a path and a stray `@` in a query are not mistaken for a login.
/// The last `@` in it rather than the first, because that is the one every client splits
/// on, and splitting anywhere else would leave part of the password in what is shown.
fn password_withheld(value: &str) -> Option<String> {
    let (scheme, rest) = value.split_once("://")?;
    let ends = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let at = rest.get(..ends)?.rfind('@')?;
    let (user, _) = rest.get(..at)?.split_once(':')?;
    Some(format!("{scheme}://{user}:{REDACTED}{}", rest.get(at..)?))
}

/// Whether what stands after a separator is the marker itself, left by an earlier pass
/// over the same text.
///
/// Text arrives here more than once — a service's words are laundered where they are
/// read and again where the report carrying them is serialised — and the marker is not a
/// value. Read as one it is withheld in half, and `api_key: (set, not shown)` comes back
/// as `api_key: (set, not shown) not shown)` on every pass after the first.
fn already_withheld(value: &str) -> bool {
    REDACTED.starts_with(value.trim())
}

/// Whether a name and what follows it read as a setting rather than as the front of a
/// sentence.
///
/// A name spelled like a field is a field wherever it appears. A name spelled like a
/// word — `Unauthorized`, `Authentication` — is one only where what follows is a value
/// rather than a clause, and a clause is several words: English introduces one with a
/// colon, and the marker words are ordinary English. Without the second half of this,
/// `Unauthorized: the request was refused by the indexer` came back as
/// `Unauthorized: (set, not shown)` — a sentence eaten by its own first word, and every
/// service that answers `401` writes that word.
///
/// Both halves are load-bearing. A credential is written under a lower-case name often
/// enough — `password: …`, `api_key: …` — that the first alone would print one, and a
/// value with a space in it is a value still, so the second alone would print whatever
/// part of it did not sit next to the separator.
fn reads_as_setting(name: &str, value: &str) -> bool {
    names_a_field(name) || value.split_whitespace().count() == 1
}

/// One line of prose as it is safe to show, with the marker rule answering about names.
///
/// The door for text no allow-list can answer for: the technical detail under an error,
/// a condition, a log line quoted back. A terminal, its scrollback and any bug report
/// pasted out of it are all the same place as far as a key is concerned — somewhere it
/// now has to be rotated from.
#[must_use]
pub fn withheld(line: &str) -> String {
    withheld_by(line, &|name| !is_secret(name))
}

/// One line as it is safe to show: a setting whose name is not vouched for keeps its
/// name and loses its value.
///
/// The separator is whichever of `:` or `=` comes **first**, because either can
/// appear inside the other's value — a password containing a colon, a key containing
/// an equals — and splitting on the later one would leave part of the value in the
/// name and print it.
///
/// What stands before that separator is taken as a name only where it *is* one, and
/// only where what follows it is a value rather than a clause. Text arriving here is as
/// often a sentence as a setting, and a sentence's first colon is punctuation: taking
/// the clause in front of it as the name of what follows withholds the message instead
/// of the credential, and the message is what the operator came for. A sentence goes to
/// [`withheld_within`] instead.
///
/// Which names keep their values is the one question this cannot answer for itself, so
/// `vouched_for` answers it, and the surface decides which answer it gets. A settings
/// file is read against the allow-list in `lemonfiber-core`'s `config::display`, where a
/// name nobody has argued for is withheld — including a name that does not exist yet.
/// Prose is read against the marker rule by [`withheld`], because there is no list to
/// consult about somebody else's sentence. Two surfaces, one splitter: a line cannot be
/// withheld one way here and another way at `/api/config`.
#[must_use]
pub fn withheld_by(line: &str, vouched_for: &dyn Fn(&str) -> bool) -> String {
    let Some((at, separator)) = line
        .char_indices()
        .find(|(_, character)| *character == ':' || *character == '=')
    else {
        return line.to_owned();
    };
    let (name, rest) = line.split_at(at);
    let value = rest.get(separator.len_utf8()..).unwrap_or_default();
    // A name with nothing after it opens a block rather than setting a value: there
    // is nothing to withhold, and blanking it would corrupt the shape. Nor is there
    // anything left to do where an earlier pass already withheld the value.
    if value.trim().is_empty() || already_withheld(value) {
        return line.to_owned();
    }
    let named = name.trim();
    if !reads_as_name(named) || !reads_as_setting(named, value) || vouched_for(named) {
        // Not a setting line, or a setting whose value is vouched for — but prose can
        // still carry a credential, and so can a value that is an address: a service
        // that fails while authenticating says so with the credential in hand, and that
        // sentence becomes an error detail, a condition, and a push notification.
        return withheld_within(line);
    }
    format!("{name}{separator} {REDACTED}")
}

/// A line of prose, with any credential embedded in it withheld.
///
/// Scans for the shapes a credential takes when a service quotes one back — a query
/// string, `api_key=abc123`, `api_key:abc123`, `api_key: abc123` — rather than treating
/// the whole line as one setting.
///
/// A query string is taken wholesale, because that is where the key nobody spotted
/// actually lives, riding inside something that reads as an address. The rest are a
/// *field* written out mid-sentence, and that is what those rules look for. The two
/// joined shapes need nothing more: prose does not put an equals sign or an internal
/// colon inside a word, so finding one is already finding a setting. The spaced shape
/// does need more, because a word followed by a colon is how English introduces a
/// clause, and the marker words are ordinary English — `key`, `password`, `auth`. So
/// there the marker has to be spelled like a field's name rather than like a word, and
/// a clause keeps its sentence.
///
/// The balance is struck the other way here than on the settings surface, and for a
/// reason that is not a preference: there, a name is read against a list, and a value
/// nobody has vouched for is withheld. Here there is no name and no list, only somebody
/// else's sentence, and a rule firing on the wrong word does not cost a reader a lookup
/// — it deletes the only account of what went wrong that the operator is ever shown.
fn withheld_within(line: &str) -> String {
    let mut safe: Vec<String> = Vec::new();
    let mut redact_next = false;
    for token in line.split_whitespace() {
        if redact_next {
            redact_next = false;
            safe.push(marked(token));
            continue;
        }
        if let Some(named) = joined(token).or_else(|| queried(token)) {
            safe.push(named);
            continue;
        }
        // `api_key: abc123` — the value is the next token along, and only where what
        // stands in front of the colon is a field's name and not a sentence's word.
        redact_next = token
            .strip_suffix(':')
            .is_some_and(|marker| is_secret(marker) && names_a_field(marker));
        safe.push(token.to_owned());
    }
    // Rebuilt from tokens, so the original spacing is not preserved; a line that
    // needed nothing withheld is returned untouched rather than reflowed.
    let rebuilt = safe.join(" ");
    if rebuilt.contains(REDACTED) {
        rebuilt
    } else {
        line.to_owned()
    }
}

/// The value standing where one was announced, withheld — or left as it is where an
/// earlier pass already withheld it and this one is reading the marker back.
fn marked(token: &str) -> String {
    if already_withheld(token) {
        token.to_owned()
    } else {
        REDACTED.to_owned()
    }
}

/// One token that is a whole setting — `api_key=abc123` or `api_key:abc123` — with its
/// value withheld, or nothing where the token is not one.
///
/// No `names_a_field` test on these two: a token carrying its own separator is already
/// configuration wherever it turns up, and English does not write `key=` or `key:` into
/// the middle of a word. The name is taken as it was written, punctuation and all, so a
/// quoted `'api_key=abc123'` is still caught.
fn joined(token: &str) -> Option<String> {
    let (name, value, separator) = match (token.split_once('='), token.split_once(':')) {
        (Some((name, value)), _) => (name, value, '='),
        (None, Some((name, value))) => (name, value, ':'),
        (None, None) => return None,
    };
    (is_secret(name) && !value.is_empty() && !already_withheld(value))
        .then(|| format!("{name}{separator}{REDACTED}"))
}

/// One token carrying a query string, with the query withheld wholesale — or nothing
/// where the token carries none.
///
/// Asked after [`joined`] and answering what it cannot: a key is reached by name only
/// where it is the first parameter, because the name read out of
/// `https://indexer.example/api?t=search&apikey=…` is `https://indexer.example/api?t`,
/// which holds no marker and vouches for everything after it. lemonfiber builds that
/// exact address to prove an indexer, and the services around it log the address they
/// failed on.
///
/// Parameters and not a question mark on its own: a question mark with nothing that
/// reads as a parameter after it is somebody asking a question in a log line.
///
/// A password written in front of the host goes with the query, since a service quoting
/// an address back at itself quotes whatever was configured into it.
fn queried(token: &str) -> Option<String> {
    let (_, query) = token.split_once('?')?;
    query.contains('=').then(|| without_credentials(token))
}

/// Every line of `text`, each withheld where it carries a credential.
#[must_use]
pub fn withheld_text(text: &str) -> String {
    text.lines().map(withheld).collect::<Vec<_>>().join("\n")
}

#[cfg(test)]
mod tests {
    use super::{
        is_secret, withheld, withheld_by, withheld_text, without_credentials, without_query,
        REDACTED,
    };

    /// A stand-in for a credential, assembled rather than written out so no value
    /// that reads as one sits in this source.
    fn a_credential() -> String {
        ["abcdef", "1234", "567890"].concat()
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
        for line in [
            format!("INDEXER_APIKEY={secret}"),
            format!("  USENET_PASS: {secret}"),
            format!("      SONARR_API_KEY: {secret}:more"),
            format!("WIREGUARD_PRIVATE_KEY={secret}=more"),
            format!("PROVIDER_CREDENTIAL={secret}"),
            // A name is a name whatever it is spelled with, so long as it is one word.
            format!("homepage-var-jellyfin-key={secret}"),
        ] {
            let shown = withheld(&line);
            assert!(shown.ends_with(REDACTED), "{line} -> {shown}");
            assert!(!shown.contains(&secret), "{line} -> {shown}");
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
            assert!(!shown.contains(&secret), "{line} -> {shown}");
            assert!(shown.contains(REDACTED), "{line} -> {shown}");
            assert!(shown.contains(survives), "{line} -> {shown}");
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
        assert!(shown.contains("it expired"), "{shown}");
        assert!(!shown.contains(&secret), "{shown}");
    }

    #[test]
    fn a_key_that_is_not_the_first_parameter_of_an_address_does_not_ride_out_in_it() {
        // The live shape. `validate` builds `?t=search&apikey=…` to prove an indexer,
        // and the services around it quote the address they failed on into the lines
        // that become an error's detail. Read as a name, what stands in front of the
        // first equals sign of that address is `https://indexer.example/api?t` — no
        // marker in it, so it vouched for everything behind it, key included.
        let secret = a_credential();
        let line = format!(
            "sonarr: GET https://indexer.example/api?t=search&q=foo&apikey={secret} failed"
        );
        let shown = withheld(&line);

        assert!(!shown.contains(&secret), "{line} -> {shown}");
        // And what the sentence was for survives: which service, which address, and
        // that the request failed.
        assert!(shown.contains("sonarr: GET"), "{line} -> {shown}");
        assert!(
            shown.contains("https://indexer.example/api?"),
            "{line} -> {shown}"
        );
        assert!(shown.ends_with("failed"), "{line} -> {shown}");
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
        for line in [
            format!("password: {secret}"),
            format!("api_key: {secret}"),
            format!("token={secret}"),
        ] {
            let shown = withheld(&line);
            assert!(!shown.contains(&secret), "{line} -> {shown}");
            assert!(shown.contains(REDACTED), "{line} -> {shown}");
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
        assert!(shown.starts_with("https://indexer.example/api?"), "{shown}");
        assert!(!shown.contains(&key), "{shown}");
    }

    #[test]
    fn a_password_written_in_front_of_a_host_is_withheld_and_the_account_is_not() {
        // An indexer behind a proxy that asks for a login, written the one way a URL can
        // carry one. The host, the path and the account survive, because each of those is
        // what somebody checks when a login is refused.
        let secret = a_credential();
        let shown = without_credentials(&format!("https://operator:{secret}@indexer.example/api"));
        assert!(!shown.contains(&secret), "{shown}");
        assert!(shown.starts_with("https://operator:"), "{shown}");
        assert!(shown.ends_with("@indexer.example/api"), "{shown}");
    }

    #[test]
    fn a_password_goes_whether_or_not_the_address_also_carries_a_query() {
        // Both halves on one value, and the sentence around it kept: this is the shape a
        // service logs when the address it was configured with was refused.
        let secret = a_credential();
        let shown = withheld(&format!(
            "prowlarr refused https://operator:{secret}@indexer.example/api?t=caps&apikey={secret}"
        ));
        assert!(!shown.contains(&secret), "{shown}");
        assert!(
            shown.starts_with("prowlarr refused https://operator:"),
            "{shown}"
        );
        assert!(shown.contains("@indexer.example/api?"), "{shown}");
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
}
