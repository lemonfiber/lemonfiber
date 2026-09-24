use super::{chosen, Chosen, Refusal, UNDECLARED, WITHHELD};
use crate::door::fixtures::{asking, service, watching};
use crate::door::Facing;
use lemonfiber_manifest::{ApiKind, Bind};

/// How the door was chosen, and which service it came out as.
fn door(
    services: &[lemonfiber_manifest::Service],
    named: Option<&str>,
) -> (Chosen, Option<String>) {
    let (chosen, door) = chosen(services, named);
    (chosen, door.map(|(_, service)| service.id.clone()))
}

/// The refusal a name comes back with, as the two things it carries.
fn refused(named: &str, because: &str) -> Chosen {
    Chosen::Refused(Refusal {
        named: named.to_owned(),
        because: because.to_owned(),
    })
}

#[test]
fn naming_nothing_leaves_the_door_where_the_stack_puts_it() {
    let services = [watching(), asking()];
    assert_eq!(
        door(&services, None),
        (Chosen::Derived, Some("seerr".to_owned())),
        "the request surface, as it was before there was a setting"
    );
    // A blank is a mistake rather than an intent, and reads as no name at all.
    assert_eq!(
        door(&services, Some("   ")),
        (Chosen::Derived, Some("seerr".to_owned()))
    );
}

#[test]
fn the_library_can_be_named_over_the_request_surface() {
    // The disagreement the setting exists for: this stack has somewhere to ask,
    // and this operator wants their household sent to what is already there.
    let services = [asking(), watching()];
    assert_eq!(
        door(&services, Some("jellyfin")),
        (
            Chosen::Named("jellyfin".to_owned()),
            Some("jellyfin".to_owned())
        )
    );
}

#[test]
fn a_name_is_read_the_way_somebody_types_it() {
    let services = [asking(), watching()];
    assert_eq!(
        door(&services, Some(" Jellyfin ")),
        (
            Chosen::Named("jellyfin".to_owned()),
            Some("jellyfin".to_owned())
        ),
        "the id the stack declares, whatever case it was written in"
    );
}

#[test]
fn a_service_the_household_tier_does_not_publish_is_refused_and_said() {
    // The refusal this setting exists to make. An admin service answers this
    // machine alone; a front door pointed at one would hand out the address the
    // binding exists to withhold.
    let services = [
        asking(),
        service("sonarr", Some(Bind::Loopback), Some(ApiKind::Servarr)),
    ];
    assert_eq!(
        door(&services, Some("sonarr")),
        (refused("sonarr", WITHHELD), Some("seerr".to_owned())),
        "refused, and the worked-out door still stands"
    );
    assert!(WITHHELD.contains("change what everybody else gets"));
}

#[test]
fn a_service_this_stack_does_not_declare_at_all_is_refused_and_said() {
    let services = [asking()];
    assert_eq!(
        door(&services, Some("plex")),
        (refused("plex", UNDECLARED), Some("seerr".to_owned()))
    );
}

#[test]
fn the_index_over_every_service_is_refused_in_the_registers_own_words() {
    // Naming it would present the household with a page listing every service
    // this stack runs, which is the one thing the register already says it is
    // not for — so the refusal borrows that sentence rather than writing a
    // second one to keep in step with it.
    let services = [asking(), service("homepage", Some(Bind::Lan), None)];
    assert_eq!(
        door(&services, Some("homepage")),
        (
            refused("homepage", Facing::Operators.because()),
            Some("seerr".to_owned())
        )
    );
}

#[test]
fn a_shelf_is_refused_for_the_reason_it_is_not_a_way_in() {
    let services = [asking(), service("audiobookshelf", Some(Bind::Lan), None)];
    assert_eq!(
        door(&services, Some("audiobookshelf")).0,
        refused("audiobookshelf", Facing::Shelf.because())
    );
}

#[test]
fn a_refused_name_over_a_stack_with_no_door_leaves_there_being_none() {
    // Two absences at once. The setting was wrong and the stack has nowhere to
    // send anybody either, and neither stands in for the other.
    let services = [service(
        "sonarr",
        Some(Bind::Loopback),
        Some(ApiKind::Servarr),
    )];
    assert_eq!(
        door(&services, Some("sonarr")),
        (refused("sonarr", WITHHELD), None)
    );
}

#[test]
fn what_was_named_is_carried_back_as_the_operator_wrote_it() {
    // So the answer names the line they have to go and change, rather than a
    // tidied version of it they then cannot find in their file.
    assert_eq!(
        door(&[asking()], Some("  Not-A-Service  ")).0,
        refused("Not-A-Service", UNDECLARED)
    );
}

#[test]
fn a_screen_with_one_line_is_told_the_same_thing_in_fewer_words() {
    assert_eq!(
        Chosen::Derived.said(),
        None,
        "nobody chose it, so nothing is said"
    );
    assert_eq!(
        Chosen::Named("jellyfin".to_owned()).said(),
        Some("named rather than worked out".to_owned())
    );
    assert_eq!(
        refused("sonarr", WITHHELD).said(),
        Some("`sonarr` was named as the front door and cannot be one".to_owned())
    );
}

#[test]
fn how_a_door_was_chosen_reads_the_same_way_to_a_browser_as_to_a_person() {
    // The field a script reads, pinned: a refusal that only ever appeared inside
    // a sentence would be one every consumer but a reader missed.
    let derived = serde_json::to_value(Chosen::Derived).ok();
    assert_eq!(
        derived,
        Some(serde_json::json!({ "chosen": "derived" })),
        "{derived:?}"
    );
    let named = serde_json::to_value(Chosen::Named("jellyfin".to_owned())).ok();
    assert_eq!(
        named,
        Some(serde_json::json!({ "chosen": "named", "door": "jellyfin" })),
        "{named:?}"
    );
    let refused = serde_json::to_value(refused("sonarr", WITHHELD)).ok();
    assert_eq!(
        refused,
        Some(serde_json::json!({
            "chosen": "refused",
            "door": { "named": "sonarr", "because": WITHHELD },
        })),
        "{refused:?}"
    );
}
