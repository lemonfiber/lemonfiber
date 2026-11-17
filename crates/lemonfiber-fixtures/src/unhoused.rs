//! Whether a piece of text is in the household's words.
//!
//! What a member's surface may not show: a service's name, a domain term, a size, a rate,
//! an address, a version, a stage of the pipeline, or how the stack stands. Shared by every
//! test that holds text a member reads to it, so there is one list.

/// The names of services the stack could run that the bundle pins no plugin for, and of
/// the engines and the tunnel it runs on.
const UNPINNED: &[&str] = &[
    "apprise",
    "calibre",
    "docker",
    "gotify",
    "jellyseerr",
    "komga",
    "lemonfiber",
    "nzbget",
    "nzbhydra",
    "overseerr",
    "plex",
    "podman",
    "readarr",
    "sickchill",
    "sickgear",
    "tdarr",
    "transmission",
    "whisparr",
    "wireguard",
];

/// Words for how the stack is built and run, and for its pipeline, that a member is never
/// handed.
const TERMS: &[&str] = &[
    "administrator",
    "admin",
    "api",
    "backbone",
    "bitrate",
    "block account",
    "byte",
    "bytes",
    "certificate",
    "codec",
    "compose",
    "config",
    "configuration",
    "container",
    "containers",
    "curator",
    "custom format",
    "dashboard",
    "database",
    "disk",
    "dns",
    "download client",
    "downloaded",
    "downloader",
    "downloading",
    "endpoint",
    "engine",
    "error",
    "gigabyte",
    "grab",
    "grabbed",
    "hardlink",
    "hdr",
    "http",
    "https",
    "id",
    "identifier",
    "image",
    "import",
    "imported",
    "indexer",
    "indexers",
    "ip",
    "json",
    "killswitch",
    "log",
    "logs",
    "media server",
    "megabyte",
    "monitored",
    "nzb",
    "operator",
    "peer",
    "pem",
    "pipeline",
    "plugin",
    "port",
    "port forwarding",
    "proxy",
    "quality",
    "quality profile",
    "queue",
    "queued",
    "quick connect",
    "ratio",
    "remux",
    "request service",
    "retention",
    "root folder",
    "seed",
    "seeding",
    "server",
    "servers",
    "service",
    "services",
    "stack",
    "stalled",
    "storage",
    "terabyte",
    "token",
    "torrent",
    "tracker",
    "transcode",
    "transcoding",
    "tunnel",
    "upstream",
    "url",
    "usenet",
    "vpn",
    "webhook",
];

/// What a size is written in.
const SIZES: &[&str] = &[
    "b", "kb", "mb", "gb", "tb", "kib", "mib", "gib", "tib", "kbps", "mbps", "gbps",
];

/// Every name a member may not be shown: the parts the bundle pins, each id's hyphens read
/// as spaces, and the services, engines and tunnel it pins none for.
fn names() -> impl Iterator<Item = String> {
    crate::parts::pinned()
        .map(|id| id.replace('-', " "))
        .chain(UNPINNED.iter().map(|name| (*name).to_owned()))
}

/// Text a version, an address, a size or a rate makes unfit for a member, published beside
/// the lists so a surface holding its own words to them restates those rules alike.
const RULED: &[&str] = &[
    "It needs 4 GB more",
    "It needs 12GB more",
    "Fetching at 3 MB/s",
    "Fetching forty per second",
    "Reached at 192.168.1.20",
    "Open it at localhost",
    "See www.house.example",
    "Open it at house://shelf",
    "Now on v1.4.2",
];

/// What a member's surface may not show, as the contract publishes it: every name, term
/// and size unit this holds text to, the parts the bundle pins among the names, and each
/// ruled example with what it names.
#[must_use]
pub fn published() -> serde_json::Value {
    let mut names: Vec<String> = names().collect();
    names.sort_unstable();
    names.dedup();
    let examples: Vec<serde_json::Value> = RULED
        .iter()
        .map(|text| serde_json::json!({ "text": text, "names": unhoused(text) }))
        .collect();
    serde_json::json!({
        "names": names,
        "terms": TERMS,
        "sizes": SIZES,
        "examples": examples,
    })
}

/// What in `text` a member may not be shown, or nothing where it is all the household's
/// words.
#[must_use]
pub fn unhoused(text: &str) -> Option<String> {
    let lower = text.to_lowercase();
    let words: Vec<&str> = lower
        .split(|letter: char| !(letter.is_alphanumeric() || letter == '\''))
        .filter(|word| !word.is_empty())
        .collect();
    let mut named = names().chain(TERMS.iter().map(|term| (*term).to_owned()));
    if let Some(term) = named.find(|term| said(&words, term)) {
        return Some(term);
    }
    let tokens: Vec<&str> = lower.split_whitespace().collect();
    for (at, token) in tokens.iter().enumerate() {
        let token = token.trim_matches(|letter: char| !letter.is_alphanumeric());
        if versioned(token) {
            return Some(token.to_owned());
        }
        let digits = token.trim_end_matches(|letter: char| letter.is_ascii_alphabetic());
        let unit = &token[digits.len()..];
        let next = tokens
            .get(at + 1)
            .and_then(|next| next.split('/').next())
            .map(|next| next.trim_matches(|letter: char| !letter.is_alphanumeric()));
        let numeric = !digits.is_empty()
            && digits
                .chars()
                .all(|letter| letter.is_ascii_digit() || letter == '.');
        if numeric && (SIZES.contains(&unit) || next.is_some_and(|next| SIZES.contains(&next))) {
            return Some(token.to_owned());
        }
    }
    ["://", "localhost", "www.", "/s", "per second"]
        .into_iter()
        .find(|marked| lower.contains(marked))
        .map(str::to_owned)
}

/// Whether `term`, which may be several words, is said among `words`.
fn said(words: &[&str], term: &str) -> bool {
    let wanted: Vec<&str> = term.split(' ').collect();
    words
        .windows(wanted.len())
        .any(|run| run == wanted.as_slice())
}

/// Whether `token` reads as a version or an address: numbers joined by dots, two or more
/// of them, with a `v` before them or not.
fn versioned(token: &str) -> bool {
    let bare = token.strip_prefix('v').unwrap_or(token);
    let parts: Vec<&str> = bare.split('.').collect();
    parts.len() >= 2
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.chars().all(|letter| letter.is_ascii_digit()))
}

#[cfg(test)]
mod tests;
