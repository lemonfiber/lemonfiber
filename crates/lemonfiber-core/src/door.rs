//! Which one service the household is sent to, and the honest answer where there
//! is none.
//!
//! The stack publishes several things to the local network, and left to improvise an
//! operator sends three links and a paragraph explaining which is which — the exact
//! complexity this product exists to remove. So the question has one answer, derived
//! here rather than decided again by each surface that shows it.
//!
//! The two that can be a door are told apart by the shape of their API rather than
//! by their name, the way every other service resolution here is, so a stack that
//! ships a different request service under the same shape resolves the same way. The
//! rest speak no API lemonfiber knows, so each is written down below with what it is
//! to the household — and one nobody has written down is not offered, for the reason
//! [`crate::config::display`] withholds a setting nobody vouched for.

mod address;

use serde::Serialize;

use lemonfiber_manifest::{ApiKind, Bind, Service};

pub use address::{address, publishes_a_name, Address};

/// What a service published to the local network is to the people in the house.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Facing {
    /// Where asking for something begins, and where what was asked for is followed.
    /// It links onward to the library, which is what makes it a place to start
    /// rather than one more address to be sent.
    Asking,
    /// The library, where what arrived is watched. A place to start only where there
    /// is nothing to ask for, because somebody sent there has no way to ask.
    Watching,
    /// One kind of media and no other, reached from the library rather than instead
    /// of it.
    Shelf,
    /// An index over every service, including the ones nobody in the house should
    /// learn exist.
    Operators,
    /// How the others are reached, rather than one of them.
    Carriage,
    /// Published to the household, and nothing here says what it is to them.
    Unstated,
}

impl Facing {
    /// Why this is, or is not, somewhere for the household to begin.
    #[must_use]
    pub const fn because(self) -> &'static str {
        match self {
            Self::Asking => {
                "where a request begins, and where what was asked for is followed — and \
                 it links onward to the library"
            }
            Self::Watching => {
                "where the library is watched, and no way to ask for anything that is \
                 not in it yet"
            }
            Self::Shelf => "one kind of media, reached from the library rather than instead of it",
            Self::Operators => {
                "an index over every service, including the ones nobody in the house \
                 should learn exist — the operator's convenience, never a way in"
            }
            Self::Carriage => "how the others are reached, rather than one of them",
            Self::Unstated => {
                "nothing here says what this is to the household, so it is not offered \
                 as somewhere to begin"
            }
        }
    }

    /// Whether the household can be sent here as a place to start.
    #[must_use]
    pub const fn begins(self) -> bool {
        matches!(self, Self::Asking | Self::Watching)
    }
}

/// The services published to the household that speak no API lemonfiber knows.
///
/// A register rather than a rule read off a name, for the reason
/// [`crate::config::display::SHOWN`] is one: a guess is wrong in whichever direction
/// nobody chose, and the service that ends up wrongly offered to a household is by
/// definition the one nobody thought about. So an entry is a decision somebody made
/// and can be reviewed on; anything else is [`Facing::Unstated`] and is offered to
/// nobody.
const NAMED: &[(&str, Facing)] = &[
    ("calibre-web-automated", Facing::Shelf),
    ("audiobookshelf", Facing::Shelf),
    ("homepage", Facing::Operators),
    ("caddy", Facing::Carriage),
];

/// What this service is to the household, or nothing where it is not published to
/// them at all.
///
/// Publication is the manifest's own [`Bind`], so a service reachable only from this
/// machine is not a candidate for anything here — which is the same fact the admin
/// tier's loopback binding rests on, read from the same field rather than from a
/// second list that could disagree with it.
#[must_use]
pub fn facing(service: &Service) -> Option<Facing> {
    if service.bind != Some(Bind::Lan) {
        return None;
    }
    match service.api.as_ref().map(|api| api.kind) {
        Some(ApiKind::Seerr) => Some(Facing::Asking),
        Some(ApiKind::Jellyfin) => Some(Facing::Watching),
        _ => Some(named(&service.id)),
    }
}

/// What the register says this service is, or that nobody has said.
fn named(id: &str) -> Facing {
    NAMED
        .iter()
        .find(|(named, _)| *named == id)
        .map_or(Facing::Unstated, |(_, facing)| *facing)
}

/// The one service the household begins at, from everything the stack declares.
///
/// Whichever request surface the stack has, and the library where it has none. Read
/// from what the stack *declares* rather than from what is up at this moment,
/// deliberately: a request service that is not running is a front door that is down,
/// and answering "the library, then" would hand the household somewhere they cannot
/// ask for anything without ever saying that is what happened.
///
/// Nothing where the stack publishes neither. That is an answer — an operator-only
/// configuration has no household front door — and it is said as one rather than
/// filled in with the nearest thing that would open.
#[must_use]
pub fn begins_at(services: &[Service]) -> Option<(Facing, &Service)> {
    let mut best: Option<(Facing, &Service)> = None;
    for service in services {
        let Some(facing) = facing(service) else {
            continue;
        };
        if !facing.begins() {
            continue;
        }
        if facing == Facing::Asking {
            return Some((facing, service));
        }
        if best.is_none() {
            best = Some((facing, service));
        }
    }
    best
}

/// A manifest service built for a test, for the two modules that need one.
///
/// Beside the rule rather than inside either test module: what a stack declares is
/// this file's vocabulary, and a second copy of an eighteen-field literal is a
/// second thing to keep agreeing with the schema.
#[cfg(test)]
pub(crate) mod fixtures {
    use lemonfiber_manifest::{Api, ApiKind, Bind, Criticality, KeySource, Service};

    /// A service as the manifest declares one, varied by the three fields the rule
    /// reads.
    pub(crate) fn service(id: &str, bind: Option<Bind>, api: Option<ApiKind>) -> Service {
        Service {
            id: id.to_owned(),
            name: id.to_owned(),
            profile: "media".to_owned(),
            image: "image".to_owned(),
            tag: "1".to_owned(),
            port: Some(1),
            bind,
            health: None,
            api: api.map(|kind| Api {
                kind,
                key_source: KeySource::Generated,
                path: None,
                version: None,
            }),
            criticality: Criticality::Important,
            license: "MIT".to_owned(),
            upstream: "https://example.invalid".to_owned(),
            last_release: "2026-01-01".to_owned(),
            describes: "a service".to_owned(),
            without_it: "nothing".to_owned(),
            media_types: Vec::new(),
            depends_on: Vec::new(),
            capabilities: Vec::new(),
            host_managed: false,
        }
    }

    /// The request surface, as this stack declares it.
    pub(crate) fn asking() -> Service {
        service("seerr", Some(Bind::Lan), Some(ApiKind::Seerr))
    }

    /// The library, as this stack declares it.
    pub(crate) fn watching() -> Service {
        service("jellyfin", Some(Bind::Lan), Some(ApiKind::Jellyfin))
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures::{asking, service, watching};
    use super::{begins_at, facing, Facing, NAMED};
    use lemonfiber_manifest::{ApiKind, Bind};

    #[test]
    fn the_request_surface_is_the_door_wherever_there_is_one() {
        let services = [watching(), asking()];
        assert_eq!(
            begins_at(&services).map(|(facing, service)| (facing, service.id.clone())),
            Some((Facing::Asking, "seerr".to_owned()))
        );
    }

    #[test]
    fn the_library_is_the_door_only_where_nothing_can_be_asked_for() {
        let services = [watching()];
        assert_eq!(
            begins_at(&services).map(|(facing, service)| (facing, service.id.clone())),
            Some((Facing::Watching, "jellyfin".to_owned()))
        );
    }

    #[test]
    fn a_stack_publishing_nothing_to_the_household_has_no_door_at_all() {
        // An operator-only configuration. Answered as none rather than filled in
        // with the nearest thing that would open.
        let services = [
            service("sonarr", Some(Bind::Loopback), Some(ApiKind::Servarr)),
            service("homepage", Some(Bind::Lan), None),
        ];
        assert!(begins_at(&services).is_none());
    }

    #[test]
    fn the_operators_index_is_never_offered_as_a_way_in() {
        let homepage = service("homepage", Some(Bind::Lan), None);
        assert_eq!(facing(&homepage), Some(Facing::Operators));
        assert!(!Facing::Operators.begins());
        assert!(Facing::Operators.because().contains("never a way in"));
    }

    #[test]
    fn a_service_reachable_only_from_this_machine_is_no_part_of_this() {
        let admin = service("sonarr", Some(Bind::Loopback), Some(ApiKind::Servarr));
        assert_eq!(facing(&admin), None);
        let unpublished = service("recyclarr", None, None);
        assert_eq!(facing(&unpublished), None);
    }

    #[test]
    fn a_shelf_is_reached_from_the_library_rather_than_instead_of_it() {
        for id in ["calibre-web-automated", "audiobookshelf"] {
            let shelf = service(id, Some(Bind::Lan), None);
            assert_eq!(facing(&shelf), Some(Facing::Shelf), "{id}");
            assert!(!Facing::Shelf.begins());
        }
    }

    #[test]
    fn the_proxy_carries_the_others_rather_than_being_one_of_them() {
        let caddy = service("caddy", Some(Bind::Lan), None);
        assert_eq!(facing(&caddy), Some(Facing::Carriage));
        assert!(!Facing::Carriage.begins());
    }

    #[test]
    fn a_published_service_nobody_vouched_for_is_offered_to_nobody() {
        let stranger = service("something-new", Some(Bind::Lan), None);
        assert_eq!(facing(&stranger), Some(Facing::Unstated));
        assert!(!Facing::Unstated.begins());
    }

    #[test]
    fn a_download_client_published_to_the_household_is_still_not_a_door() {
        // The api arm falls through to the register for every shape but the two,
        // so a stack that published one of these would not have made it a way in.
        for kind in [ApiKind::Servarr, ApiKind::Sabnzbd, ApiKind::Qbittorrent] {
            let published = service("client", Some(Bind::Lan), Some(kind));
            assert_eq!(facing(&published), Some(Facing::Unstated), "{kind:?}");
        }
        let bindery = service("bindery", Some(Bind::Lan), Some(ApiKind::Bindery));
        assert_eq!(facing(&bindery), Some(Facing::Unstated));
    }

    #[test]
    fn every_facing_says_why_it_is_or_is_not_a_way_in() {
        let said: Vec<&str> = [
            Facing::Asking,
            Facing::Watching,
            Facing::Shelf,
            Facing::Operators,
            Facing::Carriage,
            Facing::Unstated,
        ]
        .into_iter()
        .map(Facing::because)
        .collect();
        for because in &said {
            assert!(!because.is_empty());
        }
        let mut unique = said.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), said.len(), "each says something of its own");
    }

    #[test]
    fn the_register_names_nothing_the_shape_of_an_api_already_answers() {
        // A name here for the request service or the library would be a second
        // opinion about the two the shape of the API already decides.
        assert!(!NAMED
            .iter()
            .any(|(id, _)| *id == "seerr" || *id == "jellyfin"));
    }
}
