use super::fixtures::{asking, brought, installed, service, watching};
use super::{begins_at, facing, Facing, NAMED};
use lemonfiber_manifest::{ApiKind, Bind};

#[test]
fn the_request_surface_is_the_door_wherever_there_is_one() {
    let services = [watching(), asking()];
    assert_eq!(
        begins_at(&super::candidates(&services, &[]))
            .map(|(facing, candidate)| (facing, candidate.id.to_owned())),
        Some((Facing::Asking, "seerr".to_owned()))
    );
}

#[test]
fn the_library_is_the_door_only_where_nothing_can_be_asked_for() {
    let services = [watching()];
    assert_eq!(
        begins_at(&super::candidates(&services, &[]))
            .map(|(facing, candidate)| (facing, candidate.id.to_owned())),
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
    assert!(begins_at(&super::candidates(&services, &[])).is_none());
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
    let book_curator = service("bindery", Some(Bind::Lan), Some(ApiKind::Bindery));
    assert_eq!(facing(&book_curator), Some(Facing::Unstated));
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

/// A plugin's service published to the household is judged by the adapter it names:
/// a request service asks, a media server watches, anything else is unstated — and the
/// register, the stack's account of its own services, does not reach it by its id. One
/// reachable from this machine alone is no part of this.
#[test]
fn a_plugins_service_is_judged_by_its_adapter_alone() {
    use super::brought as facing_of;

    assert_eq!(
        facing_of(&brought("requests", Some(ApiKind::Seerr), Some("ask"))),
        Some(Facing::Asking)
    );
    assert_eq!(
        facing_of(&brought("server", Some(ApiKind::Jellyfin), Some("watch"))),
        Some(Facing::Watching)
    );
    assert_eq!(
        facing_of(&brought("homepage", None, Some("home"))),
        Some(Facing::Unstated)
    );
    assert_eq!(
        facing_of(&brought("requests", Some(ApiKind::Seerr), None)),
        None
    );
}

/// The stack's own request surface is the door before a plugin's, a plugin's request
/// surface is the door before the stack's library, and a plugin's library is the door
/// where nothing anywhere can be asked for.
#[test]
fn a_plugins_service_can_be_the_door_by_the_same_rule() {
    let requests = installed(vec![brought("requests", Some(ApiKind::Seerr), Some("ask"))]);
    let server = installed(vec![brought(
        "server",
        Some(ApiKind::Jellyfin),
        Some("watch"),
    )]);
    let door = |services: &[lemonfiber_manifest::Service], plugin: &crate::plugin::Installed| {
        let candidates = super::candidates(services, std::slice::from_ref(plugin));
        begins_at(&candidates).map(|(facing, candidate)| (facing, candidate.id.to_owned()))
    };

    assert_eq!(
        door(&[watching(), asking()], &requests),
        Some((Facing::Asking, "seerr".to_owned()))
    );
    assert_eq!(
        door(&[watching()], &requests),
        Some((Facing::Asking, "requests".to_owned()))
    );
    assert_eq!(
        door(&[], &server),
        Some((Facing::Watching, "server".to_owned()))
    );
}

/// A plugin's service is reached through the proxy at its label, and one reachable from
/// this machine alone is reached at no port the household could use.
#[test]
fn a_plugins_service_is_reached_through_the_proxy() {
    let plugin = installed(vec![
        brought("requests", Some(ApiKind::Seerr), Some("ask")),
        brought("hidden", Some(ApiKind::Seerr), None),
    ]);
    let candidates = super::candidates(&[], std::slice::from_ref(&plugin));
    let reached: Vec<super::Reached<'_>> = candidates.iter().map(|one| one.reached).collect();

    assert_eq!(
        reached,
        vec![super::Reached::Proxied("ask"), super::Reached::Port(None)]
    );
    assert_eq!(
        candidates.first().map(|one| one.name),
        Some("requests the plugin's")
    );
}
