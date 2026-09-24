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
