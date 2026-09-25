use super::{allowed, destination, registries, registry_of, DOCKER_HUB, EVERY, GUIDE_SOURCE};
use crate::config::{Indexer, Reaching, Settings};
use crate::outbound::Reach;
use lemonfiber_manifest::Service;

/// The stack's own services with their images replaced, because nothing in this
/// workspace builds a `Service` from nothing and a second way of spelling one
/// would be a second idea of what a service is.
fn services(images: &[&str]) -> Vec<Service> {
    let declared = crate::test_support::stack()
        .manifest()
        .map(|manifest| manifest.services)
        .unwrap_or_default();
    assert!(
        !declared.is_empty(),
        "the stack this repository carries declares services"
    );
    let mut built = Vec::new();
    for image in images {
        for service in declared.iter().take(1) {
            let mut copy = service.clone();
            copy.image = (*image).to_owned();
            built.push(copy);
        }
    }
    built
}

#[test]
fn an_image_naming_no_registry_comes_from_docker_hub() {
    assert_eq!(registry_of("caddy"), DOCKER_HUB);
    assert_eq!(registry_of("jellyfin/jellyfin"), DOCKER_HUB);
}

#[test]
fn an_image_naming_a_host_comes_from_that_host() {
    assert_eq!(registry_of("lscr.io/linuxserver/sonarr"), "lscr.io");
    assert_eq!(registry_of("ghcr.io/recyclarr/recyclarr"), "ghcr.io");
    assert_eq!(registry_of("localhost:5000/mine"), "localhost:5000");
}

#[test]
fn each_registry_is_named_once_however_many_images_come_from_it() {
    let stack = services(&[
        "lscr.io/linuxserver/sonarr",
        "lscr.io/linuxserver/radarr",
        "jellyfin/jellyfin",
    ]);
    assert_eq!(stack.len(), 3);
    assert_eq!(registries(&stack), vec!["docker.io", "lscr.io"]);
}

#[test]
fn the_guide_source_is_the_one_the_probe_is_handed() {
    assert_eq!(
        destination(Reach::Guides, &Settings::default(), &[]),
        vec![GUIDE_SOURCE.to_owned()]
    );
}

#[test]
fn an_indexer_is_named_without_the_key_it_authenticates_with() {
    let key = "k".repeat(32);
    let settings = Settings {
        indexer: Some(Indexer {
            url: format!("https://indexer.example/api?apikey={key}"),
            key,
        }),
        ..Settings::default()
    };
    let named = destination(Reach::Indexer, &settings, &[]);
    let shown = named.join(" ");
    assert_eq!(named.len(), 1, "{shown}");
    assert!(shown.starts_with("https://indexer.example/api?"), "{shown}");
    assert!(!shown.contains(&"k".repeat(32)), "{shown}");
}

#[test]
fn a_request_with_nothing_configured_names_nowhere() {
    for reach in [Reach::Indexer, Reach::Usenet] {
        assert!(destination(reach, &Settings::default(), &[]).is_empty());
    }
    // And what a surface puts there instead, which is read from here by the
    // renderer in another crate — so it is exercised from both compilations of
    // this file rather than only from the one that draws it.
    assert!(!super::nothing_configured().is_empty());
}

#[test]
fn a_usenet_provider_is_named_by_the_host_the_operator_gave() {
    let settings = Settings {
        provider_host: Some("news.example.net".to_owned()),
        ..Settings::default()
    };
    assert_eq!(
        destination(Reach::Usenet, &settings, &[]),
        vec!["news.example.net".to_owned()]
    );
}

#[test]
fn the_echo_is_the_sources_in_force_and_is_off_when_there_are_none() {
    let settings = Settings::default();
    assert!(!destination(Reach::Echo, &settings, &[]).is_empty());
    assert!(allowed(Reach::Echo, &settings));
    let switched_off = Settings {
        ip_echo: Vec::new(),
        ..Settings::default()
    };
    assert!(!allowed(Reach::Echo, &switched_off));
}

/// The two the sender may post to are the two the list names, and no others.
///
/// Read from here by [`crate::telling`], which is what makes the enumeration a
/// property rather than a description: a message can only go where an operator
/// reading this was told it could go.
#[test]
fn where_a_household_member_is_told_is_the_two_the_list_names() {
    let named = destination(Reach::Household, &Settings::default(), &[]);

    assert_eq!(named.len(), 2, "{named:?}");
    assert!(
        named.iter().all(|at| at.starts_with("https://")),
        "{named:?}"
    );
    assert!(named.contains(&super::PUSHOVER.to_owned()), "{named:?}");
    assert!(named.contains(&super::PUSHBULLET.to_owned()), "{named:?}");
}

#[test]
fn each_of_the_others_answers_its_own_switch() {
    let others: Vec<&Reach> = EVERY
        .iter()
        .filter(|reach| **reach != Reach::Echo)
        .collect();
    // The filter is what makes this a sweep of *the others*, and it is also what
    // would leave it a sweep of nothing if the set were ever reduced to one.
    assert!(!others.is_empty(), "there are others to sweep");
    for reach in others {
        let refused = Settings {
            reaching: Reaching::none(),
            ..Settings::default()
        };
        assert!(!allowed(*reach, &refused), "{reach:?} ignored its switch");
        assert!(allowed(*reach, &Settings::default()), "{reach:?}");
    }
}
