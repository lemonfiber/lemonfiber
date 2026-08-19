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

/// One line as it is safe to show: a setting whose name reads as a credential keeps
/// its name and loses its value.
///
/// Wherever text that might carry a credential reaches a person — a diff of a file
/// they edited, the technical detail under an error, a log line quoted back — it
/// comes through here. A terminal, its scrollback and any bug report pasted out of
/// it are all the same place as far as a key is concerned: somewhere it now has to
/// be rotated from.
///
/// The separator is whichever of `:` or `=` comes **first**, because either can
/// appear inside the other's value — a password containing a colon, a key containing
/// an equals — and splitting on the later one would leave part of the value in the
/// name and print it.
#[must_use]
pub fn withheld(line: &str) -> String {
    let Some((at, separator)) = line
        .char_indices()
        .find(|(_, character)| *character == ':' || *character == '=')
    else {
        return line.to_owned();
    };
    let (name, rest) = line.split_at(at);
    let value = rest.get(separator.len_utf8()..).unwrap_or_default();
    // A name with nothing after it opens a block rather than setting a value: there
    // is nothing to withhold, and blanking it would corrupt the shape.
    if value.trim().is_empty() {
        return line.to_owned();
    }
    if !is_secret(name) {
        // Not a setting line, but prose can still carry one: a service that fails
        // while authenticating says so with the credential in hand, and that
        // sentence becomes an error detail, a condition, and a push notification.
        return withheld_within(line);
    }
    format!("{name}{separator} {REDACTED}")
}

/// A line of prose, with any credential embedded in it withheld.
///
/// Scans for the two shapes a credential takes when a service quotes one back —
/// `api_key=abc123` and `api_key: abc123` — rather than treating the whole line as
/// one setting. Over-redacting a word that merely reads like a key costs a reader
/// one lookup; under-redacting costs a rotation.
fn withheld_within(line: &str) -> String {
    let mut safe: Vec<String> = Vec::new();
    let mut redact_next = false;
    for token in line.split_whitespace() {
        if redact_next {
            redact_next = false;
            safe.push(REDACTED.to_owned());
            continue;
        }
        match token.split_once('=') {
            Some((name, value)) if is_secret(name) && !value.is_empty() => {
                safe.push(format!("{name}={REDACTED}"));
                continue;
            }
            _ => {}
        }
        // `api_key: abc123` — the value is the next token along.
        redact_next = token.strip_suffix(':').is_some_and(is_secret);
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

/// Every line of `text`, each withheld where it carries a credential.
#[must_use]
pub fn withheld_text(text: &str) -> String {
    text.lines().map(withheld).collect::<Vec<_>>().join("\n")
}
