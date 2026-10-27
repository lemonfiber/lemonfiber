//! The core names no part the bundle pins.
//!
//! Every part is a plugin the core asks over a contract, so a part's name in the core's
//! code or strings is code that works with that one part. Each part id the bundle pins is
//! looked for in every identifier and string literal of what ships, as words: `Gluetun`,
//! `SONARR_KEY` and `"config/homepage/x"` all name a part, and `seerrs` does not.
//!
//! Three kinds of name are not about a part. A part named for what it does shares its
//! name with the core's word for that, which [`OWNED`] lists. A native API is named after
//! the application that defined it, so the type naming native APIs and every
//! path through it are read past. And a household is told which app to install by the
//! product's name, which [`Until::Product`] marks.
//!
//! Everything else found is [`NAMED`], each entry a file, a part, how many times, and what
//! removes it. The list is held to exactly what is found, so it can only shrink: a new
//! name is refused, and a name taken out has its entry lowered in the same change.

use std::collections::BTreeMap;

use crate::source_tree::{crates_that_ship, in_a_crate_that_ships, shipped};

/// The crates that are the bundle, where every part is named by design.
const THE_BUNDLE: [&str; 2] = ["crates/lemonfiber-bundle/", "crates/lemonfiber-bundled/"];

/// The file declaring the native APIs, whose every variant is an application's name.
const NATIVE_APIS: &str = "crates/lemonfiber-manifest/src/schema/api.rs";

/// The type naming a native API, through whose paths an application's name is the API's.
const NATIVE_API: &str = "ApiKind";

/// The part ids that are also the core's own words, each with what the core means by it.
const OWNED: [(&str, &str); 3] = [
    (
        "door",
        "the front door a household is handed, which the core decides",
    ),
    ("decline", "invite.decline, a capability of the vocabulary"),
    (
        "request-gate",
        "request.gate, a capability of the vocabulary",
    ),
];

/// What takes a name off [`NAMED`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Until {
    /// The part moving to the plugin path with its adapter, whose code then goes.
    Moves(&'static str),
    /// A journalled migration renaming a setting, a secret, a directory or an answer an
    /// earlier lemonfiber wrote, which keeps reading the old name.
    Migrated,
    /// Never: the product the household installs, named in words telling it what to
    /// install on its own devices.
    Product,
    /// Never: the application a native API's stand-in answers as.
    NativeApi,
    /// A name an earlier release or the service it ran wrote, read so a stack it set up
    /// keeps working, which goes once no stack from before it is upgraded.
    Earlier,
}

/// A part named in one file, how many times, and what removes it.
struct Named {
    /// The file, from the workspace root.
    file: &'static str,
    /// The part id.
    part: &'static str,
    /// How many identifiers and strings name it.
    count: usize,
    /// What takes it off this list.
    until: Until,
}

/// Every part named in what ships, beyond [`OWNED`] and the native APIs.
const NAMED: &[Named] = &[
    Named {
        file: "crates/lemonfiber-contract/src/capabilities.rs",
        part: "prowlarr",
        count: 1,
        until: Until::NativeApi,
    },
    Named {
        file: "crates/lemonfiber-contract/src/capabilities.rs",
        part: "sonarr",
        count: 1,
        until: Until::NativeApi,
    },
    Named {
        file: "crates/lemonfiber-contract/src/capabilities/library.rs",
        part: "sabnzbd",
        count: 1,
        until: Until::NativeApi,
    },
    Named {
        file: "crates/lemonfiber-contract/src/capabilities/request.rs",
        part: "jellyfin",
        count: 2,
        until: Until::NativeApi,
    },
    Named {
        file: "crates/lemonfiber-core/src/app/credentials/declining.rs",
        part: "jellyfin",
        count: 1,
        until: Until::Moves("decline"),
    },
    Named {
        file: "crates/lemonfiber-core/src/app/credentials/gating.rs",
        part: "jellyfin",
        count: 4,
        until: Until::Moves("request-gate"),
    },
    Named {
        file: "crates/lemonfiber-core/src/app/forwarding/reaching.rs",
        part: "gluetun",
        count: 2,
        until: Until::Moves("gluetun"),
    },
    Named {
        file: "crates/lemonfiber-core/src/app/gating.rs",
        part: "jellyfin",
        count: 1,
        until: Until::Moves("request-gate"),
    },
    Named {
        file: "crates/lemonfiber-core/src/app/gating.rs",
        part: "radarr",
        count: 2,
        until: Until::Moves("request-gate"),
    },
    Named {
        file: "crates/lemonfiber-core/src/app/gating.rs",
        part: "sonarr",
        count: 2,
        until: Until::Moves("request-gate"),
    },
    Named {
        file: "crates/lemonfiber-core/src/app/gating/configured.rs",
        part: "jellyfin",
        count: 1,
        until: Until::Moves("request-gate"),
    },
    Named {
        file: "crates/lemonfiber-core/src/app/materialise.rs",
        part: "recyclarr",
        count: 3,
        until: Until::Moves("recyclarr"),
    },
    Named {
        file: "crates/lemonfiber-core/src/app/plugins/proving.rs",
        part: "caddy",
        count: 1,
        until: Until::Moves("caddy"),
    },
    Named {
        file: "crates/lemonfiber-core/src/app/showing/configured.rs",
        part: "homepage",
        count: 4,
        until: Until::Moves("homepage"),
    },
    Named {
        file: "crates/lemonfiber-core/src/app/targets/media.rs",
        part: "jellyfin",
        count: 1,
        until: Until::Moves("request-gate"),
    },
    Named {
        file: "crates/lemonfiber-core/src/backup.rs",
        part: "audiobookshelf",
        count: 2,
        until: Until::Migrated,
    },
    Named {
        file: "crates/lemonfiber-core/src/backup.rs",
        part: "bazarr",
        count: 1,
        until: Until::Migrated,
    },
    Named {
        file: "crates/lemonfiber-core/src/backup.rs",
        part: "jellyfin",
        count: 2,
        until: Until::Migrated,
    },
    Named {
        file: "crates/lemonfiber-core/src/backup.rs",
        part: "lidarr",
        count: 2,
        until: Until::Migrated,
    },
    Named {
        file: "crates/lemonfiber-core/src/backup.rs",
        part: "prowlarr",
        count: 1,
        until: Until::Migrated,
    },
    Named {
        file: "crates/lemonfiber-core/src/backup.rs",
        part: "radarr",
        count: 2,
        until: Until::Migrated,
    },
    Named {
        file: "crates/lemonfiber-core/src/backup.rs",
        part: "sabnzbd",
        count: 1,
        until: Until::Migrated,
    },
    Named {
        file: "crates/lemonfiber-core/src/backup.rs",
        part: "sonarr",
        count: 2,
        until: Until::Migrated,
    },
    Named {
        file: "crates/lemonfiber-core/src/clients.rs",
        part: "jellyfin",
        count: 5,
        until: Until::Product,
    },
    Named {
        file: "crates/lemonfiber-core/src/config/display.rs",
        part: "flaresolverr",
        count: 1,
        until: Until::Migrated,
    },
    Named {
        file: "crates/lemonfiber-core/src/config/display.rs",
        part: "homepage",
        count: 1,
        until: Until::Migrated,
    },
    Named {
        file: "crates/lemonfiber-core/src/config/display.rs",
        part: "recyclarr",
        count: 1,
        until: Until::Migrated,
    },
    Named {
        file: "crates/lemonfiber-core/src/config/display.rs",
        part: "seerr",
        count: 1,
        until: Until::Migrated,
    },
    Named {
        file: "crates/lemonfiber-core/src/config/renamed.rs",
        part: "jellyfin",
        count: 2,
        until: Until::Earlier,
    },
    Named {
        file: "crates/lemonfiber-core/src/config/keys.rs",
        part: "audiobookshelf",
        count: 1,
        until: Until::Migrated,
    },
    Named {
        file: "crates/lemonfiber-core/src/config/keys.rs",
        part: "bindery",
        count: 1,
        until: Until::Migrated,
    },
    Named {
        file: "crates/lemonfiber-core/src/config/keys.rs",
        part: "homepage",
        count: 1,
        until: Until::Moves("homepage"),
    },
    Named {
        file: "crates/lemonfiber-core/src/config/keys.rs",
        part: "jellyfin",
        count: 1,
        until: Until::Migrated,
    },
    Named {
        file: "crates/lemonfiber-core/src/config/keys.rs",
        part: "nzbhydra2",
        count: 1,
        until: Until::Migrated,
    },
    Named {
        file: "crates/lemonfiber-core/src/config/keys.rs",
        part: "qbittorrent",
        count: 1,
        until: Until::Migrated,
    },
    Named {
        file: "crates/lemonfiber-core/src/credential/held.rs",
        part: "gluetun",
        count: 1,
        until: Until::Moves("gluetun"),
    },
    Named {
        file: "crates/lemonfiber-core/src/doctor/vpn.rs",
        part: "gluetun",
        count: 2,
        until: Until::Moves("gluetun"),
    },
    Named {
        file: "crates/lemonfiber-core/src/doctor/vpn/killswitch.rs",
        part: "gluetun",
        count: 1,
        until: Until::Moves("gluetun"),
    },
    Named {
        file: "crates/lemonfiber-core/src/doctor/vpn/tunnel.rs",
        part: "gluetun",
        count: 5,
        until: Until::Moves("gluetun"),
    },
    Named {
        file: "crates/lemonfiber-core/src/door.rs",
        part: "caddy",
        count: 1,
        until: Until::Moves("caddy"),
    },
    Named {
        file: "crates/lemonfiber-core/src/door.rs",
        part: "calibre-web-automated",
        count: 1,
        until: Until::Moves("calibre-web-automated"),
    },
    Named {
        file: "crates/lemonfiber-core/src/door.rs",
        part: "homepage",
        count: 1,
        until: Until::Moves("homepage"),
    },
    Named {
        file: "crates/lemonfiber-core/src/lib.rs",
        part: "recyclarr",
        count: 1,
        until: Until::Moves("recyclarr"),
    },
    Named {
        file: "crates/lemonfiber-core/src/plugin/fronting.rs",
        part: "caddy",
        count: 1,
        until: Until::Moves("caddy"),
    },
    Named {
        file: "crates/lemonfiber-core/src/recyclarr.rs",
        part: "radarr",
        count: 5,
        until: Until::Moves("recyclarr"),
    },
    Named {
        file: "crates/lemonfiber-core/src/recyclarr.rs",
        part: "sonarr",
        count: 3,
        until: Until::Moves("recyclarr"),
    },
    Named {
        file: "crates/lemonfiber-core/src/seed/run/taken_back.rs",
        part: "seerr",
        count: 1,
        until: Until::Earlier,
    },
    Named {
        file: "crates/lemonfiber-core/src/settings/legacy.rs",
        part: "flaresolverr",
        count: 2,
        until: Until::Migrated,
    },
    Named {
        file: "crates/lemonfiber-core/src/settings/legacy.rs",
        part: "gluetun",
        count: 4,
        until: Until::Migrated,
    },
    Named {
        file: "crates/lemonfiber-core/src/settings/legacy.rs",
        part: "homepage",
        count: 2,
        until: Until::Migrated,
    },
    Named {
        file: "crates/lemonfiber-core/src/settings/legacy.rs",
        part: "recyclarr",
        count: 2,
        until: Until::Migrated,
    },
    Named {
        file: "crates/lemonfiber-core/src/settings/legacy.rs",
        part: "seerr",
        count: 2,
        until: Until::Migrated,
    },
    Named {
        file: "crates/lemonfiber-core/src/settings/legacy.rs",
        part: "unpackerr",
        count: 1,
        until: Until::Migrated,
    },
    Named {
        file: "crates/lemonfiber-core/src/uninstall/outside.rs",
        part: "jellyfin",
        count: 5,
        until: Until::Product,
    },
    Named {
        file: "crates/lemonfiber-core/src/wizard/answers.rs",
        part: "jellyfin",
        count: 2,
        until: Until::Earlier,
    },
    Named {
        file: "crates/lemonfiber-error/src/codes/vpn.rs",
        part: "gluetun",
        count: 1,
        until: Until::Moves("gluetun"),
    },
    Named {
        file: "crates/lemonfiber-plugin/src/refusing/bundled.rs",
        part: "caddy",
        count: 1,
        until: Until::Moves("caddy"),
    },
    Named {
        file: "crates/lemonfiber-sidecar/src/decline.rs",
        part: "jellyfin",
        count: 1,
        until: Until::Moves("decline"),
    },
    Named {
        file: "crates/lemonfiber-sidecar/src/gate.rs",
        part: "jellyfin",
        count: 1,
        until: Until::Moves("request-gate"),
    },
    Named {
        file: "crates/lemonfiber-sidecar/src/gate.rs",
        part: "radarr",
        count: 1,
        until: Until::Moves("request-gate"),
    },
    Named {
        file: "crates/lemonfiber-sidecar/src/gate.rs",
        part: "sonarr",
        count: 1,
        until: Until::Moves("request-gate"),
    },
];

/// Every part id the bundle pins, except the core's own words.
fn parts() -> Vec<String> {
    lemonfiber_bundled::PINNED
        .iter()
        .map(|pinned| pinned.first_party.plugin.to_owned())
        .filter(|id| !OWNED.iter().any(|(owned, _)| owned == id))
        .collect()
}

/// The words of an identifier or a string, lowercased: split at every character that is
/// not a letter or a digit, and where a lowercase letter or a digit meets an uppercase one.
fn words(said: &str) -> Vec<String> {
    let mut words: Vec<String> = Vec::new();
    let mut word = String::new();
    let mut before: Option<char> = None;
    for at in said.chars() {
        let parted = !at.is_ascii_alphanumeric()
            || (at.is_ascii_uppercase()
                && before.is_some_and(|was| was.is_ascii_lowercase() || was.is_ascii_digit()));
        if parted && !word.is_empty() {
            words.push(std::mem::take(&mut word));
        }
        if at.is_ascii_alphanumeric() {
            word.push(at.to_ascii_lowercase());
        }
        before = Some(at);
    }
    if !word.is_empty() {
        words.push(word);
    }
    words
}

/// Whether `said`'s words hold the part `part`'s words, in a row.
fn names(said: &[String], part: &str) -> bool {
    let wanted: Vec<&str> = part.split('-').collect();
    said.windows(wanted.len())
        .any(|run| run.iter().zip(&wanted).all(|(one, other)| one == other))
}

/// How many identifiers and string literals of `text` name each of `parts`, read past
/// every path through [`NATIVE_API`].
fn named_in(text: &str, parts: &[String]) -> BTreeMap<String, usize> {
    let mut found = BTreeMap::new();
    let mut at = 0;
    let mut behind: [&str; 2] = ["", ""];
    for token in rustc_lexer::tokenize(text, rustc_lexer::FrontmatterAllowed::No) {
        let length = token.len as usize;
        let said = text.get(at..at + length).unwrap_or_default();
        at += length;
        let read = match token.kind {
            rustc_lexer::TokenKind::Whitespace
            | rustc_lexer::TokenKind::LineComment { .. }
            | rustc_lexer::TokenKind::BlockComment { .. } => continue,
            rustc_lexer::TokenKind::Ident => behind != [NATIVE_API, "::"],
            rustc_lexer::TokenKind::Literal { kind, .. } => matches!(
                kind,
                rustc_lexer::LiteralKind::Str { .. }
                    | rustc_lexer::LiteralKind::RawStr { .. }
                    | rustc_lexer::LiteralKind::ByteStr { .. }
                    | rustc_lexer::LiteralKind::RawByteStr { .. }
                    | rustc_lexer::LiteralKind::CStr { .. }
                    | rustc_lexer::LiteralKind::RawCStr { .. }
            ),
            _ => false,
        };
        behind = match (behind, said) {
            ([name, ":"], ":") => [name, "::"],
            ([_, last], next) => [last, next],
        };
        if !read {
            continue;
        }
        let spoken = words(said);
        for part in parts.iter().filter(|part| names(&spoken, part)) {
            *found.entry(part.clone()).or_insert(0) += 1;
        }
    }
    found
}

/// Every part named in what ships, by file and part, outside the bundle and the file
/// declaring the native APIs.
fn found() -> BTreeMap<(String, String), usize> {
    let parts = parts();
    let ships = crates_that_ship();
    shipped()
        .into_iter()
        .filter(|(path, _)| in_a_crate_that_ships(path, &ships))
        .filter(|(path, _)| !THE_BUNDLE.iter().any(|bundle| path.starts_with(bundle)))
        .filter(|(path, _)| path != NATIVE_APIS)
        .flat_map(|(path, text)| {
            named_in(&text, &parts)
                .into_iter()
                .map(move |(part, count)| ((path.clone(), part), count))
        })
        .collect()
}

#[test]
fn no_part_is_named_in_the_core_beyond_what_waits_on_its_removal() {
    let found = found();
    let listed: BTreeMap<(String, String), usize> = NAMED
        .iter()
        .map(|named| ((named.file.to_owned(), named.part.to_owned()), named.count))
        .collect();
    let new: Vec<String> = found
        .iter()
        .filter(|(at, count)| listed.get(*at).is_none_or(|allowed| *count > allowed))
        .map(|((file, part), count)| format!("{file} names {part} {count} time(s)"))
        .collect();
    let stale: Vec<String> = listed
        .iter()
        .filter(|(at, allowed)| found.get(*at).is_none_or(|count| count < allowed))
        .map(|((file, part), allowed)| {
            format!(
                "{file} names {part} {} time(s), and the list allows {allowed}",
                found
                    .get(&(file.clone(), part.clone()))
                    .copied()
                    .unwrap_or(0)
            )
        })
        .collect();
    assert!(
        new.is_empty(),
        "the core names a part the bundle pins, which it reaches only over a contract: \
         {new:#?}"
    );
    assert!(
        stale.is_empty(),
        "a name the list allows is gone, so lower its entry: {stale:#?}"
    );
}

#[test]
fn every_entry_names_a_part_the_bundle_pins_once() {
    let parts = parts();
    let mut seen = std::collections::BTreeSet::new();
    for named in NAMED {
        assert!(
            parts.iter().any(|part| part == named.part),
            "{} is no part the bundle pins",
            named.part
        );
        assert!(
            named.count > 0,
            "{} {} counts nothing",
            named.file,
            named.part
        );
        if let Until::Moves(part) = named.until {
            assert!(
                lemonfiber_bundled::PINNED
                    .iter()
                    .any(|pinned| pinned.first_party.plugin == part),
                "{part} is no part the bundle pins"
            );
        }
        assert!(
            seen.insert((named.file, named.part)),
            "{} {} is listed twice",
            named.file,
            named.part
        );
    }
}

#[test]
fn a_planted_name_is_found_and_nothing_this_reads_past() {
    let parts: Vec<String> = ["sonarr", "radarr", "calibre-web-automated", "seerr"]
        .iter()
        .map(|part| (*part).to_owned())
        .collect();
    let planted = r#"
        // sonarr, in a comment
        fn sonarr_client() -> &'static str { "Radarr" }
        const PATH: &str = "config/calibre-web-automated/x";
        let api = ApiKind::Sonarr;
        let api = ApiKind :: Seerr;
        let seerrs = 1;
        struct CalibreWeb;
    "#;

    let found = named_in(planted, &parts);

    assert_eq!(
        found,
        BTreeMap::from([
            ("calibre-web-automated".to_owned(), 1),
            ("radarr".to_owned(), 1),
            ("sonarr".to_owned(), 1),
        ])
    );
}

#[test]
fn the_cores_own_words_are_not_read_as_parts() {
    let parts = parts();
    for (owned, _) in OWNED {
        assert!(!parts.iter().any(|part| part == owned), "{owned}");
    }
    assert!(parts.len() > 10, "the bundle's parts were read: {parts:?}");
}
