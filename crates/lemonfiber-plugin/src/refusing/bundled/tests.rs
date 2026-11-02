use super::{declared, named, routed, routed_to, services, STACK};

/// The register found the stack, rather than finding nothing and passing.
///
/// Held to the shipped description by two facts it would be absurd to lose
/// quietly — that there are services at all, and that the one every other part
/// of this crate's fixtures is written around is among them — because the
/// failure this guards is not a wrong answer. It is no answer: a parse that
/// stopped working leaves every rule over this register walking an empty list,
/// and each of them would go on reporting that a manifest collides with nothing.
#[test]
fn the_register_holds_the_stack_this_build_ships() {
    // Counted before the assertion rather than inside its message. A value a
    // message reaches for is read only where the message is built, which is only
    // where the assertion fails — so the reading is a line no passing run enters,
    // and the coverage gate is right to call it one.
    let shipped = services().len();
    assert!(
        shipped > 10,
        "the shipped stack description was not read, so every rule over this register is \
             walking nothing: {shipped} services"
    );
    assert!(
        named("jellyfin").is_some(),
        "the stack ships a media server"
    );
    assert_eq!(named("jellyfin").and_then(|held| held.port), Some(8095));
}

/// A description this build cannot read holds nothing, and holds it visibly.
///
/// The reading is asked both ways here because the register above cannot ask
/// it either way: it reads one file, once, and an answer of *no services* from
/// it is the same silence whether the file was unreadable or empty.
#[test]
fn a_stack_description_that_cannot_be_read_holds_nothing() {
    assert!(declared("= not a stack description").is_empty());
    assert!(!declared(STACK).is_empty());
}

/// And the names the household reaches the bundle's parts at, for the same reason.
#[test]
fn the_register_holds_the_names_the_household_reaches_the_bundle_at() {
    let answered = routed().len();
    assert!(
        answered >= 5,
        "the bundle's parts were not read, so a plugin could take any name the household \
             already reaches: {answered} names"
    );
    assert!(routed_to("watch").is_some());
    assert!(
        routed_to("comics").is_none(),
        "and nothing the bundle does not name"
    );
}

/// A name nothing declares is not held.
#[test]
fn nothing_the_stack_does_not_ship_is_reserved() {
    assert!(named("komga").is_none());
}

#[test]
fn each_name_is_routed_to_the_stack_service_its_part_stands_in_for() {
    assert_eq!(routed_to("watch"), Some("door"));
    assert_eq!(routed_to("requests"), Some("seerr"));
    assert_eq!(routed_to("nowhere"), None);
    assert_eq!(super::label_of("door"), Some("watch"));
    assert_eq!(super::label_of("nowhere"), None);
}

/// A household service standing in for none of the stack's is routed to its own id.
#[test]
fn a_name_standing_in_for_nothing_is_routed_to_its_own_service() {
    let mut routed = 0;
    for pinned in lemonfiber_bundled::PINNED {
        let Ok(mut part) = crate::Manifest::from_toml(pinned.copy) else {
            continue;
        };
        for service in &mut part.services {
            service.replaces = None;
        }
        for (_, service) in super::labelled(&part) {
            assert!(
                part.services.iter().any(|own| own.id == service),
                "{service}"
            );
            routed += 1;
        }
    }

    assert!(routed > 0, "a pinned part with a household service");
}
