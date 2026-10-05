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

/// The words that name a credential itself: a setting called by one is secret, and a
/// path segment after one is the credential it names.
const KEY_MARKERS: &[&str] = &["KEY", "PASS", "SECRET", "TOKEN", "PRIVATE", "CREDENTIAL"];

/// The words that name the account or the sign-in a credential belongs to: a setting
/// called by one is secret too.
///
/// A path segment after one is not withheld, because a path names accounts and sign-ins
/// by their routes: what follows `/Users/` is an account and what follows `/auth/` is
/// `login`, and withholding either hides the route somebody came to read.
const ACCOUNT_MARKERS: &[&str] = &["AUTH", "USER", "SESSION", "COOKIE"];

/// Names that are a credential only as a whole word, being too short to look for
/// inside a longer one: a session cookie is called `SID`, and `INSIDE` is not one.
const SECRET_WORDS: &[&str] = &["SID"];

/// Headers whose whole value is a credential, whatever it is written in.
///
/// A scheme comes first and the credential second — `Bearer …`, `Basic …` — and a
/// cookie line is a list of pairs, so neither reads as a setting with one value after
/// it. The rest of the line goes, rather than the next word.
const SECRET_HEADERS: &[&str] = &[
    "AUTHORIZATION",
    "PROXY-AUTHORIZATION",
    "COOKIE",
    "SET-COOKIE",
];

/// The shortest run that reads as a key where it stands as a path segment of its own.
///
/// Longer than any word or version a path is built from, and shorter than the keys
/// and passkeys services put there: thirty-two hexadecimal digits is the usual shape.
const KEY_LENGTH: usize = 20;

/// What is shown in place of a secret.
pub const REDACTED: &str = "(set, not shown)";

/// Whether a setting's value must be withheld when configuration is displayed.
#[must_use]
pub fn is_secret(key: &str) -> bool {
    let upper = key.to_ascii_uppercase();
    KEY_MARKERS
        .iter()
        .chain(ACCOUNT_MARKERS)
        .any(|marker| upper.contains(marker))
        || upper
            .split(|character: char| !character.is_ascii_alphanumeric())
            .any(|word| SECRET_WORDS.contains(&word))
}

/// Whether a name is a header whose whole value is a credential.
fn hides_the_line(name: &str) -> bool {
    SECRET_HEADERS.contains(&name.to_ascii_uppercase().as_str())
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
pub(crate) fn without_query(value: &str) -> String {
    match value.split_once('?') {
        None => value.to_owned(),
        Some((address, _)) => format!("{address}?{REDACTED}"),
    }
}

/// A value with every place a URL can carry a credential in withheld: the query, the
/// login in front of the host, and a path segment that is a key.
///
/// The login goes whole rather than only the password after its colon. A service
/// reached as `https://<token>@host` authenticates by the name alone, so a rule that
/// knew only about passwords printed the token; the URI syntax says where the login
/// stands, so no guessing is involved in finding it.
///
/// A path segment is the one place here that is read by shape. A key is put in a path
/// where an API names the thing it acts on by it — `DELETE /Auth/Keys/<key>` revokes
/// that key — and a tracker's announce address carries a passkey the same way. So a
/// segment goes where the one in front of it names a credential, and where it is long
/// and dense enough to read as one on its own.
#[must_use]
pub fn without_credentials(value: &str) -> String {
    without_query(&without_path_keys(&without_login(value)))
}

/// Where a URL carries a login in front of its host: what stands before `://`, the
/// login, and the rest of the value from its `@` on.
///
/// Read out of the authority alone — what stands between `://` and the first `/`, `?` or
/// `#` — so an `@` in a path or a query is not mistaken for a login. The last `@` in it
/// rather than the first, because that is the one every client splits on.
///
/// Public because a support bundle marks a login rather than withholding it, and must
/// find it where this does rather than by a rule of its own.
#[must_use]
pub fn login(value: &str) -> Option<(&str, &str, &str)> {
    let (scheme, rest) = value.split_once("://")?;
    let ends = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let at = rest.get(..ends)?.rfind('@')?;
    Some((scheme, rest.get(..at)?, rest.get(at..)?))
}

/// The same value with any login in front of its host withheld, or the value itself
/// where there is none.
fn without_login(value: &str) -> String {
    login(value).map_or_else(
        || value.to_owned(),
        |(scheme, _, after)| format!("{scheme}://{REDACTED}{after}"),
    )
}

/// The same value with every path segment that is a key withheld.
fn without_path_keys(value: &str) -> String {
    let Some((scheme, rest)) = value.split_once("://") else {
        return value.to_owned();
    };
    let ends = rest.find(['?', '#']).unwrap_or(rest.len());
    let (address, tail) = rest.split_at(ends);
    let Some((authority, path)) = address.split_once('/') else {
        return value.to_owned();
    };
    let mut after_a_name = false;
    let segments: Vec<&str> = path
        .split('/')
        .map(|segment| {
            let named = names_a_key(segment);
            let keyed = (after_a_name && !named) || reads_as_key(segment);
            after_a_name = named;
            if keyed && !segment.is_empty() && !already_withheld(segment) {
                REDACTED
            } else {
                segment
            }
        })
        .collect();
    format!("{scheme}://{authority}/{}{tail}", segments.join("/"))
}

/// Whether a path segment names a credential, so the segment after it is that credential.
fn names_a_key(segment: &str) -> bool {
    let upper = segment.to_ascii_uppercase();
    KEY_MARKERS.iter().any(|marker| upper.contains(marker))
}

/// Whether a path segment reads as a key rather than as a name: long, built only of
/// letters and digits, and carrying both.
fn reads_as_key(segment: &str) -> bool {
    segment.len() >= KEY_LENGTH
        && segment
            .chars()
            .all(|character| character.is_ascii_alphanumeric())
        && segment.chars().any(|character| character.is_ascii_digit())
        && segment
            .chars()
            .any(|character| character.is_ascii_alphabetic())
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
    if hides_the_line(named) {
        return format!("{name}{separator} {REDACTED}");
    }
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
/// actually lives, riding inside something that reads as an address. An address with no
/// query is asked the other questions [`without_credentials`] answers — whether it
/// carries a login in front of its host, or a key in its path — because [`queried`]
/// reaches those only where there is a query to strip. `http://host:8080/path`, which
/// has neither, is left alone. A header whose value is a credential takes the rest of
/// the line with it, since its scheme and its pairs are words a setting rule cannot
/// read. The rest are a *field* written out mid-sentence, and that is what those rules
/// look for. The two joined shapes need nothing more: prose does not put an equals sign or an internal
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
        // `Authorization: Bearer …` — the scheme is a word and the credential the next
        // one, and a cookie line is a list, so everything after the header goes.
        if token.strip_suffix(':').is_some_and(hides_the_line) {
            safe.push(token.to_owned());
            safe.push(REDACTED.to_owned());
            break;
        }
        if let Some(named) = joined(token)
            .or_else(|| queried(token))
            .or_else(|| addressed(token))
        {
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
/// A login in front of the host and a key in the path go with the query, since a
/// service quoting an address back at itself quotes whatever was configured into it.
fn queried(token: &str) -> Option<String> {
    let (_, query) = token.split_once('?')?;
    query.contains('=').then(|| without_credentials(token))
}

/// One token that is an address carrying a login or a key in its path, with those
/// withheld — or nothing where it carries neither.
fn addressed(token: &str) -> Option<String> {
    let shown = without_path_keys(&without_login(token));
    (shown != token).then_some(shown)
}

/// Every line of `text`, each withheld where it carries a credential.
#[must_use]
pub fn withheld_text(text: &str) -> String {
    text.lines().map(withheld).collect::<Vec<_>>().join("\n")
}

#[cfg(test)]
mod tests;
