use super::super::Fillers;
use super::shipped;
use crate::origin::Origin;
use crate::test_support::{a_placed, an_installed};
use crate::wiring::Chosen;

/// The id of what fills `capability` and of what asks for it, where one service fills it.
fn filled_by(fillers: &Fillers, capability: &str) -> Option<(String, Option<String>)> {
    fillers
        .filling(capability)
        .map(|(filler, asker)| (filler.id.clone(), asker.map(|one| one.id.clone())))
}

/// A capability the stack asks for is filled by what the ask settled on, with the
/// service that asked; one the ask leaves contested is filled by nothing.
#[test]
fn a_capability_asked_for_is_filled_as_the_ask_settled() {
    let fillers = shipped(&[], &Chosen::default(), |_| ());
    assert_eq!(
        filled_by(&fillers, "identity.source"),
        Some(("jellyfin".to_owned(), Some("decline".to_owned())))
    );

    let rival = an_installed(
        "emby",
        vec![a_placed("emby", &["identity.source"], Some(8920))],
    );
    let contested = shipped(&[rival], &Chosen::default(), |_| ());
    assert_eq!(filled_by(&contested, "identity.source"), None);
}

/// A capability nothing asks for is filled by the one service providing it, with
/// nobody asking; by nothing where none provides it, or two do and nothing settles
/// which.
#[test]
fn a_capability_nothing_asks_for_is_filled_by_its_one_provider() {
    let fillers = shipped(&[], &Chosen::default(), |_| ());
    assert_eq!(
        filled_by(&fillers, "request.intake"),
        Some(("seerr".to_owned(), None))
    );
    assert_eq!(filled_by(&fillers, "nothing.provides"), None);

    let second = an_installed(
        "intake",
        vec![a_placed("requests", &["request.intake"], Some(8080))],
    );
    let two = shipped(&[second], &Chosen::default(), |_| ());
    assert_eq!(filled_by(&two, "request.intake"), None);
}

/// A link by name reaches the stack's own service it names, where the stack runs it, and
/// nothing for a service that makes no such link or names one the stack does not run.
#[test]
fn a_link_by_name_reaches_the_stacks_own_service_it_names() {
    let fillers = shipped(&[], &Chosen::default(), |_| ());
    let gone = shipped(&[], &Chosen::default(), |manifest| {
        manifest.services.retain(|service| service.id != "jellyfin");
    });

    assert_eq!(
        fillers.named_by("decline").map(|one| one.id.as_str()),
        Some("jellyfin")
    );
    assert_eq!(
        fillers.named_by("decline").map(|one| one.origin.clone()),
        Some(Origin::Bundled)
    );
    assert_eq!(fillers.named_by("seerr"), None);
    assert_eq!(gone.named_by("decline"), None);
}
