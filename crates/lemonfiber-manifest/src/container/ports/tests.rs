use super::{exposed, exposes, inside, reached, Reach};
use crate::container::Publish;

fn further(port: u16, listens: Option<u16>) -> Publish {
    Publish { port, listens }
}

#[test]
fn every_way_a_port_inside_is_reached_is_named_with_how() {
    assert_eq!(
        reached(
            Some(8096),
            Some(8097),
            &[further(8920, None), further(7000, Some(7001))]
        ),
        vec![
            (Reach::Port, 8097),
            (Reach::Publish, 8920),
            (Reach::Publish, 7001),
            (Reach::Route, 8096),
            (Reach::Ask, 8097),
        ]
    );
    assert_eq!(reached(None, None, &[]), Vec::new());
    assert_eq!((inside(80, None), inside(80, Some(8080))), (80, 8080));
}

#[test]
fn a_contract_port_reached_any_way_but_an_ask_where_it_listens_is_exposed() {
    let door = |port, publish: &[Publish]| exposed(8081, Some(port), Some(8096), publish);
    assert_eq!(door(8096, &[further(8920, None)]), None);
    assert_eq!(
        door(8096, &[further(8920, Some(8081))]),
        Some(Reach::Publish)
    );
    assert_eq!(door(8096, &[further(8081, None)]), Some(Reach::Publish));
    assert_eq!(door(8081, &[]), Some(Reach::Route));
    assert_eq!(
        exposed(8096, Some(8096), Some(8096), &[]),
        Some(Reach::Port)
    );
    assert_eq!(
        exposed(8096, Some(9000), Some(8096), &[]),
        Some(Reach::Port)
    );
    assert_eq!(exposed(8096, Some(8096), None, &[]), Some(Reach::Port));
    assert_eq!(exposed(8080, None, Some(8080), &[]), None);
    assert!(exposes(Reach::Ask, 8081, Some(8081), Some(8096)));
    assert!(!exposes(Reach::Ask, 8080, Some(8080), Some(8080)));
    assert!(!exposes(Reach::Port, 8080, None, Some(8080)));
}

#[test]
fn each_way_names_the_field_that_makes_it_and_says_what_reaches() {
    let named: Vec<(&str, &str)> = [Reach::Port, Reach::Publish, Reach::Route, Reach::Ask]
        .iter()
        .map(|how| (how.field(), how.said()))
        .collect();
    assert_eq!(
        named,
        vec![
            ("port", "the port it publishes"),
            ("publish", "a further port it publishes"),
            ("port", "the proxy's route to it"),
            ("listens", "every service that asks for what it fills"),
        ]
    );
}
