//! The five requests lemonfiber makes on its own account.
//!
//! Each answers four questions, and the answers are prose because the reader is a
//! person deciding whether they are comfortable with it. *Where* it goes is read
//! from this machine instead, because a written-down destination is a claim that
//! goes stale the moment an operator points a setting somewhere else.
//!
//! The guide source is declared here rather than beside the check that probes it,
//! and that is the direction the dependency has to run: a list an operator reads is
//! worth nothing if the code can reach somewhere the list does not name, so the
//! list owns the address and the check is handed it.

use super::{Outbound, Reach};
use crate::config::{
    Settings, IP_ECHO_KEY, REACH_GUIDES_KEY, REACH_INDEXER_KEY, REACH_REGISTRY_KEY,
    REACH_USENET_KEY,
};
use lemonfiber_manifest::Service;

/// Every request lemonfiber makes, in the order an operator meets them: what the
/// stack is built from, what keeps it current, and the three that prove something.
pub const EVERY: &[Reach] = &[
    Reach::Registry,
    Reach::Guides,
    Reach::Echo,
    Reach::Indexer,
    Reach::Usenet,
];

/// The repository the community quality guides are synced from, probed for
/// reachability rather than read.
///
/// A hand-maintained literal — Recyclarr does not publish where it syncs from in a
/// form anything here could read — so it must be changed by hand if the upstream
/// moves, or both the probe and the list below name a source unrelated to what
/// actually syncs.
pub const GUIDE_SOURCE: &str = "https://github.com/TRaSH-Guides/Guides";

/// What an image with no registry in its name is fetched from.
const DOCKER_HUB: &str = "docker.io";

/// Where a request goes, said once for the case where nothing is configured to
/// reach.
const NOTHING_CONFIGURED: &str = "nothing configured";

/// One request, filled in against this machine.
pub(super) fn outbound(reach: Reach, settings: &Settings, services: &[Service]) -> Outbound {
    Outbound {
        reach,
        destination: destination(reach, settings, services),
        purpose: purpose(reach).to_owned(),
        sends: sends(reach).to_owned(),
        allowed: allowed(reach, settings),
        switch: switch(reach).to_owned(),
        cost: cost(reach).to_owned(),
    }
}

/// Where a request goes as this machine stands.
fn destination(reach: Reach, settings: &Settings, services: &[Service]) -> Vec<String> {
    match reach {
        Reach::Registry => registries(services),
        Reach::Guides => vec![GUIDE_SOURCE.to_owned()],
        Reach::Echo => settings.ip_echo.clone(),
        Reach::Indexer => settings.indexer.as_ref().map_or_else(Vec::new, |indexer| {
            vec![lemonfiber_ports::withheld::without_credentials(
                &indexer.url,
            )]
        }),
        Reach::Usenet => settings
            .provider_host
            .as_ref()
            .map_or_else(Vec::new, |host| vec![host.clone()]),
    }
}

/// The registries the images in this stack are fetched from, each named once.
///
/// Read from the manifest rather than written down, because which registry an image
/// comes from is a property of the image and the stack chooses its own images.
fn registries(services: &[Service]) -> Vec<String> {
    let mut found: Vec<String> = services
        .iter()
        .map(|service| registry_of(&service.image).to_owned())
        .collect();
    found.sort_unstable();
    found.dedup();
    found
}

/// The registry an image reference names, or Docker Hub where it names none.
///
/// A first segment is a registry when it looks like a host — a dot or a port — and
/// is otherwise the first half of a Docker Hub namespace: `jellyfin/jellyfin` is on
/// Docker Hub and `lscr.io/linuxserver/jellyfin` is not.
fn registry_of(image: &str) -> &str {
    match image.split_once('/') {
        Some((head, _)) if head.contains('.') || head.contains(':') => head,
        _ => DOCKER_HUB,
    }
}

/// Whether this machine's settings allow a request.
///
/// The echo answers for itself, because its setting names a source as well as
/// saying yes or no and two readings of that would eventually disagree; every other
/// request is allowed unless its own setting was switched off.
fn allowed(reach: Reach, settings: &Settings) -> bool {
    if matches!(reach, Reach::Echo) {
        return !settings.ip_echo.is_empty();
    }
    settings.reaching.allows(switch(reach))
}

/// Why lemonfiber asks.
pub fn purpose(reach: Reach) -> &'static str {
    match reach {
        Reach::Registry => {
            "Fetch the service images this stack runs, and the newer ones when it is updated."
        }
        Reach::Guides => {
            "Confirm the source Recyclarr syncs the community quality profiles from can be \
             reached, so a sync that would bring nothing back is noticed rather than mistaken \
             for a preset that has no effect."
        }
        Reach::Echo => {
            "Read the public address the download client's traffic comes out of, so it can be \
             compared with this machine's own and a tunnel that is not carrying it is caught."
        }
        Reach::Indexer => {
            "Prove the indexer key works by making one search with it, so a key that will not \
             work is caught while somebody is still sitting at the setup rather than weeks later."
        }
        Reach::Usenet => {
            "Prove the Usenet login works by making it, so a password that will not work is \
             caught at setup rather than as downloads that never start."
        }
    }
}

/// Exactly what travels.
pub fn sends(reach: Reach) -> &'static str {
    match reach {
        Reach::Registry => {
            "The name and tag of each image, to whichever registry that image names. Nothing \
             about this machine, this stack, or the person running it."
        }
        Reach::Guides => {
            "An unauthenticated request for the repository page, and nothing else. No \
             credential, no version, nothing that would distinguish this installation from \
             any other."
        }
        Reach::Echo => {
            "Nothing but the request itself. It is made from inside the download client's own \
             container rather than by lemonfiber, because the address worth knowing is the one \
             that container's traffic leaves from."
        }
        Reach::Indexer => {
            "One search, with the API key the operator gave. The indexer sees that search and \
             that key, which is what proving the key means; nothing about this machine goes \
             with it."
        }
        Reach::Usenet => {
            "The username and password the operator gave, over TLS. A plaintext connection is \
             refused rather than downgraded, so the password is never sent in the clear."
        }
    }
}

/// The setting that switches a request off.
pub fn switch(reach: Reach) -> &'static str {
    match reach {
        Reach::Registry => REACH_REGISTRY_KEY,
        Reach::Guides => REACH_GUIDES_KEY,
        Reach::Echo => IP_ECHO_KEY,
        Reach::Indexer => REACH_INDEXER_KEY,
        Reach::Usenet => REACH_USENET_KEY,
    }
}

/// What stops working once a request is switched off.
pub fn cost(reach: Reach) -> &'static str {
    match reach {
        Reach::Registry => {
            "Nothing can be installed or updated. A service whose image is not already on this \
             machine will not start, and one that is stays at the version it already has."
        }
        Reach::Guides => {
            "The diagnosis stops confirming the quality-guide source is reachable and reports \
             that it did not look. Recyclarr goes on syncing to its own schedule and the \
             profiles already in place are unaffected."
        }
        Reach::Echo => {
            "Leak detection stops. A tunnel that has quietly fallen back to this machine's own \
             address goes unnoticed, which is the one failure here whose consequences reach \
             outside the machine."
        }
        Reach::Indexer => {
            "An indexer key is recorded as unverified rather than proven. A key that has \
             rotted then shows up as searches that find nothing, weeks after it stopped working."
        }
        Reach::Usenet => {
            "A Usenet login is recorded as unverified rather than proven, and a wrong password \
             shows up as downloads that never start rather than as an answer at setup."
        }
    }
}

/// What is said where a request has nowhere configured to go.
#[must_use]
pub const fn nothing_configured() -> &'static str {
    NOTHING_CONFIGURED
}

#[cfg(test)]
mod tests {
    use super::{allowed, destination, registries, registry_of, DOCKER_HUB, EVERY, GUIDE_SOURCE};
    use crate::config::{Indexer, Reaching, Settings};
    use crate::outbound::Reach;
    use lemonfiber_manifest::Service;

    /// The stack's own services with their images replaced, because nothing in this
    /// workspace builds a `Service` from nothing and a second way of spelling one
    /// would be a second idea of what a service is.
    fn services(images: &[&str]) -> Vec<Service> {
        let declared = crate::test_support::stack()
            .manifest()
            .map(|manifest| manifest.services)
            .unwrap_or_default();
        assert!(
            !declared.is_empty(),
            "the stack this repository carries declares services"
        );
        let mut built = Vec::new();
        for image in images {
            for service in declared.iter().take(1) {
                let mut copy = service.clone();
                copy.image = (*image).to_owned();
                built.push(copy);
            }
        }
        built
    }

    #[test]
    fn an_image_naming_no_registry_comes_from_docker_hub() {
        assert_eq!(registry_of("caddy"), DOCKER_HUB);
        assert_eq!(registry_of("jellyfin/jellyfin"), DOCKER_HUB);
    }

    #[test]
    fn an_image_naming_a_host_comes_from_that_host() {
        assert_eq!(registry_of("lscr.io/linuxserver/sonarr"), "lscr.io");
        assert_eq!(registry_of("ghcr.io/recyclarr/recyclarr"), "ghcr.io");
        assert_eq!(registry_of("localhost:5000/mine"), "localhost:5000");
    }

    #[test]
    fn each_registry_is_named_once_however_many_images_come_from_it() {
        let stack = services(&[
            "lscr.io/linuxserver/sonarr",
            "lscr.io/linuxserver/radarr",
            "jellyfin/jellyfin",
        ]);
        assert_eq!(stack.len(), 3);
        assert_eq!(registries(&stack), vec!["docker.io", "lscr.io"]);
    }

    #[test]
    fn the_guide_source_is_the_one_the_probe_is_handed() {
        assert_eq!(
            destination(Reach::Guides, &Settings::default(), &[]),
            vec![GUIDE_SOURCE.to_owned()]
        );
    }

    #[test]
    fn an_indexer_is_named_without_the_key_it_authenticates_with() {
        let key = "k".repeat(32);
        let settings = Settings {
            indexer: Some(Indexer {
                url: format!("https://indexer.example/api?apikey={key}"),
                key,
            }),
            ..Settings::default()
        };
        let named = destination(Reach::Indexer, &settings, &[]);
        let shown = named.join(" ");
        assert_eq!(named.len(), 1, "{shown}");
        assert!(shown.starts_with("https://indexer.example/api?"), "{shown}");
        assert!(!shown.contains(&"k".repeat(32)), "{shown}");
    }

    #[test]
    fn a_request_with_nothing_configured_names_nowhere() {
        for reach in [Reach::Indexer, Reach::Usenet] {
            assert!(destination(reach, &Settings::default(), &[]).is_empty());
        }
        // And what a surface puts there instead, which is read from here by the
        // renderer in another crate — so it is exercised from both compilations of
        // this file rather than only from the one that draws it.
        assert!(!super::nothing_configured().is_empty());
    }

    #[test]
    fn a_usenet_provider_is_named_by_the_host_the_operator_gave() {
        let settings = Settings {
            provider_host: Some("news.example.net".to_owned()),
            ..Settings::default()
        };
        assert_eq!(
            destination(Reach::Usenet, &settings, &[]),
            vec!["news.example.net".to_owned()]
        );
    }

    #[test]
    fn the_echo_is_the_sources_in_force_and_is_off_when_there_are_none() {
        let settings = Settings::default();
        assert!(!destination(Reach::Echo, &settings, &[]).is_empty());
        assert!(allowed(Reach::Echo, &settings));
        let switched_off = Settings {
            ip_echo: Vec::new(),
            ..Settings::default()
        };
        assert!(!allowed(Reach::Echo, &switched_off));
    }

    #[test]
    fn each_of_the_other_four_answers_its_own_switch() {
        for reach in EVERY.iter().filter(|reach| **reach != Reach::Echo) {
            let refused = Settings {
                reaching: Reaching::none(),
                ..Settings::default()
            };
            assert!(!allowed(*reach, &refused), "{reach:?} ignored its switch");
            assert!(allowed(*reach, &Settings::default()), "{reach:?}");
        }
    }
}
