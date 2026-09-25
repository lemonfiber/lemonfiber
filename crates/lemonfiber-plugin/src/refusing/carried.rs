//! What a manifest's values may be, where lemonfiber writes them into a file
//! something else reads.
//!
//! A plugin supplies no container definition, no proxy stanza and no dashboard
//! entry, and lemonfiber writes all three from what it declared. That keeps the
//! *keys* out of a plugin's hands; it does not keep the *values* in them harmless.
//! Each of the three files has a reader with its own grammar — Compose reads YAML
//! and then substitutes `${…}` from the stack's environment, the proxy reads site
//! blocks separated by braces and line breaks, the dashboard substitutes `{{…}}`
//! from its own environment — and a value carrying that grammar's punctuation is a
//! value that has stopped being a value.
//!
//! So each value that reaches one of them is held to the smallest alphabet that
//! says what it is, rather than to a list of characters somebody thought of:
//!
//! - a **service id** names a container on the stack's network, the directory its
//!   configuration is kept in and the key its entry is written under, so it is one
//!   DNS label;
//! - a **hostname** is the label in front of the operator's domain, so it is one
//!   DNS label;
//! - an **image** is a registry path, so it is the registry's own reference grammar;
//! - a **configuration directory** is a path inside the container, so it is letters,
//!   digits and `._/-`;
//! - a **request path** is a route on the service the plugin installed, so it starts
//!   at that service's root and carries nothing that could name another host.
//!
//! The prose that reaches the dashboard — a service's name, the plugin's
//! description, a group — has no alphabet to be held to, so what is refused there is
//! exactly what a reader downstream would act on: `$` and `{{`.
//!
//! The rules are public because the reader is not the only place they are asked. A
//! record the register reads back and a request the core is about to send are both
//! held to the same definitions, so there is one answer to what a service id may be.

use crate::schema::{Manifest, Request};
use crate::Violation;

/// The longest a DNS label may be.
const LABEL: usize = 63;

/// The longest a registry path may be, as the registry's own grammar bounds it.
const REFERENCE: usize = 255;

/// The longest a registry port may be written.
const PORT: usize = 5;

/// Whether `text` is a plugin id: lowercase letters, digits and hyphens, and at
/// least one of them.
///
/// It is the namespace every capability and contribution the plugin declares is
/// prefixed with, and the name its Compose document is kept under, so a separator
/// in it would let two plugins write one identity or one file.
#[must_use]
pub fn is_plugin_id(text: &str) -> bool {
    !text.is_empty()
        && text
            .chars()
            .all(|letter| letter.is_ascii_lowercase() || letter.is_ascii_digit() || letter == '-')
}

/// Whether `text` is one DNS label: lowercase letters, digits and hyphens, at most
/// sixty-three of them, starting and ending with a letter or a digit.
///
/// Lowercase only, although DNS is not case-sensitive: the label is also a container
/// name and a directory name, and a service written `Komga` in one place and `komga`
/// in another would be two directories on a case-sensitive disk and one address.
#[must_use]
pub fn is_label(text: &str) -> bool {
    let letters = |letter: char| letter.is_ascii_lowercase() || letter.is_ascii_digit();
    !text.is_empty()
        && text.len() <= LABEL
        && text.chars().all(|letter| letters(letter) || letter == '-')
        && text.starts_with(letters)
        && text.ends_with(letters)
}

/// Whether `text` is a registry path as a registry reads one, with no tag and no
/// digest after it.
///
/// The grammar the container ecosystem publishes for a reference's name: an optional
/// registry host (with an optional port) followed by one or more path components,
/// each lowercase letters and digits joined by `.`, `_`, `__` or a run of `-`. The
/// first segment is a host only where it could not be a path component — it carries
/// a `.`, a `:` or an uppercase letter, or it is `localhost` — which is how a registry
/// client decides the same question.
#[must_use]
pub fn is_reference(text: &str) -> bool {
    if text.len() > REFERENCE {
        return false;
    }
    let mut segments = text.split('/');
    let first = segments.next().unwrap_or_default();
    let rest: Vec<&str> = segments.collect();
    let hosted = !rest.is_empty()
        && (first.contains('.')
            || first.contains(':')
            || first == "localhost"
            || first.chars().any(|letter| letter.is_ascii_uppercase()));
    let path_ok = rest.iter().all(|segment| is_component(segment));
    if hosted {
        is_host(first) && path_ok
    } else {
        is_component(first) && path_ok
    }
}

/// One path component of a registry reference.
fn is_component(text: &str) -> bool {
    let mut joining = String::new();
    let mut started = false;
    for letter in text.chars() {
        if letter.is_ascii_lowercase() || letter.is_ascii_digit() {
            if !joining.is_empty() && !joins(&joining) {
                return false;
            }
            joining.clear();
            started = true;
        } else if started && matches!(letter, '.' | '_' | '-') {
            joining.push(letter);
        } else {
            return false;
        }
    }
    started && joining.is_empty()
}

/// Whether a run of separators is one the grammar joins two components with.
fn joins(run: &str) -> bool {
    matches!(run, "." | "_" | "__") || run.chars().all(|letter| letter == '-')
}

/// A registry host, with the port it answers on where one is written.
fn is_host(text: &str) -> bool {
    let (host, port) = text
        .split_once(':')
        .map_or((text, None), |(host, port)| (host, Some(port)));
    let port_ok = port.is_none_or(|port| {
        !port.is_empty() && port.len() <= PORT && port.chars().all(|digit| digit.is_ascii_digit())
    });
    port_ok && !host.is_empty() && host.split('.').all(is_host_label)
}

/// One dot-separated part of a registry host.
fn is_host_label(text: &str) -> bool {
    !text.is_empty()
        && text
            .chars()
            .all(|letter| letter.is_ascii_alphanumeric() || letter == '-')
        && !text.starts_with('-')
        && !text.ends_with('-')
}

/// Whether `text` is a `sha256:` content address of sixty-four hexadecimal
/// characters, which is what fixes the bytes that run.
#[must_use]
pub fn is_digest(text: &str) -> bool {
    let (prefix, length) = super::DIGEST;
    text.strip_prefix(prefix).is_some_and(|after| {
        after.len() == length && after.chars().all(|letter| letter.is_ascii_hexdigit())
    })
}

/// Whether `text` is written only in what a configuration directory may carry:
/// letters, digits, `.`, `_`, `/` and `-`.
///
/// Whether it is absolute, the root, or inside the library are the reader's other
/// rules; this is only the alphabet, which is what keeps it one value in the file it
/// is written into.
#[must_use]
pub fn is_directory(text: &str) -> bool {
    text.chars()
        .all(|letter| letter.is_ascii_alphanumeric() || matches!(letter, '.' | '_' | '/' | '-'))
}

/// Whether `text` is a route on the service it is sent to, and nothing more.
///
/// It is appended to the address lemonfiber resolved for that service, so it has to
/// start at that service's root: a leading `/` is what ends the address before it,
/// and without one the text is still part of the address. What would let it name
/// another place from inside a route is refused too — `@` (credentials before a
/// host), `\` (read as `/` by some clients), `#` (the rest of the line never sent),
/// whitespace and anything a diff cannot show.
#[must_use]
pub fn is_route(text: &str) -> bool {
    text.starts_with('/')
        && !text.chars().any(|letter| {
            matches!(letter, '@' | '\\' | '#') || letter.is_whitespace() || letter.is_control()
        })
}

/// What in `text` a reader downstream would substitute, where anything is.
///
/// `$` is Compose's, which substitutes from the stack's environment file — the one
/// holding every key the stack has. `{{` is the dashboard's, which substitutes from
/// its own environment, which carries the stack's API keys so its widgets can read.
#[must_use]
pub fn substituted(text: &str) -> Option<&'static str> {
    if text.contains('$') {
        Some("$")
    } else if text.contains("{{") {
        Some("{{")
    } else {
        None
    }
}

/// Every value a manifest carries into a file something else reads, held to what it
/// may be.
pub(super) fn carried(manifest: &Manifest, found: &mut Vec<Violation>) {
    for service in &manifest.services {
        let at = format!("service {}", service.id);
        if !is_label(&service.id) {
            found.push(Violation {
                location: format!("{at}.id"),
                message: format!(
                    "{} is not one DNS label of lowercase letters, digits and hyphens; it names \
                     the container on the stack's network, the directory its configuration is \
                     kept in and the entry it is written under",
                    service.id
                ),
            });
        }
        // A tag or a digest is already refused by name, and a second refusal of the
        // same text for its grammar would be two reasons for one mistake.
        let pinned_here = service.image.contains('@')
            || service.image.rsplit('/').next().is_some_and(super::has_tag);
        if !pinned_here && !is_reference(&service.image) {
            found.push(Violation {
                location: format!("{at}.image"),
                message: format!(
                    "{} is not a registry path; what is permitted is an optional registry host \
                     and lowercase path components joined by `/`, which is the whole of what a \
                     registry reads there",
                    service.image
                ),
            });
        }
        prose(&format!("{at}.name"), &service.name, found);
    }
    prose("plugin.description", &manifest.plugin.description, found);
    for (at, wiring) in manifest.wirings.iter().enumerate() {
        let location = wiring.service.as_deref().map_or_else(
            || format!("wiring #{}", at + 1),
            |id| format!("wiring {id}"),
        );
        if let Some(hostname) = &wiring.hostname {
            if !is_label(hostname) {
                found.push(Violation {
                    location: format!("{location}.hostname"),
                    message: format!(
                        "{hostname} is not one DNS label of lowercase letters, digits and \
                         hyphens; it is the label in front of the operator's domain, and \
                         anything else there is written into the proxy's own configuration"
                    ),
                });
            }
        }
        if let Some(group) = &wiring.dashboard_group {
            prose(&format!("{location}.dashboard_group"), group, found);
        }
    }
    for (at, request) in requests(manifest) {
        if !is_route(&request.path) {
            found.push(Violation {
                location: format!("{at}.request.path"),
                message: format!(
                    "{} is not a route on the service it asks; it has to start with `/` and \
                     carry no `@`, `\\`, `#` or whitespace, because it is appended to the \
                     address lemonfiber resolved and anything else could name another host",
                    request.path
                ),
            });
        }
    }
}

/// Prose a reader downstream substitutes into, refused where it would.
fn prose(at: &str, text: &str, found: &mut Vec<Violation>) {
    if let Some(what) = substituted(text) {
        found.push(Violation {
            location: at.to_owned(),
            message: format!(
                "carries `{what}`, which the file it is written into substitutes from an \
                 environment holding the stack's keys; a plugin's text is shown, never expanded"
            ),
        });
    }
}

/// Every request a manifest would have lemonfiber send, with where it was declared.
fn requests(manifest: &Manifest) -> Vec<(String, &Request)> {
    let mut every = Vec::new();
    for claim in &manifest.claims {
        for probe in &claim.probes {
            every.push((
                format!("claim {} probe {}", claim.capability, probe.id),
                &probe.request,
            ));
        }
    }
    for proof in &manifest.proofs {
        every.push((format!("proof {}", proof.id), &proof.request));
    }
    for entry in &manifest.contributions {
        if let Some(request) = &entry.request {
            every.push((format!("contribution {}", entry.id), request));
        }
    }
    every
}

#[cfg(test)]
mod tests;
