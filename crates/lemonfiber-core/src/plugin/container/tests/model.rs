use std::collections::BTreeMap;

use lemonfiber_manifest::container::{
    Constant, Fields, Mount, Network, Posture, Publish, SecretOf, Setting, Shape, UrlOf, UserMode,
    Value,
};

use super::super::super::installed::Placed;
use super::super::{written, Reading};
use super::{document, installed, placed};

fn with(fields: Fields) -> Placed {
    Placed { fields, ..placed() }
}

fn entry_of(text: &str) -> serde_yaml_ng::Value {
    serde_yaml_ng::from_str::<serde_yaml_ng::Value>(text)
        .ok()
        .and_then(|document| document.get("services")?.get("komga").cloned())
        .unwrap_or_default()
}

fn scalar(value: &serde_yaml_ng::Value) -> Option<String> {
    value
        .as_str()
        .map(str::to_owned)
        .or_else(|| value.as_u64().map(|number| number.to_string()))
        .or_else(|| value.as_bool().map(|flag| flag.to_string()))
}

fn strings(entry: &serde_yaml_ng::Value, key: &str) -> Vec<String> {
    entry
        .get(key)
        .and_then(serde_yaml_ng::Value::as_sequence)
        .map(|items| items.iter().filter_map(scalar).collect())
        .unwrap_or_default()
}

fn text(entry: &serde_yaml_ng::Value, path: &[&str]) -> Option<String> {
    path.iter()
        .try_fold(entry, |at, key| at.get(*key))
        .and_then(scalar)
}

#[test]
fn further_ports_are_published_on_the_tier_to_where_the_container_listens() {
    let entry = entry_of(&document(Placed {
        listens: Some(8080),
        ..with(Fields {
            publish: vec![Publish {
                port: 8920,
                listens: None,
            }],
            ..Fields::default()
        })
    }));
    assert_eq!(
        strings(&entry, "ports"),
        [
            "${LAN_BIND:-0.0.0.0}:25600:8080",
            "${LAN_BIND:-0.0.0.0}:8920:8920"
        ]
    );
}

#[test]
fn further_mounts_come_from_the_mount_tree_and_the_configuration_may_be_read_only() {
    let entry = entry_of(&document(with(Fields {
        config_ro: true,
        mount: vec![
            Mount {
                from: "cache".to_owned(),
                to: "/cache".to_owned(),
                ro: false,
            },
            Mount {
                from: "Caddyfile".to_owned(),
                to: "/etc/caddy/Caddyfile".to_owned(),
                ro: true,
            },
        ],
        tmpfs: vec!["/tmp".to_owned()],
        ..Fields::default()
    })));
    assert_eq!(
        strings(&entry, "volumes"),
        [
            "${DATA_ROOT:-./data}:/data",
            "./config/komga:/config:ro",
            "./mounts/komga/cache:/cache",
            "./mounts/komga/Caddyfile:/etc/caddy/Caddyfile:ro",
        ]
    );
    assert_eq!(strings(&entry, "tmpfs"), ["/tmp"]);
}

#[test]
fn each_variable_is_written_as_its_kind_and_none_is_substituted() {
    let url = |capability: &str| {
        Value::Url(UrlOf {
            url_of: capability.to_owned(),
            files: None,
            native: None,
        })
    };
    let env = BTreeMap::from([
        (
            "PRICE".to_owned(),
            Value::Constant(Constant::Text("$5".to_owned())),
        ),
        ("COUNT".to_owned(), Value::Constant(Constant::Number(3))),
        (
            "DOMAIN".to_owned(),
            Value::Setting(Setting {
                setting: "domain".to_owned(),
                default: Constant::Text("home.local".to_owned()),
            }),
        ),
        (
            "SET".to_owned(),
            Value::Setting(Setting {
                setting: "komga:mode".to_owned(),
                default: Constant::Flag(false),
            }),
        ),
        ("INDEXER".to_owned(), url("indexer.search")),
        ("UNFILLED".to_owned(), url("subtitles.fetch")),
        (
            "KEY".to_owned(),
            Value::Secret(SecretOf {
                secret: "key".to_owned(),
            }),
        ),
    ]);
    let setting = |name: &str| (name == "komga:mode").then(|| "fast".to_owned());
    let address = |asked: &UrlOf| {
        (asked.url_of == "indexer.search").then(|| "http://prowlarr:9696".to_owned())
    };
    let reading = Reading {
        setting: &setting,
        address: &address,
        ..Reading::bare()
    };
    let document = written(
        &installed(vec![with(Fields {
            env,
            ..Fields::default()
        })]),
        &reading,
    );
    let entry = entry_of(&document);
    let environment = |name: &str| text(&entry, &["environment", name]);
    assert_eq!(environment("PRICE").as_deref(), Some("$$5"));
    assert_eq!(environment("COUNT").as_deref(), Some("3"));
    assert_eq!(environment("DOMAIN").as_deref(), Some("home.local"));
    assert_eq!(environment("SET").as_deref(), Some("fast"));
    assert_eq!(
        environment("INDEXER").as_deref(),
        Some("http://prowlarr:9696")
    );
    assert_eq!(environment("UNFILLED"), None);
    assert_eq!(environment("KEY"), None);
    assert_eq!(
        environment("TZ").as_deref(),
        Some("${TZ:-Europe/Amsterdam}")
    );
}

#[test]
fn an_image_that_drops_to_the_operators_pair_is_told_it() {
    let entry = entry_of(&document(with(Fields {
        user_mode: Some(UserMode::PuidEnv),
        ..Fields::default()
    })));
    assert_eq!(
        text(&entry, &["environment", "PUID"]).as_deref(),
        Some("${PUID:-1000}")
    );
    assert_eq!(
        text(&entry, &["environment", "PGID"]).as_deref(),
        Some("${PGID:-1000}")
    );
    assert_eq!(text(&entry, &["user"]), None);
}

#[test]
fn a_confined_service_is_read_only_holds_no_capability_and_runs_as_the_operator() {
    let entry = entry_of(&document(with(Fields {
        posture: Posture::Confined,
        mem_limit_mib: Some(64),
        ..Fields::default()
    })));
    assert_eq!(text(&entry, &["read_only"]).as_deref(), Some("true"));
    assert!(strings(&entry, "cap_add").is_empty());
    assert_eq!(strings(&entry, "cap_drop"), ["ALL"]);
    assert_eq!(
        text(&entry, &["user"]).as_deref(),
        Some("${PUID:-1000}:${PGID:-1000}")
    );
    assert_eq!(text(&entry, &["mem_limit"]).as_deref(), Some("64m"));
}

#[test]
fn render_adds_the_devices_and_their_group_only_where_the_host_has_them() {
    let shaped = Placed {
        shape: Some(Shape::Render),
        ..placed()
    };
    let with_gpu = Reading {
        render_group: Some(44),
        ..Reading::bare()
    };
    let gpu = entry_of(&written(&installed(vec![shaped.clone()]), &with_gpu));
    assert_eq!(strings(&gpu, "devices"), ["/dev/dri:/dev/dri"]);
    assert_eq!(strings(&gpu, "group_add"), ["44"]);
    let none = entry_of(&document(shaped));
    assert!(strings(&none, "devices").is_empty());
    assert!(strings(&none, "group_add").is_empty());
}

#[test]
fn a_service_inside_its_guards_network_has_none_of_its_own_and_waits_for_it() {
    let inside = with(Fields {
        network: Some(Network {
            via: "network.egress-guard".to_owned(),
        }),
        ..Fields::default()
    });
    let guard =
        |capability: &str| (capability == "network.egress-guard").then(|| "gluetun".to_owned());
    let guarded = Reading {
        guard: &guard,
        ..Reading::bare()
    };
    let entry = entry_of(&written(&installed(vec![inside.clone()]), &guarded));
    assert_eq!(
        text(&entry, &["network_mode"]).as_deref(),
        Some("service:gluetun")
    );
    assert_eq!(
        text(&entry, &["depends_on", "gluetun", "condition"]).as_deref(),
        Some("service_healthy")
    );
    assert!(strings(&entry, "ports").is_empty());
    let unguarded = entry_of(&document(inside));
    assert_eq!(text(&unguarded, &["network_mode"]).as_deref(), Some("none"));
    assert!(strings(&unguarded, "ports").is_empty());
}
