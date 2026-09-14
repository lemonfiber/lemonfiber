//! The requests the stack's own services make, attributed to them.
//!
//! An indexer query is Prowlarr asking an indexer. A poster is Radarr asking a
//! metadata provider. A peer connection is qBittorrent being a torrent client.
//! Counting any of those as lemonfiber's would overstate what this product does —
//! and leaving them out would understate what running the stack does, which is the
//! thing an operator is actually deciding about.
//!
//! **The stack answers first, and this table answers for the ones that do not.** A
//! service may say in its own manifest entry where it reaches and what it asks for,
//! and where it does, that is what the inventory carries — so adding a service to a
//! Compose project is a change to that project and to nothing here.
//!
//! It was not always so, and what it replaced is worth stating, because the thing it
//! was defending is real. This table was held against the stack by a set-equality
//! test in both directions, so a service arriving in the stack turned this crate red
//! until somebody edited Rust — which is the one thing adding a service to a Compose
//! project must never cost. But an inventory of what leaves a machine is only honest
//! if a service cannot arrive in it unlisted, and that is what the test bought.
//!
//! Both, now, from two directions. A service the stack describes needs nothing here.
//! A service neither source describes is carried into the inventory saying lemonfiber
//! has no record of what it reaches, which is the truth and is neither a guess nor a
//! silent omission. And what is still a build failure is the half that is about this
//! file rather than about somebody's stack: an entry naming a service the stack does
//! not declare is a stale decision or a mistyped id, and a mistyped id would now
//! degrade quietly into "nothing is written down about this", which is exactly the
//! failure the report cannot tell from the real thing.

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

/// Where an unrecorded service is said to reach, which is the one thing that can
/// honestly be said about it.
///
/// A sentence rather than an empty string, because empty already means *nothing
/// leaves this machine* in this report and the two are opposite claims.
const UNKNOWN: &str = "not known to lemonfiber";

/// What is said about a service this build ships no record for.
const NO_RECORD: &str = "This service is not one lemonfiber knows, so nothing here can say \
                         where it reaches or what it asks for. It is listed because leaving \
                         it out would make this inventory read as complete while it was \
                         short. Its own documentation, and the Compose file that declares \
                         it, are what answer this.";

/// What the services in this stack reach, in the order the stack declares them.
///
/// Every service declared, whether or not anything describes it. One nothing
/// describes says so; it is never dropped, and never rendered as reaching nothing.
pub(super) fn elsewhere(services: &[Service]) -> Vec<Elsewhere> {
    services
        .iter()
        .map(|service| from_the_stack(service).unwrap_or_else(|| written_down(service)))
        .collect()
}

/// What a service says about itself, where its own manifest entry says anything.
///
/// Both halves or neither, which the manifest's own validation holds it to: half an
/// answer here would attribute a purpose to a service the same report says goes
/// nowhere. Preferred over the table below because it travels with the stack — a fork
/// describes its own services, and a service added to the stack needs no release of
/// this binary to be described.
fn from_the_stack(service: &Service) -> Option<Elsewhere> {
    let destination = service.reaches.clone()?;
    let purpose = service.asks_for.clone()?;
    Some(Elsewhere {
        service: service.id.clone(),
        destination,
        purpose,
        recorded: true,
    })
}

/// What this build knows about a service that says nothing about itself, or the
/// admission that it knows nothing.
fn written_down(service: &Service) -> Elsewhere {
    ELSEWHERE
        .iter()
        .find(|(id, _, _)| *id == service.id)
        .map_or_else(
            || Elsewhere {
                service: service.id.clone(),
                destination: UNKNOWN.to_owned(),
                purpose: NO_RECORD.to_owned(),
                recorded: false,
            },
            |(_, destination, purpose)| Elsewhere {
                service: service.id.clone(),
                destination: (*destination).to_owned(),
                purpose: (*purpose).to_owned(),
                recorded: true,
            },
        )
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

    /// One direction, and it is the one about this file rather than about somebody's
    /// stack: an entry here that names no declared service is a stale decision or a
    /// mistyped id. The other direction is deliberately not a build failure any more
    /// — a service arriving in the stack with no entry is reported as unknown by the
    /// test below rather than turning this crate red, since adding a service to a
    /// Compose project must not cost a Rust change.
    ///
    /// The mistyped id is why this half stays. Under the old pairing a typo showed up
    /// as a service missing from the list; under the new one it shows up as a service
    /// lemonfiber claims to know nothing about, which reads exactly like the real
    /// thing and would be believed.
    #[test]
    fn nothing_written_down_here_names_a_service_the_stack_does_not_declare() {
        let services: BTreeSet<String> = declared().into_iter().map(|service| service.id).collect();
        let counted = services.len();
        assert!(
            counted > 10,
            "the stack declares {counted} services, which means this is reading the wrong manifest"
        );
        let stale: Vec<&str> = ELSEWHERE
            .iter()
            .map(|(id, _, _)| *id)
            .filter(|id| !services.contains(*id))
            .collect();
        assert!(
            stale.is_empty(),
            "these are written down here and are not in the stack any more, or are spelt \
             differently from the id the stack declares — either way what is written about \
             them reaches nobody: {stale:?}"
        );
    }

    /// Every service the shipped stack declares is answered, from one source or the
    /// other.
    ///
    /// This is the half of the old set-equality worth keeping, and it no longer costs
    /// what that one cost: a service added to the stack answers for itself in its own
    /// manifest entry, which is a change to the stack and not to this binary. What it
    /// refuses is a service that says nothing anywhere — because an inventory of what
    /// leaves a machine is only honest if a service cannot arrive in it unlisted, and
    /// the shipped stack is the one stack whose pairing with this binary is decided
    /// here rather than by an operator.
    #[test]
    fn every_service_the_shipped_stack_declares_is_answered_from_one_source_or_the_other() {
        let silent: Vec<String> = elsewhere(&declared())
            .into_iter()
            .filter(|entry| !entry.recorded)
            .map(|entry| entry.service)
            .collect();
        assert!(
            silent.is_empty(),
            "these are in the stack this build ships and nothing says what they reach. \
             Say it in the stack's own manifest entry — `reaches` and `asks_for`, which \
             travel with the stack — or, failing that, write it down here: {silent:?}"
        );
    }

    /// The manifest wins where it speaks, which is what makes adding a service to a
    /// Compose project cost nothing here.
    #[test]
    fn a_service_that_describes_itself_is_taken_at_its_word() {
        let Some(mut service) = declared().into_iter().next() else {
            unreachable!("the stack declares services")
        };
        service.id = "somebodys-own-service".to_owned();
        service.reaches = Some("a place of their own".to_owned());
        service.asks_for = Some("Whatever they built it to ask for.".to_owned());

        let found = elsewhere(&[service]);
        let entry = found.first();
        assert!(
            entry.is_some_and(|one| one.destination == "a place of their own"),
            "{found:?}"
        );
        assert!(entry.is_some_and(|one| one.recorded), "{found:?}");
    }

    /// An empty destination is an answer, and the whole of what this inventory is for
    /// turns on it reading as one. "Nothing leaves this machine" is the strongest
    /// thing a privacy inventory can say about a service, and a stack that says it
    /// must not come back indistinguishable from a stack that said nothing at all.
    #[test]
    fn a_stack_saying_a_service_reaches_nothing_is_recorded_as_having_said_so() {
        let Some(mut service) = declared().into_iter().next() else {
            unreachable!("the stack declares services")
        };
        service.id = "somebodys-own-service".to_owned();
        service.reaches = Some(String::new());
        service.asks_for = Some("Nothing leaves this machine.".to_owned());

        let found = elsewhere(&[service]);
        let entry = found.first();
        assert!(
            entry.is_some_and(|one| one.destination.is_empty()),
            "{found:?}"
        );
        assert!(entry.is_some_and(|one| one.recorded), "{found:?}");
    }

    /// Half an answer is no answer, and what it falls back to is the table rather
    /// than a blank.
    ///
    /// A stack that says where a service reaches and not what it asks for there has
    /// described a destination with no purpose attached — and a purpose is the half
    /// an operator actually reads, because "it talks to a metadata provider" and
    /// "it sends your library to a metadata provider" are the same destination and
    /// different decisions. The manifest's own validation holds a stack to both
    /// halves or neither, so this is the shape that arrives when something has gone
    /// wrong upstream of it; taking the half offered would attribute a purpose this
    /// service never claimed, or leave one blank in a report whose blanks already
    /// mean *nothing leaves this machine*.
    #[test]
    fn half_an_answer_from_the_stack_is_no_answer_and_falls_back_to_the_table() {
        let found = declared().into_iter().find(|one| one.id == "prowlarr");
        let Some(mut prowlarr) = found else {
            unreachable!("the stack declares an indexer manager")
        };
        prowlarr.reaches = Some("somewhere this binary never heard of".to_owned());
        prowlarr.asks_for = None;

        let carried = elsewhere(&[prowlarr]);
        let entry = carried.first();
        assert!(
            entry.is_some_and(|one| one.destination.contains("indexers")),
            "{carried:?}"
        );
        assert!(entry.is_some_and(|one| one.recorded), "{carried:?}");
    }

    /// And where both speak, the stack's own word is the one carried: the point of
    /// the field is that a fork can correct what this binary believes.
    #[test]
    fn the_stacks_own_word_is_preferred_to_what_is_written_down_here() {
        let Some(mut prowlarr) = declared()
            .into_iter()
            .find(|service| service.id == "prowlarr")
        else {
            unreachable!("the stack declares an indexer manager")
        };
        prowlarr.reaches = Some("somewhere this binary never heard of".to_owned());
        prowlarr.asks_for = Some("Something this binary never heard of either.".to_owned());

        let found = elsewhere(&[prowlarr]);
        assert!(
            found
                .first()
                .is_some_and(|one| one.destination == "somewhere this binary never heard of"),
            "{found:?}"
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

    /// The case the whole rearrangement is for: a service nobody has written anything
    /// about is carried into the inventory saying so, rather than dropped from it.
    #[test]
    fn a_service_nothing_is_written_down_about_is_reported_rather_than_left_out() {
        let unknown: Vec<lemonfiber_manifest::Service> = declared()
            .into_iter()
            .take(1)
            .map(|mut service| {
                service.id = "something-new".to_owned();
                // And says nothing for itself. A stack may now declare where a
                // service reaches, and a renamed entry keeping those two lines is a
                // service this build has been told about rather than one it knows
                // nothing of — which is the opposite of what these cases are.
                service.reaches = None;
                service.asks_for = None;
                service
            })
            .collect();
        assert_eq!(unknown.len(), 1, "the stack declares services to rename");

        let found = elsewhere(&unknown);
        let entry = found.first();
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(
            entry.is_some_and(|one| one.service == "something-new"),
            "{found:?}"
        );
        assert!(entry.is_some_and(|one| !one.recorded), "{found:?}");
    }

    /// And it is not reported as reaching nothing, which is the one wrong answer here.
    ///
    /// An empty destination is what `unpackerr` and `caddy` say, and it means no
    /// request leaves this machine. Saying that about a service nobody has looked at
    /// would be this product making a privacy claim out of its own ignorance.
    #[test]
    fn an_unknown_service_is_never_reported_as_reaching_nothing() {
        let unknown: Vec<lemonfiber_manifest::Service> = declared()
            .into_iter()
            .take(1)
            .map(|mut service| {
                service.id = "something-new".to_owned();
                // And says nothing for itself. A stack may now declare where a
                // service reaches, and a renamed entry keeping those two lines is a
                // service this build has been told about rather than one it knows
                // nothing of — which is the opposite of what these cases are.
                service.reaches = None;
                service.asks_for = None;
                service
            })
            .collect();

        let found = elsewhere(&unknown);
        let entry = found.first();
        assert!(
            entry.is_some_and(|one| !one.destination.is_empty()),
            "an empty destination reads as nothing leaving this machine: {found:?}"
        );
        assert!(
            entry.is_some_and(|one| one.purpose.contains("not one lemonfiber knows")),
            "{found:?}"
        );
    }

    /// Adding a service to the stack is a Compose change and a manifest change and
    /// nothing else. Asserted here because the rule it replaces was asserted here,
    /// and a rule taken out without one to stand in its place comes back.
    #[test]
    fn a_service_added_to_the_stack_needs_no_entry_here_to_be_reported() {
        let mut services = declared();
        let Some(template) = services.first().cloned() else {
            unreachable!("the stack declares services to copy")
        };
        let mut theirs = template;
        theirs.id = "somebodys-own-service".to_owned();
        services.push(theirs);

        let found = elsewhere(&services);
        assert_eq!(
            found.len(),
            services.len(),
            "every declared service reaches the inventory: {found:?}"
        );
    }
}
