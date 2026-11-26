use lemonfiber_fixtures::unhoused::unhoused;

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
    assert_eq!(told.service.as_deref(), Some("Where you ask"));
    assert_eq!(told.facing, Some(Facing::Asking));
    assert_eq!(told.address, Some(at("http://192.168.1.5:5055")));
    assert_eq!(told.reaches, Reaches::Here);
    assert_eq!(told.unaddressed, None);
    assert_eq!(told.meaning, ASK_FIRST);
    let named: Vec<&str> = told.beside.iter().map(|one| one.service.as_str()).collect();
    assert_eq!(named, ["Where you watch", "A shelf"]);
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
    for (standing, said) in [
        (Standing::Established, ASK_FIRST),
        (Standing::LibraryOnly, WATCH_FIRST),
        (Standing::Unreachable, NOT_ANSWERING),
        (Standing::Stranded, NOT_SET_UP),
        (Standing::Absent, NOWHERE),
    ] {
        assert_eq!(meaning(standing, true), said, "{standing:?}");
    }
    assert_eq!(meaning(Standing::Established, false), NOWHERE);
}

#[test]
fn nothing_a_member_is_told_of_the_door_names_a_service_or_is_out_of_the_households_words() {
    let standings = [
        Standing::Established,
        Standing::LibraryOnly,
        Standing::Unreachable,
        Standing::Stranded,
        Standing::Absent,
    ];
    let facings = [
        Facing::Asking,
        Facing::Watching,
        Facing::Shelf,
        Facing::Operators,
        Facing::Carriage,
        Facing::Unstated,
    ];
    for standing in standings {
        for facing in facings {
            for reaches in [Reaches::Here, Reaches::NotHere, Reaches::Unknown] {
                let mut report = operators(reaches);
                report.standing = standing;
                report.facing = Some(facing);
                report.address = None;
                let told = told(report);
                let mut said = vec![told.meaning.clone()];
                said.extend(told.service.clone());
                said.extend(told.unaddressed.clone());
                for beside in &told.beside {
                    said.push(beside.service.clone());
                    said.push(beside.because.clone());
                    said.extend(beside.unaddressed.clone());
                }
                for text in said {
                    assert_eq!(unhoused(&text), None, "{standing:?} {facing:?}: {text}");
                    assert!(
                        ["Requests", "Jellyfin", "Music", "Homepage", "Caddy", "Mystery"]
                            .iter()
                            .all(|name| !text.contains(name)),
                        "{standing:?} {facing:?}: {text}"
                    );
                }
            }
        }
    }
}
