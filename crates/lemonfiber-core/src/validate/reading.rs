//! What a service's answer means.
//!
//! The pure half of proving a credential: given a status and a body, is this a valid key,
//! a rejected one, or a service that could not be asked? Three protocols answer three
//! different ways — a Servarr status, an NNTP greeting, a Torznab search — and each is read
//! on its own terms rather than through a shared guess.
//!
//! Nothing here reaches a service, so every case runs in a test with no network.

use super::Validation;

/// Read a service's answer to an authenticated identity request into an outcome.
///
/// What a transport failure amounts to, in the operator's terms.
///
/// The transport's own words lead, as they must — they are the only account of
/// what actually happened. What lemonfiber adds is whether it kept happening: a
/// service that did not answer once may have been busy, and one that did not
/// answer every time it was asked is down, and those are different things to do
/// about.
pub(crate) fn persisting(unreachable: &crate::ports::http::Unreachable) -> String {
    match crate::retry::said(unreachable.attempts) {
        Some(persisted) => format!("{} — {persisted}", unreachable.reason),
        None => unreachable.reason.clone(),
    }
}

/// A refusing status is the key being wrong; a well-formed identity proves it and
/// carries what the service said about itself as the observed capability; any
/// other answer came from something that is not the service's API, which points
/// at the URL rather than the key.
pub(crate) fn interpret_service(status: u16, body: &str) -> Validation {
    if status == UNAUTHORIZED || status == FORBIDDEN {
        return Validation::Rejected {
            detail: format!("the service answered {status} — the key was refused"),
        };
    }
    if (200..300).contains(&status) {
        return Validation::Valid {
            observed: identity(body),
        };
    }
    Validation::Unreachable {
        detail: format!("the service answered {status}, not as a reachable API — check the URL"),
    }
}

/// Read a provider's replies to a login into an outcome.
///
/// The reply to the password is what decides it: accepted proves the login,
/// refused is a wrong username or password, and a permission or connection-limit
/// code is an account that authenticated but cannot serve right now. A greeting
/// that already turned the connection away — the provider at its limit — is that
/// same degraded case, told before the login was even reached. Too few replies to
/// have finished the exchange leave it unreachable.
pub(crate) fn interpret_usenet(replies: &[String]) -> Validation {
    // A provider that turns the connection away at the greeting never reaches the
    // login; its limit is the reason, not the credential.
    if matches!(replies.first().and_then(|line| code(line)), Some(400 | 502)) {
        return Validation::Degraded {
            detail: "the provider is at its connection limit; try again shortly".to_owned(),
        };
    }
    // Some providers accept at the username step and ask for no password; the login
    // is already proven, and the password reply that follows would read as an
    // out-of-sequence refusal if it were taken as the answer.
    if matches!(
        replies.get(1).and_then(|line| code(line)),
        Some(AUTH_ACCEPTED)
    ) {
        return Validation::Valid {
            observed: "the provider accepted the login".to_owned(),
        };
    }
    // The greeting, the reply to the username, then the reply to the password.
    let Some(password_reply) = replies.get(2) else {
        return Validation::Unreachable {
            detail: "the provider did not complete the login exchange".to_owned(),
        };
    };
    match code(password_reply) {
        Some(AUTH_ACCEPTED) => Validation::Valid {
            observed: "the provider accepted the login".to_owned(),
        },
        Some(AUTH_REJECTED | AUTH_OUT_OF_SEQUENCE) => Validation::Rejected {
            detail: "the provider refused the username or password".to_owned(),
        },
        Some(NO_PERMISSION) => Validation::Degraded {
            detail: "the provider accepted the account but would not serve it — usually its \
                     connection limit"
                .to_owned(),
        },
        Some(other) => Validation::Rejected {
            detail: format!("the provider answered {other} to the login"),
        },
        None => Validation::Unreachable {
            detail: "the provider did not answer the login in a way that could be read".to_owned(),
        },
    }
}

/// The NNTP reply code for an accepted authentication.
const AUTH_ACCEPTED: u16 = 281;

/// The code for a rejected authentication.
const AUTH_REJECTED: u16 = 481;

/// The code for an authentication command out of sequence — the login as a whole
/// did not take, so it reads the same as a refusal to the operator.
const AUTH_OUT_OF_SEQUENCE: u16 = 482;

/// The code for a command the account is not permitted — on a login, an account
/// that is known but cannot be served, typically at its connection limit.
const NO_PERMISSION: u16 = 502;

/// The three-digit status code an NNTP reply line begins with, where it begins
/// with one — the only part of the line an outcome turns on.
pub(crate) fn code(line: &str) -> Option<u16> {
    let digits = line.get(0..3)?;
    if digits.bytes().all(|byte| byte.is_ascii_digit()) {
        digits.parse().ok()
    } else {
        None
    }
}

/// What a service says about itself, from the identity it answered with.
///
/// A Servarr status names the instance and its version; either alone is worth
/// reporting, and where neither is there the fact that the key was accepted is
/// still the observation.
pub(crate) fn identity(body: &str) -> String {
    let parsed = serde_json::from_str::<serde_json::Value>(body).ok();
    let field = |name: &str| {
        parsed
            .as_ref()
            .and_then(|value| value.get(name))
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
    };
    match (field("instanceName"), field("version")) {
        (Some(name), Some(version)) => format!("reached {name}, version {version}"),
        (Some(name), None) => format!("reached {name}"),
        (None, Some(version)) => format!("the service accepted the key — version {version}"),
        (None, None) => "the service accepted the key".to_owned(),
    }
}

/// Read a Torznab or Newznab indexer's answer to a search into an outcome.
///
/// An error element is the indexer refusing the query, and its code says whether
/// that is the key (rejected) or a limit the account has hit (degraded, and
/// transient). A well-formed feed proves the key and carries how many results it
/// held as the observed capability. Anything else — a login page, an unrelated
/// site — answered without being the indexer, which points at the URL rather than
/// the key, so it is unreachable-for-this-purpose rather than a refusal.
pub(crate) fn interpret_indexer(status: u16, body: &str) -> Validation {
    if let Some(code) = error_attr(body, "code").and_then(|code| code.parse::<u32>().ok()) {
        let said = error_attr(body, "description").unwrap_or_else(|| "no reason given".to_owned());
        return match code {
            // The request-limit code is a rate-limit, not a wrong key: authenticated,
            // but temporarily unable, which is degraded and worth retrying.
            RATE_LIMITED => Validation::Degraded {
                detail: format!(
                    "the indexer is rate-limiting this key ({said}); try again shortly"
                ),
            },
            _ => Validation::Rejected {
                detail: format!("the indexer refused the key: {said}"),
            },
        };
    }

    if status == UNAUTHORIZED || status == FORBIDDEN {
        return Validation::Rejected {
            detail: format!("the indexer answered {status} — the key was refused"),
        };
    }

    let lower = body.to_ascii_lowercase();
    if lower.contains("<rss") || lower.contains("<channel") || lower.contains("<caps") {
        let results = lower.matches("<item").count();
        return Validation::Valid {
            observed: format!("answered a search — {results} result(s) offered"),
        };
    }

    Validation::Unreachable {
        detail: "answered, but not as a Torznab or Newznab indexer — check the URL".to_owned(),
    }
}

/// The Newznab error code for a request the account has rate-limited.
const RATE_LIMITED: u32 = 500;

/// The status a service returns when a credential is refused outright.
const UNAUTHORIZED: u16 = 401;

/// The status a service returns when a credential is known but not permitted.
const FORBIDDEN: u16 = 403;

/// The value of `attr="…"` (or `'…'`) on the first `<error` element in `body`, or
/// nothing where there is no such element or attribute.
///
/// A deliberately small reader rather than a full XML parse: the one element whose
/// shape matters here is the error the Newznab and Torznab specs both fix, and a
/// dependency to read one attribute off it would be its own liability.
pub(crate) fn error_attr(body: &str, attr: &str) -> Option<String> {
    let error = body.find("<error")?;
    let within = &body[error..];
    let at = within.find(attr)?;
    let after = within[at + attr.len()..].trim_start();
    let value = after.strip_prefix('=')?.trim_start();
    let quote = value.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let rest = &value[quote.len_utf8()..];
    let end = rest.find(quote)?;
    Some(rest[..end].to_owned())
}
