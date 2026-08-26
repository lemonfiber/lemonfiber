//! The requests the stack's own services make, attributed to them.
//!
//! An indexer query is Prowlarr asking an indexer. A poster is Radarr asking a
//! metadata provider. A peer connection is qBittorrent being a torrent client.
//! Counting any of those as lemonfiber's would overstate what this product does —
//! and leaving them out would understate what running the stack does, which is the
//! thing an operator is actually deciding about.
//!
//! Every service the stack declares has an entry, including the ones that reach
//! nothing. That is what makes the list hold: a service arriving in the stack is red
//! until somebody writes down what it talks to, rather than quietly absent from a
//! list of the ones that do.

use super::Elsewhere;
use lemonfiber_manifest::Service;

/// What each service in the stack reaches, by the id the stack declares it under.
///
/// The second half is where it goes and the third is what it asks for, both in the
/// terms an operator would recognise rather than in protocol names. Where a service
/// reaches nothing, the entry says so — an empty destination is an answer.
pub const ELSEWHERE: &[(&str, &str, &str)] = &[
    (
        "prowlarr",
        "the indexers you configured",
        "Runs the searches everything else asks for, and reads each indexer's capabilities and \
         remaining allowance, authenticating with the keys you gave it.",
    ),
    (
        "flaresolverr",
        "the indexer sites that challenge it",
        "Fetches a page through a headless browser when an indexer sits behind bot protection, \
         which means it visits that indexer directly.",
    ),
    (
        "nzbhydra2",
        "the Usenet indexers you configured",
        "Searches several indexers at once and merges what they answer, with the keys you gave \
         it.",
    ),
    (
        "sabnzbd",
        "your Usenet provider",
        "Signs in with your account and fetches the articles a download is made of.",
    ),
    (
        "gluetun",
        "your VPN provider's servers",
        "Dials the tunnel everything torrent-shaped is routed through, and asks the provider \
         for a forwarded port where you asked for one.",
    ),
    (
        "qbittorrent",
        "trackers and peers, through the tunnel",
        "Announces to a torrent's trackers and exchanges data with peers, which is what a \
         torrent client is; all of it inside gluetun's network, so it stops when the tunnel \
         does.",
    ),
    (
        "sonarr",
        "television metadata providers",
        "Reads series, season and episode information, artwork and air dates for what is in \
         your library and what you add to it.",
    ),
    (
        "radarr",
        "film metadata providers",
        "Reads titles, release dates and artwork for the films in your library and the ones \
         you add.",
    ),
    (
        "lidarr",
        "music metadata providers",
        "Reads artist, album and track information for the music in your library.",
    ),
    (
        "bindery",
        "book and audiobook metadata providers",
        "Reads author, title and cover information for the books it watches for.",
    ),
    (
        "bazarr",
        "the subtitle providers you enable",
        "Searches for subtitles matching what is in your library, signing in where a provider \
         requires an account.",
    ),
    (
        "jellyfin",
        "metadata providers and its own plugin repository",
        "Reads artwork and descriptions for what is in your library, and checks its plugin \
         repository for updates unless you turn that off in its own settings.",
    ),
    (
        "seerr",
        "the metadata provider it lists titles from",
        "Reads the catalogue the household browses and requests from, and the artwork beside \
         it.",
    ),
    (
        "calibre-web-automated",
        "book metadata providers",
        "Reads covers and descriptions for the ebooks in your library when you ask it to.",
    ),
    (
        "audiobookshelf",
        "audiobook metadata providers",
        "Reads covers, chapters and descriptions for the audiobooks in your library when you \
         ask it to.",
    ),
    (
        "recyclarr",
        "the community quality-guide repository",
        "Syncs the quality profiles on its own schedule. This is the sync lemonfiber's own \
         guide probe reports on and does not perform.",
    ),
    (
        "unpackerr",
        "",
        "Nothing. It watches the download directories and extracts archived releases where it \
         finds them, entirely on this machine.",
    ),
    (
        "homepage",
        "",
        "Nothing beyond this machine. It reads the other services' own APIs over the stack's \
         internal network to draw their status.",
    ),
    (
        "caddy",
        "",
        "Nothing. It answers names on this machine and forwards to the services beside it; no \
         certificate is fetched, because nothing here is published to the internet.",
    ),
];

/// What the services in this stack reach, in the order the stack declares them.
pub(super) fn elsewhere(services: &[Service]) -> Vec<Elsewhere> {
    services
        .iter()
        .filter_map(|service| {
            ELSEWHERE.iter().find(|(id, _, _)| *id == service.id).map(
                |(_, destination, purpose)| Elsewhere {
                    service: service.id.clone(),
                    destination: (*destination).to_owned(),
                    purpose: (*purpose).to_owned(),
                },
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{elsewhere, ELSEWHERE};
    use std::collections::BTreeSet;

    fn declared() -> Vec<lemonfiber_manifest::Service> {
        crate::test_support::stack()
            .manifest()
            .map(|manifest| manifest.services)
            .unwrap_or_default()
    }

    /// Both directions, because either alone passes about nothing. A service with
    /// no entry is one nobody has said anything about; an entry with no service is
    /// a decision about a stack that has moved on.
    #[test]
    fn every_service_the_stack_declares_says_what_it_reaches() {
        let services: BTreeSet<String> = declared().into_iter().map(|service| service.id).collect();
        let counted = services.len();
        assert!(
            counted > 10,
            "the stack declares {counted} services, which means this is reading the wrong manifest"
        );
        let listed: BTreeSet<String> = ELSEWHERE
            .iter()
            .map(|(id, _, _)| (*id).to_owned())
            .collect();
        assert_eq!(
            services, listed,
            "a service the stack runs is not written down here, or something written down \
             here is not in the stack any more — say what it talks to, or take it out"
        );
    }

    #[test]
    fn every_entry_says_what_it_asks_for_including_the_ones_that_ask_nothing() {
        let silent: Vec<&str> = ELSEWHERE
            .iter()
            .filter(|(_, _, purpose)| purpose.split_whitespace().count() < 6)
            .map(|(id, _, _)| *id)
            .collect();
        assert!(
            silent.is_empty(),
            "these are listed and the list does not say what they ask for: {silent:?}"
        );
    }

    #[test]
    fn a_service_that_reaches_nothing_is_named_with_nowhere_to_go() {
        let quiet: Vec<&str> = ELSEWHERE
            .iter()
            .filter(|(_, destination, _)| destination.is_empty())
            .map(|(id, _, _)| *id)
            .collect();
        assert_eq!(quiet, vec!["unpackerr", "homepage", "caddy"]);
    }

    #[test]
    fn the_list_follows_the_stack_it_is_given_rather_than_the_one_written_down() {
        let one = declared().into_iter().take(1).collect::<Vec<_>>();
        let found = elsewhere(&one);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(
            found
                .iter()
                .any(|entry| entry.service == "prowlarr" && entry.destination.contains("indexers")),
            "{found:?}"
        );
    }

    #[test]
    fn a_service_nothing_is_written_down_about_is_left_out_rather_than_guessed_at() {
        let unknown: Vec<lemonfiber_manifest::Service> = declared()
            .into_iter()
            .take(1)
            .map(|mut service| {
                service.id = "something-new".to_owned();
                service
            })
            .collect();
        assert_eq!(unknown.len(), 1, "the stack declares services to rename");
        assert!(elsewhere(&unknown).is_empty());
    }
}
