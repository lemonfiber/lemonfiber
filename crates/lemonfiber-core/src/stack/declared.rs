//! What a stack's Compose files declare: the services, and the host ports they publish.
//!
//! Read off the files as Compose reads them, across every file at once, because what an
//! operator runs is the stack and whatever they layered over it, and a service declared
//! in their overlay is as much a service as one the stack ships. Nothing here decides
//! what to do about any of it; it answers what is there.
//!
//! **A port is the host side of a mapping**, which is the side two services cannot
//! share. The address in front of it is not compared: one service on every interface
//! and another on loopback still meet at one port on this machine. A mapping published
//! over UDP is left out, because a plugin's port is TCP and the two do not meet.

use std::path::PathBuf;

use serde_yaml_ng::Value;

/// One service a Compose file declares, and the file that declares it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Declared {
    /// The service's name, which is what Compose merges a second declaration into.
    pub service: String,
    /// The file it is declared in.
    pub file: PathBuf,
}

/// One host port a Compose file publishes, and what publishes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Published {
    /// The port on this machine.
    pub port: u16,
    /// The service it is published for.
    pub service: String,
    /// The file it is published in.
    pub file: PathBuf,
}

/// Every service and every published host port these files declare.
///
/// `variable` answers what a `${NAME}` in a port is set to, where it is set at all; a
/// port still unresolved after that, and after its own default, says nothing about any
/// port and is left out.
#[must_use]
pub fn read(
    files: &[(PathBuf, String)],
    variable: &dyn Fn(&str) -> Option<String>,
) -> (Vec<Declared>, Vec<Published>) {
    let mut services = Vec::new();
    let mut ports = Vec::new();
    for (file, text) in files {
        for (service, body) in in_file(text) {
            for port in published(&body, variable) {
                ports.push(Published {
                    port,
                    service: service.clone(),
                    file: file.clone(),
                });
            }
            services.push(Declared {
                service,
                file: file.clone(),
            });
        }
    }
    (services, ports)
}

/// The services one file declares, each with its body, or none where it is not a
/// Compose file this can read.
fn in_file(text: &str) -> Vec<(String, Value)> {
    let Ok(mut document) = serde_yaml_ng::from_str::<Value>(text) else {
        return Vec::new();
    };
    if document.apply_merge().is_err() {
        return Vec::new();
    }
    document
        .get("services")
        .and_then(Value::as_mapping)
        .map(|services| {
            services
                .iter()
                .filter_map(|(name, body)| Some((name.as_str()?.to_owned(), body.clone())))
                .collect()
        })
        .unwrap_or_default()
}

/// Every host port one service publishes over TCP, in either syntax.
fn published(service: &Value, variable: &dyn Fn(&str) -> Option<String>) -> Vec<u16> {
    let Some(ports) = service.get("ports").and_then(Value::as_sequence) else {
        return Vec::new();
    };
    ports
        .iter()
        .flat_map(|entry| match entry {
            Value::String(short) => short_form(short, variable),
            Value::Number(_) => Vec::new(),
            long => long_form(long, variable),
        })
        .collect()
}

/// The host ports of a mapping written as `[address:]host:container[/protocol]`.
///
/// A mapping naming only the container's port publishes on a port the engine picks,
/// which no plugin can be told in advance, so it yields nothing.
fn short_form(entry: &str, variable: &dyn Fn(&str) -> Option<String>) -> Vec<u16> {
    let (mapping, protocol) = entry.rsplit_once('/').unwrap_or((entry, "tcp"));
    if !protocol.eq_ignore_ascii_case("tcp") {
        return Vec::new();
    }
    let parts = split(mapping);
    let Some(host) = parts.len().checked_sub(2).and_then(|at| parts.get(at)) else {
        return Vec::new();
    };
    ports_in(host, variable)
}

/// The host ports of a mapping written as a table with `published` and `protocol`.
fn long_form(entry: &Value, variable: &dyn Fn(&str) -> Option<String>) -> Vec<u16> {
    let protocol = entry
        .get("protocol")
        .and_then(Value::as_str)
        .unwrap_or("tcp");
    if !protocol.eq_ignore_ascii_case("tcp") {
        return Vec::new();
    }
    match entry.get("published") {
        Some(Value::Number(number)) => number
            .as_u64()
            .and_then(|port| u16::try_from(port).ok())
            .into_iter()
            .collect(),
        Some(Value::String(written)) => ports_in(written, variable),
        _ => Vec::new(),
    }
}

/// Split a mapping at every `:` outside a variable reference and outside the brackets
/// an IPv6 address is written in.
fn split(mapping: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut from = 0;
    for (at, letter) in mapping.char_indices() {
        match letter {
            '{' | '[' => depth += 1,
            '}' | ']' => depth = depth.saturating_sub(1),
            ':' if depth == 0 => {
                parts.push(&mapping[from..at]);
                from = at + 1;
            }
            _ => {}
        }
    }
    parts.push(&mapping[from..]);
    parts
}

/// The ports a host side names: one, or every one of a `low-high` range.
fn ports_in(host: &str, variable: &dyn Fn(&str) -> Option<String>) -> Vec<u16> {
    let resolved = resolve(host.trim(), variable);
    let (low, high) = resolved
        .split_once('-')
        .unwrap_or((resolved.as_str(), resolved.as_str()));
    match (low.trim().parse::<u16>(), high.trim().parse::<u16>()) {
        (Ok(low), Ok(high)) if low <= high => (low..=high).collect(),
        _ => Vec::new(),
    }
}

/// A value with a whole-value variable reference replaced by what it is set to, or by
/// its own default where it is set to nothing.
///
/// The forms Compose writes a default in — `${NAME:-default}` and `${NAME-default}` —
/// and the plain `${NAME}` and `$NAME`. Anything else is answered as written, and a
/// value that is not then a port is left out by the caller.
fn resolve(value: &str, variable: &dyn Fn(&str) -> Option<String>) -> String {
    let inner = value
        .strip_prefix("${")
        .and_then(|rest| rest.strip_suffix('}'))
        .or_else(|| value.strip_prefix('$'));
    let Some(inner) = inner else {
        return value.to_owned();
    };
    let (name, fallback) = inner
        .split_once(":-")
        .or_else(|| inner.split_once('-'))
        .map_or((inner, None), |(name, fallback)| (name, Some(fallback)));
    variable(name)
        .filter(|set| !set.is_empty())
        .or_else(|| fallback.map(str::to_owned))
        .unwrap_or_default()
}

/// The fixed address one service is given on one network, as a Compose file declares
/// it, or nothing where the file gives it none.
#[must_use]
pub fn fixed_address(text: &str, service: &str, network: &str) -> Option<String> {
    let document: Value = serde_yaml_ng::from_str(text).ok()?;
    document
        .get("services")?
        .get(service)?
        .get("networks")?
        .get(network)?
        .get("ipv4_address")?
        .as_str()
        .map(str::to_owned)
}

#[cfg(test)]
mod tests;
