use lemonfiber_manifest::container::{Fields, Publish};

use super::super::super::installed::{Placed, Reached};
use super::{document, placed, HOUSEHOLD, LOOPBACK};

fn speaking(listens: u16, contract: Option<u16>, port: u16, publish: Vec<Publish>) -> Placed {
    Placed {
        reached: Some(Reached::Household {
            port,
            hostname: "comics".to_owned(),
            group: None,
        }),
        listens: Some(listens),
        contract,
        speaks: vec!["stream.guard@1".to_owned()],
        fields: Fields {
            publish,
            ..Fields::default()
        },
        ..placed()
    }
}

#[test]
fn no_port_the_writer_publishes_reaches_the_port_a_service_speaks_its_contracts_on() {
    let door = document(speaking(
        8096,
        Some(8081),
        8096,
        vec![
            Publish {
                port: 8920,
                listens: None,
            },
            Publish {
                port: 9000,
                listens: Some(8081),
            },
        ],
    ));
    for written in [
        format!("{HOUSEHOLD}:8096:8096"),
        format!("{HOUSEHOLD}:8920:8920"),
        format!("{LOOPBACK}::8081"),
    ] {
        assert!(door.contains(&written), "{written}: {door}");
    }
    assert!(!door.contains("9000:8081"), "got: {door}");

    let adapter = document(speaking(8080, None, 25600, Vec::new()));
    assert!(
        adapter.contains(&format!("{LOOPBACK}::8080")),
        "got: {adapter}"
    );
    assert!(!adapter.contains("25600:8080"), "got: {adapter}");
}
