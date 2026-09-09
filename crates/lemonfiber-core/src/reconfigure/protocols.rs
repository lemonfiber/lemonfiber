//! What adding a way of downloading opens, and what dropping one keeps.
//!
//! The commonest reconfiguration there is: somebody starts library-only and wants
//! Usenet, or starts on Usenet and wants torrents. Growing into a full stack one
//! decision at a time is a supported path rather than a repair, which means each
//! step opens exactly what the new protocol needs and nothing else — an operator
//! adding Usenet asked no question about a tunnel and must not be handed one.
//!
//! Dropping one is the same path walked backwards, and the sentence that matters
//! is the opposite one. What stops is obvious from what was switched off; what is
//! *kept* is not, and it is the whole of what somebody is weighing when they drop
//! the way they built their library with. So it is stated in full rather than left
//! to be inferred from silence.
//!
//! Both answers are read off what the stack and the walk already declare — the
//! profiles a protocol guards, and the steps setup puts for it. A second list here
//! would be a list that could disagree with the questions actually asked.

use std::path::Path;

use lemonfiber_manifest::Manifest;

use super::Opening;
use crate::config::{reads_as_on, Protocols, TORRENT_KEY, USENET_KEY};
use crate::prerequisites::prerequisites;
use crate::stack::closure::everything;
use crate::wizard::opened_by;

/// What the protocols become when `key` is set to `value`.
///
/// `None` where the setting is not one of the two that decide them, which is every
/// other setting a change can name.
#[must_use]
pub fn changed(now: Protocols, key: &str, value: &str) -> Option<Protocols> {
    let on = reads_as_on(value);
    match key {
        USENET_KEY => Some(Protocols { usenet: on, ..now }),
        TORRENT_KEY => Some(Protocols { torrent: on, ..now }),
        _ => None,
    }
}

/// What moving from `before` to `after` newly asks the operator for.
///
/// The accounts they have to go and obtain, then the settings the answers are kept
/// in — in that order, because an account has to exist before a credential for it
/// can be pasted anywhere. Empty for a reduction, which asks for nothing.
#[must_use]
pub fn opened(before: Protocols, after: Protocols) -> Vec<Opening> {
    let had = prerequisites(before).items;
    let mut opens: Vec<Opening> = prerequisites(after)
        .items
        .into_iter()
        .filter(|item| !had.iter().any(|held| held.id == item.id))
        .map(|item| Opening {
            what: item.label.to_owned(),
            because: item.why.to_owned(),
            setting: None,
        })
        .collect();
    opens.extend(opened_by(before, after).into_iter().flat_map(|step| {
        step.settings().iter().map(move |setting| Opening {
            what: (*setting).to_owned(),
            because: format!("{} is asked for once this is on", step.label()),
            setting: Some((*setting).to_owned()),
        })
    }));
    opens
}

/// The services `after` stops that `before` was running, by the name the stack
/// gives them.
///
/// The stack's own answer: a profile declares the protocol it cannot run without,
/// so what a reduction takes away is read from the manifest rather than from a list
/// here that a renamed profile would silently outlive. Empty where the reduction
/// leaves the stack with nothing to run at all, which the closure refuses to plan
/// rather than answer.
#[must_use]
pub fn stopped(manifest: &Manifest, before: Protocols, after: Protocols) -> Vec<String> {
    let Ok(was) = everything(manifest, before) else {
        return Vec::new();
    };
    let now = everything(manifest, after).map(|plan| plan.services);
    let now = now.unwrap_or_default();
    was.services
        .into_iter()
        .filter(|service| !now.contains(service))
        .collect()
}

/// What dropping a way of downloading leaves exactly as it is.
///
/// Stated in full and with the paths where they are known, because the fear this
/// answers is specific: that switching off torrents takes the library built with
/// them. It does not. The change writes one setting; nothing walks the operator's
/// disk, and nothing removes a service's record of what it already has.
///
/// Empty for a change that takes nothing away, which has nothing to reassure
/// anybody about.
#[must_use]
pub fn kept(before: Protocols, after: Protocols, root: Option<&Path>) -> Vec<String> {
    if !reduces(before, after) {
        return Vec::new();
    }
    let under = |what: &str, folder: &str| {
        root.map_or_else(
            || format!("everything already {what}"),
            |root| {
                format!(
                    "everything already {what}, in {}",
                    root.join(folder).display()
                )
            },
        )
    };
    vec![
        under("downloaded", "downloads"),
        under("imported into the library", "media"),
        "each service's own record of what it holds, so nothing is searched for a second time"
            .to_owned(),
    ]
}

/// Whether the change takes a way of downloading away.
#[must_use]
pub const fn reduces(before: Protocols, after: Protocols) -> bool {
    (before.usenet && !after.usenet) || (before.torrent && !after.torrent)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{changed, kept, opened, reduces, stopped, Opening};
    use crate::config::{Protocols, DATA_ROOT_KEY, TORRENT_KEY, USENET_KEY};

    /// The settings an opening names, so a claim about what was opened is a claim
    /// about a setting rather than about a sentence.
    fn settings(opens: &[Opening]) -> Vec<String> {
        opens
            .iter()
            .filter_map(|open| open.setting.clone())
            .collect()
    }

    #[test]
    fn a_setting_that_decides_nothing_about_downloading_changes_no_protocol() {
        assert_eq!(changed(Protocols::both(), DATA_ROOT_KEY, "/srv"), None);
    }

    #[test]
    fn switching_one_on_leaves_the_other_where_it_was() {
        let from_nothing = changed(Protocols::none(), USENET_KEY, "on");
        assert_eq!(
            from_nothing,
            Some(Protocols {
                usenet: true,
                torrent: false
            })
        );
        let and_then_torrents = changed(Protocols::none(), TORRENT_KEY, "on");
        assert_eq!(
            and_then_torrents,
            Some(Protocols {
                usenet: false,
                torrent: true
            })
        );
    }

    #[test]
    fn adding_usenet_opens_its_own_login_and_never_the_tunnel() {
        // The whole of the requirement in one assertion: somebody who asked for
        // Usenet is not handed a VPN question they never asked about.
        let opens = opened(
            Protocols::none(),
            Protocols {
                usenet: true,
                torrent: false,
            },
        );
        let named = settings(&opens);
        assert!(
            named.contains(&"USENET_HOST".to_owned()),
            "USENET_HOST is settled, of {} settings",
            named.len()
        );
        assert!(
            named.contains(&"USENET_PASS".to_owned()),
            "USENET_PASS is settled, of {} settings",
            named.len()
        );
        assert!(
            named.contains(&"INDEXER_URL".to_owned()),
            "INDEXER_URL is settled, of {} settings",
            named.len()
        );
        assert!(
            !named.contains(&"VPN_PROVIDER".to_owned()),
            "VPN_PROVIDER is settled, of {} settings",
            named.len()
        );
        let accounts: Vec<&str> = opens
            .iter()
            .filter(|open| open.setting.is_none())
            .map(|open| open.what.as_str())
            .collect();
        assert!(
            accounts.contains(&"Usenet provider"),
            "the provider is asked for, of {} accounts",
            accounts.len()
        );
        assert!(
            !accounts.iter().any(|what| what.contains("VPN")),
            "no tunnel is asked for, of {} accounts",
            accounts.len()
        );
    }

    #[test]
    fn adding_torrents_opens_the_tunnel_and_never_the_usenet_login() {
        // The tunnel is an account to go and obtain rather than a setting to paste, so
        // it is opened as a prerequisite; what it must never open is the Usenet login,
        // which somebody who asked for torrents did not ask about.
        let opens = opened(
            Protocols::none(),
            Protocols {
                usenet: false,
                torrent: true,
            },
        );
        let named = settings(&opens);
        assert!(
            !named.contains(&"USENET_HOST".to_owned()),
            "the provider is not asked for again, of {} settings",
            named.len()
        );
        let accounts: Vec<&str> = opens
            .iter()
            .filter(|open| open.setting.is_none())
            .map(|open| open.what.as_str())
            .collect();
        assert!(
            accounts.iter().any(|what| what.contains("VPN")),
            "the tunnel is asked for, of {} accounts",
            accounts.len()
        );
        assert!(
            !accounts.contains(&"Usenet provider"),
            "the provider is not asked for again, of {} accounts",
            accounts.len()
        );
    }

    #[test]
    fn adding_the_second_protocol_opens_only_what_the_first_did_not() {
        // The indexer was already asked for, so it is not asked for again — which is
        // the difference between growing into a stack and being walked through setup
        // a second time.
        let opens = opened(
            Protocols {
                usenet: true,
                torrent: false,
            },
            Protocols::both(),
        );
        let named = settings(&opens);
        assert!(
            !named.contains(&"INDEXER_URL".to_owned()),
            "the indexer is not asked for again, of {} settings",
            named.len()
        );
        assert!(
            !named.contains(&"USENET_HOST".to_owned()),
            "USENET_HOST is settled, of {} settings",
            named.len()
        );
        // What it does open is what torrents need and Usenet did not: an indexer that
        // carries them, and a tunnel.
        let accounts: Vec<&str> = opens
            .iter()
            .filter(|open| open.setting.is_none())
            .map(|open| open.what.as_str())
            .collect();
        assert!(!accounts.is_empty(), "something is asked for");
        assert!(
            !accounts.contains(&"Usenet provider"),
            "the provider is not asked for again, of {} accounts",
            accounts.len()
        );
    }

    #[test]
    fn dropping_one_opens_nothing_at_all() {
        assert!(opened(Protocols::both(), Protocols::none()).is_empty());
    }

    #[test]
    fn dropping_one_keeps_the_downloads_and_the_library_by_name() {
        let said = kept(
            Protocols::both(),
            Protocols {
                usenet: true,
                torrent: false,
            },
            Some(Path::new("/srv/media")),
        );
        let joined = said.join(" | ");
        assert!(joined.contains("/srv/media/downloads"), "{joined}");
        assert!(joined.contains("/srv/media/media"), "{joined}");
        assert!(joined.contains("searched for a second time"), "{joined}");
    }

    #[test]
    fn a_location_nobody_has_chosen_is_said_without_a_path_rather_than_guessed_at() {
        let said = kept(Protocols::both(), Protocols::none(), None);
        assert_eq!(
            said.first().map(String::as_str),
            Some("everything already downloaded")
        );
    }

    #[test]
    fn adding_one_keeps_nothing_because_it_takes_nothing_away() {
        assert!(kept(Protocols::none(), Protocols::both(), None).is_empty());
        assert!(!reduces(Protocols::none(), Protocols::both()));
    }

    /// A stack of two profiles: one that runs whatever the operator configured,
    /// and one that cannot run without torrents.
    fn a_stack() -> Option<lemonfiber_manifest::Manifest> {
        let toml = r#"
schema_version = 1
stack_version = "0.1.0"
min_cli_version = "0.1.0"

[[profile]]
id = "library"
name = "Library"
description = "Serves what is already here"

[[profile]]
id = "torrent"
name = "Torrents"
description = "Fetches over BitTorrent"
protocol = "torrent"

[[service]]
id = "jellyfin"
name = "Jellyfin"
profile = "library"
image = "jellyfin/jellyfin"
tag = "10.10.3"
criticality = "core"
license = "GPL-2.0"
upstream = "https://jellyfin.org"
last_release = "2026-01-01"
describes = "Plays the library"
without_it = "Nothing plays"

[[service]]
id = "qbittorrent"
name = "qBittorrent"
profile = "torrent"
image = "linuxserver/qbittorrent"
tag = "5.0.3"
criticality = "core"
license = "GPL-2.0"
upstream = "https://qbittorrent.org"
last_release = "2026-01-01"
describes = "Fetches torrents"
without_it = "No torrents"
"#;
        lemonfiber_manifest::Manifest::from_toml(toml).ok()
    }

    #[test]
    fn dropping_torrents_stops_its_own_services_and_leaves_the_library_running() {
        let stops = a_stack()
            .map(|stack| stopped(&stack, Protocols::both(), Protocols::none()))
            .unwrap_or_default();
        assert_eq!(stops, vec!["qbittorrent".to_owned()]);
    }

    #[test]
    fn a_stack_left_with_nothing_to_run_stops_every_service_it_had() {
        // Every profile this stack declares needs torrents, so dropping them leaves
        // the closure with nothing to plan — which is a refusal to plan, not a plan
        // holding everything, and reading it as the latter would report that nothing
        // stops when in fact all of it does.
        let toml = r#"
schema_version = 1
stack_version = "0.1.0"
min_cli_version = "0.1.0"

[[profile]]
id = "torrent"
name = "Torrents"
description = "Fetches over BitTorrent"
protocol = "torrent"

[[service]]
id = "qbittorrent"
name = "qBittorrent"
profile = "torrent"
image = "linuxserver/qbittorrent"
tag = "5.0.3"
criticality = "core"
license = "GPL-2.0"
upstream = "https://qbittorrent.org"
last_release = "2026-01-01"
describes = "Fetches torrents"
without_it = "No torrents"
"#;
        let stops = lemonfiber_manifest::Manifest::from_toml(toml)
            .ok()
            .map(|stack| stopped(&stack, Protocols::both(), Protocols::none()));
        assert_eq!(stops, Some(vec!["qbittorrent".to_owned()]));
    }

    #[test]
    fn a_stack_with_nothing_left_to_run_stops_nothing_it_cannot_name() {
        // The closure refuses to plan a stack with no profile at all rather than
        // answering, and a reduction against one names no service rather than
        // inventing them.
        let bare = lemonfiber_manifest::Manifest::from_toml(
            "schema_version = 1\nstack_version = \"0.1.0\"\nmin_cli_version = \"0.1.0\"\n",
        )
        .ok();
        let stops = bare.map(|stack| stopped(&stack, Protocols::both(), Protocols::none()));
        assert_eq!(stops, Some(Vec::new()));
    }
}
