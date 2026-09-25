//! The seven requests lemonfiber makes on its own account.
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
    Settings, IP_ECHO_KEY, REACH_GUIDES_KEY, REACH_HOUSEHOLD_KEY, REACH_INDEXER_KEY,
    REACH_REGISTRY_KEY, REACH_UPDATES_KEY, REACH_USENET_KEY,
};
use lemonfiber_manifest::Service;

/// Every request lemonfiber makes, in the order an operator meets them: what the
/// stack is built from, what keeps it current, the three that prove something, the
/// one that carries a sentence to somebody who lives here, and the one this program
/// makes about itself.
pub const EVERY: &[Reach] = &[
    Reach::Registry,
    Reach::Guides,
    Reach::Echo,
    Reach::Indexer,
    Reach::Usenet,
    Reach::Household,
    Reach::Updates,
];

/// The repository the community quality guides are synced from, probed for
/// reachability rather than read.
///
/// A hand-maintained literal — Recyclarr does not publish where it syncs from in a
/// form anything here could read — so it must be changed by hand if the upstream
/// moves, or both the probe and the list below name a source unrelated to what
/// actually syncs.
pub const GUIDE_SOURCE: &str = "https://github.com/TRaSH-Guides/Guides";

/// Where a member who is reached on Pushover is reached.
///
/// Declared here for the reason the guide source is, and with one more reason of its
/// own: this is the only entry whose destination is somebody else's choice, so the
/// sender is handed these two rather than holding addresses the list does not name.
/// Both are the addresses the request service's own agents post to, read off
/// `ghcr.io/seerr-team/seerr:v3.3.0` — so what arrives, arrives where that service
/// already sends the same person.
pub const PUSHOVER: &str = "https://api.pushover.net/1/messages.json";

/// Where a member who is reached on Pushbullet is reached.
pub const PUSHBULLET: &str = "https://api.pushbullet.com/v2/pushes";

/// Where the list of lemonfiber's own releases is read.
///
/// The list rather than the address that serves "the latest one", and that is forced
/// rather than chosen: every release of this project is published as a pre-release,
/// and the latest-release address passes over pre-releases — so it answers with
/// nothing at all and a check built on it would report, for ever, that there is no
/// version to move to.
///
/// Declared here for the reason the guide source is: the list an operator reads owns
/// the address, and the check is handed it, so nothing can ask somewhere this page
/// does not name.
pub const RELEASE_LIST: &str = "https://api.github.com/repos/lemonfiber/lemonfiber/releases";

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
            vec![crate::error::withheld::without_credentials(&indexer.url)]
        }),
        Reach::Usenet => settings
            .provider_host
            .as_ref()
            .map_or_else(Vec::new, |host| vec![host.clone()]),
        Reach::Household => vec![PUSHOVER.to_owned(), PUSHBULLET.to_owned()],
        Reach::Updates => vec![RELEASE_LIST.to_owned()],
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
        Reach::Household => {
            "Carry the reason a request was turned down to the person who asked for it, \
             because the request service tells them it was declined and has nowhere to put \
             a reason — so a refusal that reached them alone is the silent one this exists \
             to prevent."
        }
        Reach::Updates => {
            "Read which version of lemonfiber has been released, so an operator asking \
             whether theirs is current gets an answer rather than a shrug. Nothing waits on \
             it and nothing stops without it."
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
        Reach::Household => {
            "One short message and the member's own token for the service it goes to. The \
             message is the word \"Why\" and the reason you typed, and nothing else: not this \
             program's name, not an address, not what was asked for — the request service \
             keeps no title — and nothing about this machine or the household. It goes only \
             where that member already told the request service to reach them, and only if \
             they left refusals switched on there."
        }
        Reach::Updates => {
            "One unauthenticated request for a list of releases. No credential, no setting, \
             nothing about this machine and not even the version running — the answer is \
             compared here rather than there. The one thing that travels is a name, which \
             the address requires of anybody asking and which is the same word in every \
             copy of this program."
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
        Reach::Household => REACH_HOUSEHOLD_KEY,
        Reach::Updates => REACH_UPDATES_KEY,
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
        Reach::Household => {
            "A reason reaches nobody but you. The refusal itself still arrives — the request \
             service sends that — so somebody is told no and never told why, and passing the \
             words on becomes yours to do by hand. The reason is still written down here and \
             still said back to you when you turn a request down."
        }
        Reach::Updates => {
            "The version report stops saying whether this copy is the newest, and answers \
             that it could not tell. Nothing else changes: lemonfiber never replaces itself \
             and every other thing it does works exactly as well on an old one."
        }
    }
}

/// What is said where a request has nowhere configured to go.
#[must_use]
pub const fn nothing_configured() -> &'static str {
    NOTHING_CONFIGURED
}

#[cfg(test)]
mod tests;
