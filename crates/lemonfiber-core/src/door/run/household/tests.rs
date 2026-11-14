use super::*;
use crate::door::Chosen;

fn at(url: &str) -> Address {
    Address {
        url: url.to_owned(),
        caution: None,
    }
}

fn beside(service: &str, facing: Facing, address: Option<Address>, reaches: Reaches) -> Beside {
    Beside {
        service: service.to_owned(),
        facing,
        because: facing.because().to_owned(),
        unaddressed: address.is_none().then(|| "operator words".to_owned()),
        address,
        reaches,
    }
}

fn operators(reaches: Reaches) -> FrontDoorReport {
    FrontDoorReport {
        standing: Standing::Established,
        chosen: Some(Chosen::Derived),
        service: Some("Requests".to_owned()),
        address: Some(at("http://192.168.1.5:5055")),
        reaches,
        unaddressed: None,
        facing: Some(Facing::Asking),
        meaning: "Send them to Requests.".to_owned(),
        beside: vec![
            beside(
                "Jellyfin",
                Facing::Watching,
                Some(at("http://192.168.1.5:8096")),
                reaches,
            ),
            beside("Music", Facing::Shelf, None, Reaches::Unknown),
            beside("Homepage", Facing::Operators, None, Reaches::Unknown),
            beside("Caddy", Facing::Carriage, None, Reaches::Unknown),
            beside("Mystery", Facing::Unstated, None, Reaches::Unknown),
        ],
    }
}

#[test]
fn a_member_at_home_is_handed_the_services_facing_them_in_their_words() {
    let told = told(operators(Reaches::Here));

    assert_eq!(told.chosen, None);
    assert_eq!(told.service.as_deref(), Some("Requests"));
    assert_eq!(told.facing, Some(Facing::Asking));
    assert_eq!(told.address, Some(at("http://192.168.1.5:5055")));
    assert_eq!(told.reaches, Reaches::Here);
    assert_eq!(told.unaddressed, None);
    assert!(told.meaning.starts_with("Start at Requests"));
    let named: Vec<&str> = told.beside.iter().map(|one| one.service.as_str()).collect();
    assert_eq!(named, ["Jellyfin", "Music"]);
    assert_eq!(
        told.beside.first().and_then(|one| one.address.clone()),
        Some(at("http://192.168.1.5:8096"))
    );
    assert_eq!(
        told.beside.first().map(|one| one.because.as_str()),
        Facing::Watching.household()
    );
    assert_eq!(
        told.beside
            .get(1)
            .and_then(|one| one.unaddressed.as_deref()),
        Some(NO_ADDRESS)
    );
}

#[test]
fn a_member_away_is_handed_no_address_that_will_not_answer_and_told_why() {
    let told = told(operators(Reaches::NotHere));

    assert_eq!(told.address, None);
    assert_eq!(told.reaches, Reaches::NotHere);
    assert_eq!(told.unaddressed.as_deref(), Some(AWAY));
    assert_eq!(
        told.beside.first().and_then(|one| one.address.clone()),
        None
    );
    assert_eq!(
        told.beside
            .first()
            .and_then(|one| one.unaddressed.as_deref()),
        Some(AWAY)
    );
}

#[test]
fn a_door_facing_nobody_in_the_house_is_not_named_to_a_member() {
    let mut report = operators(Reaches::Here);
    report.facing = Some(Facing::Unstated);

    let told = told(report);

    assert_eq!(told.service, None);
    assert_eq!(told.facing, None);
    assert_eq!(told.address, None);
    assert_eq!(told.unaddressed, None);
    assert_eq!(told.reaches, Reaches::Unknown);
    assert_eq!(told.standing, Standing::Established);
    assert_eq!(told.meaning, NOWHERE);
}

#[test]
fn every_standing_is_said_in_the_households_words() {
    for (standing, opening) in [
        (Standing::Established, "Start at Requests: ask"),
        (Standing::LibraryOnly, "Start at Requests to watch"),
        (Standing::Unreachable, "Requests isn't answering"),
        (Standing::Stranded, "Requests is running, but"),
    ] {
        assert!(
            meaning(standing, Some("Requests")).starts_with(opening),
            "{standing:?}"
        );
    }
    assert_eq!(meaning(Standing::Absent, Some("Requests")), NOWHERE);
    assert_eq!(meaning(Standing::Established, None), NOWHERE);
}
