use lemonfiber_manifest::container::Mount;

use super::super::document::sources;
use super::super::{generated, Reading};
use crate::test_support::{a_placed, an_installed};
use crate::wiring::Chosen;

#[test]
fn every_source_beneath_the_project_is_named_with_its_service() {
    let mut placed = a_placed("shelf", &[], None);
    placed.takes_data = true;
    placed.fields.mount = vec![Mount {
        from: "cache".to_owned(),
        to: "/cache".to_owned(),
        ro: true,
    }];
    let project = generated(
        &super::carried(),
        &[an_installed("shelf", vec![placed])],
        &Chosen::read(None),
        &Reading::bare(),
    );
    let shelf: Vec<String> = sources(&project.text)
        .into_iter()
        .filter(|(service, _)| service == "shelf")
        .map(|(_, source)| source)
        .collect();
    assert_eq!(shelf, ["./config/shelf", "./mounts/shelf/cache"]);
}

#[test]
fn only_the_configuration_and_mount_trees_are_sources() {
    let text = "services:\n  one:\n    volumes:\n      - library:/data\n      - ./data:/data\n      - /srv/media:/media\n      - ./config/one:/config\n      - ./configured:/elsewhere\n      - ./mounts/one/pay$$day:/pay\n      - type: bind\n  two: {}\n";
    assert_eq!(
        sources(text),
        [
            ("one".to_owned(), "./config/one".to_owned()),
            ("one".to_owned(), "./mounts/one/pay$day".to_owned()),
        ]
    );
}

#[test]
fn a_document_that_does_not_parse_has_no_sources() {
    assert!(sources(":\n  - [").is_empty());
}
