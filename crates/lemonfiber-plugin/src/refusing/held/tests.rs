use lemonfiber_contract::capabilities::proxy::front;

use super::{
    answered, answering, held, joined, on_port, proxied, spelled_as, taking, Answered, Held,
};

fn holder(id: &str) -> Option<&'static str> {
    taking(held(), "", id).map(|one| one.plugin.as_str())
}

#[test]
fn every_service_of_every_pinned_part_is_held_by_its_plugin() {
    let mut counted = 0;
    for pinned in lemonfiber_bundled::PINNED {
        let Ok(part) = crate::Manifest::from_toml(pinned.copy) else {
            continue;
        };
        for service in &part.services {
            assert_eq!(
                holder(&service.id),
                Some(pinned.first_party.plugin),
                "{}",
                service.id
            );
            counted += 1;
        }
    }
    assert!(counted > 20, "{counted} services");
}

#[test]
fn an_installed_parts_adapter_is_held_by_its_plugin() {
    assert_eq!(holder("jellyfin-adapter"), Some("jellyfin"));
    assert_eq!(holder("caddy-adapter"), Some("caddy"));
}

#[test]
fn every_service_the_stack_ships_is_held_by_a_pinned_plugin() {
    let pinned: Vec<&str> = lemonfiber_bundled::PINNED
        .iter()
        .map(|pinned| pinned.first_party.plugin)
        .collect();
    for service in super::bundled::services() {
        let plugin = holder(&service.id);
        assert!(
            plugin.is_some_and(|plugin| pinned.contains(&plugin)),
            "{} is held by {plugin:?}",
            service.id
        );
    }
}

#[test]
fn nothing_is_held_against_the_plugin_holding_it() {
    assert_eq!(taking(held(), "jellyfin", "jellyfin-adapter"), None);
    assert_eq!(
        taking(held(), "komga", "jellyfin-adapter").map(|one| one.plugin.as_str()),
        Some("jellyfin")
    );
    assert_eq!(spelled_as(held(), "jellyfin", "Jellyfin_Adapter"), None);
    assert_eq!(
        spelled_as(held(), "komga", "Jellyfin_Adapter").map(|one| one.id.as_str()),
        Some("jellyfin-adapter")
    );
}

#[test]
fn a_port_is_held_by_the_plugin_publishing_it() {
    let held = [Held {
        plugin: "door".to_owned(),
        id: "door".to_owned(),
        name: "Door".to_owned(),
        ports: vec![8096, 8920],
    }];
    assert_eq!(
        on_port(&held, "komga", 8920).map(|one| one.plugin.as_str()),
        Some("door")
    );
    assert_eq!(on_port(&held, "door", 8920), None);
    assert_eq!(on_port(&held, "komga", 8921), None);
    assert_eq!(
        on_port(super::held(), "komga", 8096).map(|one| one.plugin.as_str()),
        Some("door")
    );
}

#[test]
fn a_name_the_household_reaches_is_held_by_the_plugin_answering_on_it() {
    assert_eq!(answering(answered(), "komga", "watch"), Some("door"));
    assert_eq!(answering(answered(), "door", "watch"), None);
    assert_eq!(answering(answered(), "komga", "comics"), None);
}

#[test]
fn the_proxy_configuration_names_every_label_it_answers_on() {
    let configuration = br#"{"apps":{"http":{"servers":{"one":{"routes":[
        {"match":[{"host":["status.home.local","watch.home.local"]}],
         "handle":[{"handler":"subroute","routes":[{"match":[{"host":["inner.example"]}]}]}]},
        {"match":[{"path":["/x"]}]}
    ]}}}}}"#;
    let files: &[(&str, &[u8])] = &[
        ("config/proxy/proxy.json", configuration),
        (
            "config/proxy/notes.txt",
            br#"{"match":[{"host":["text.home"]}]}"#,
        ),
        (
            "recordings/proxy/one.json",
            br#"{"match":[{"host":["recorded.home"]}]}"#,
        ),
        ("config/proxy/broken.json", b"{"),
    ];
    assert_eq!(
        proxied(&[front::CAPABILITY], files),
        ["inner", "status", "watch"]
    );
    assert!(proxied(&["dashboard.show"], files).is_empty());
}

#[test]
fn the_pinned_proxy_configuration_is_read() {
    let read: Vec<String> = lemonfiber_bundled::PINNED
        .iter()
        .flat_map(|pinned| proxied(pinned.fills, pinned.files))
        .collect();
    assert!(read.iter().any(|label| label == "watch"), "{read:?}");
}

#[test]
fn a_further_port_a_shipped_service_publishes_is_held() {
    let further = 8921;
    let mut shipped = super::bundled::services().to_vec();
    if let Some(jellyfin) = shipped.iter_mut().find(|one| one.id == "jellyfin") {
        jellyfin.publish = vec![lemonfiber_manifest::container::Publish {
            port: further,
            listens: None,
        }];
    }
    let held = super::reserved(&[], &shipped);
    assert_eq!(
        on_port(&held, "komga", further).map(|one| one.id.as_str()),
        Some("jellyfin")
    );
    assert_eq!(on_port(&held, "komga", further + 1), None);
}

#[test]
fn a_label_only_the_proxy_configuration_answers_on_is_held_by_the_proxy_plugin() {
    let door = Answered {
        label: "watch".to_owned(),
        plugin: "door".to_owned(),
    };
    let joined = joined(
        vec![door.clone()],
        "caddy",
        vec!["status".to_owned(), "watch".to_owned()],
    );
    assert_eq!(
        joined,
        [
            door,
            Answered {
                label: "status".to_owned(),
                plugin: "caddy".to_owned(),
            }
        ]
    );
    assert_eq!(answering(&joined, "komga", "status"), Some("caddy"));
    assert_eq!(answering(&joined, "caddy", "status"), None);
}
