use std::collections::BTreeMap;

use lemonfiber_manifest::container::{
    Constant, Fields, Mount, Network, Posture, Publish, SecretOf, Setting, Shape, UrlOf, UserMode,
    Value,
};

use super::super::super::installed::{Installed, Placed};
use super::super::{variables, Holding, Reading as Secrets, Variable};
use super::{document, installed, placed, written};
use crate::project::Reading;

fn with(fields: Fields) -> Placed {
    Placed { fields, ..placed() }
}

fn entry_of(text: &str) -> serde_norway::Value {
    serde_norway::from_str::<serde_norway::Value>(text)
        .ok()
        .and_then(|document| document.get("services")?.get("komga").cloned())
        .unwrap_or_default()
}

fn scalar(value: &serde_norway::Value) -> Option<String> {
    value
        .as_str()
        .map(str::to_owned)
        .or_else(|| value.as_u64().map(|number| number.to_string()))
        .or_else(|| value.as_bool().map(|flag| flag.to_string()))
}

fn strings(entry: &serde_norway::Value, key: &str) -> Vec<String> {
    entry
        .get(key)
        .and_then(serde_norway::Value::as_sequence)
        .map(|items| items.iter().filter_map(scalar).collect())
        .unwrap_or_default()
}

fn text(entry: &serde_norway::Value, path: &[&str]) -> Option<String> {
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
        ["0.0.0.0:25600:8080", "0.0.0.0:8920:8920"]
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
            "./data:/data",
            "./config/komga:/config:ro",
            "./mounts/komga/cache:/cache",
            "./mounts/komga/Caddyfile:/etc/caddy/Caddyfile:ro",
        ]
    );
    assert_eq!(strings(&entry, "tmpfs"), ["/tmp"]);
}

#[test]
fn an_adapter_shares_the_whole_configuration_directory_of_the_service_it_fronts() {
    let sharing = |fronts: Option<&str>, at: &str| {
        let mut placed = with(Fields {
            shares_config: Some(at.to_owned()),
            ..Fields::default()
        });
        placed.fronts = fronts.map(str::to_owned);
        strings(&entry_of(&document(placed)), "volumes")
    };
    assert_eq!(
        sharing(Some("komga-server"), "/upstream"),
        [
            "./data:/data",
            "./config/komga:/config",
            "./config/komga-server:/upstream",
        ]
    );
    assert_eq!(
        sharing(None, "/upstream"),
        ["./data:/data", "./config/komga:/config"]
    );
    assert_eq!(
        sharing(Some("komga-server"), "/data/upstream"),
        ["./data:/data", "./config/komga:/config"]
    );
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
        ("ON".to_owned(), Value::Constant(Constant::Flag(true))),
        (
            "DOMAIN".to_owned(),
            Value::Setting(Setting {
                setting: "domain".to_owned(),
                default: Constant::Text("home.local".to_owned()),
                recorded_with: None,
                was: None,
                needs: lemonfiber_manifest::container::choices::Needs::new(),
                one_of: None,
            }),
        ),
        (
            "SET".to_owned(),
            Value::Setting(Setting {
                setting: "komga:mode".to_owned(),
                default: Constant::Flag(false),
                recorded_with: None,
                was: None,
                needs: lemonfiber_manifest::container::choices::Needs::new(),
                one_of: None,
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
    let setting = |_: &str, name: &str| (name == "komga:mode").then(|| "fast".to_owned());
    let reading = Reading {
        setting: &setting,
        ..Reading::bare()
    };
    let asking = Placed {
        asks: vec![crate::plugin::Asking {
            native: None,
            capability: "indexer.search".to_owned(),
            each: false,
        }],
        ..with(Fields {
            env,
            ..Fields::default()
        })
    };
    let document = written(&installed(vec![asking]), &reading);
    let entry = entry_of(&document);
    let environment = |name: &str| text(&entry, &["environment", name]);
    assert_eq!(environment("PRICE").as_deref(), Some("$$5"));
    assert_eq!(environment("COUNT").as_deref(), Some("3"));
    assert_eq!(environment("ON").as_deref(), Some("true"));
    assert_eq!(environment("DOMAIN").as_deref(), Some("home.local"));
    assert_eq!(environment("SET").as_deref(), Some("fast"));
    assert_eq!(
        environment("INDEXER").as_deref(),
        Some("http://prowlarr:9696")
    );
    assert_eq!(environment("UNFILLED"), None);
    assert_eq!(environment("KEY"), None);
    assert_eq!(environment("TZ").as_deref(), Some("Europe/Amsterdam"));
}

#[test]
fn an_image_that_drops_to_the_operator_under_its_own_names_is_told_them() {
    let entry = entry_of(&document(with(Fields {
        user_mode: Some(UserMode::IdsIn(
            lemonfiber_manifest::container::IdVariables {
                uid_env: "PLEX_UID".to_owned(),
                gid_env: "PLEX_GID".to_owned(),
            },
        )),
        ..Fields::default()
    })));
    assert_eq!(
        text(&entry, &["environment", "PLEX_UID"]).as_deref(),
        Some("1000")
    );
    assert_eq!(
        text(&entry, &["environment", "PLEX_GID"]).as_deref(),
        Some("1000")
    );
    assert_eq!(text(&entry, &["environment", "PUID"]), None);
}

#[test]
fn an_image_that_drops_to_the_operators_pair_is_told_it() {
    let entry = entry_of(&document(with(Fields {
        user_mode: Some(UserMode::IdsIn(
            lemonfiber_manifest::container::IdVariables::puid(),
        )),
        ..Fields::default()
    })));
    assert_eq!(
        text(&entry, &["environment", "PUID"]).as_deref(),
        Some("1000")
    );
    assert_eq!(
        text(&entry, &["environment", "PGID"]).as_deref(),
        Some("1000")
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
    assert_eq!(text(&entry, &["user"]).as_deref(), Some("1000:1000"));
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
            via: Some("network.egress-guard".to_owned()),
            fenced: false,
        }),
        ..Fields::default()
    });
    let entry = entry_of(&document(inside));
    assert_eq!(
        text(&entry, &["network_mode"]).as_deref(),
        Some("service:gluetun")
    );
    assert_eq!(
        text(&entry, &["depends_on", "gluetun", "condition"]).as_deref(),
        Some("service_healthy")
    );
    assert!(strings(&entry, "ports").is_empty());
}

#[test]
fn a_record_whose_paths_the_model_refuses_writes_none_of_them() {
    let mount = |from: &str, to: &str| Mount {
        from: from.to_owned(),
        to: to.to_owned(),
        ro: false,
    };
    let entry = entry_of(&document(Placed {
        config_path: "/data/config".to_owned(),
        ..with(Fields {
            mount: vec![
                mount("../../etc", "/etc-of-host"),
                mount("/etc", "/absolute"),
                mount("library", "/data/x"),
                mount("a:b", "/colon"),
                mount("ok", "/c:ro"),
                mount("outer", "/outer"),
                mount("outer/inner", "/inner"),
            ],
            tmpfs: vec!["/data/scratch".to_owned(), "/tmp:exec".to_owned()],
            ..Fields::default()
        })
    }));
    assert_eq!(
        strings(&entry, "volumes"),
        [
            "./data:/data",
            "./config/komga:/config",
            "./mounts/komga/outer:/outer",
        ]
    );
    assert!(strings(&entry, "tmpfs").is_empty());
}

#[test]
fn a_record_naming_lemonfibers_own_variables_does_not_override_them() {
    let entry = entry_of(&document(with(Fields {
        user_mode: Some(UserMode::IdsIn(
            lemonfiber_manifest::container::IdVariables::puid(),
        )),
        env: ["TZ", "PUID", "PGID"]
            .map(|name| {
                (
                    name.to_owned(),
                    Value::Constant(Constant::Text("0".to_owned())),
                )
            })
            .into(),
        ..Fields::default()
    })));
    assert_eq!(
        text(&entry, &["environment", "TZ"]).as_deref(),
        Some("Europe/Amsterdam")
    );
    assert_eq!(
        text(&entry, &["environment", "PUID"]).as_deref(),
        Some("1000")
    );
    assert_eq!(
        text(&entry, &["environment", "PGID"]).as_deref(),
        Some("1000")
    );
}

const SECRETS_ENTRY: &str = "services:\n  \
         komga:\n    \
         image: ghcr.io/gotson/komga@sha256:\
         4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945\n    \
         profiles:\n    \
         - plugin-komga\n    \
         restart: unless-stopped\n    \
         environment:\n      \
         MODE: fast$$1\n      \
         TZ: Europe/Amsterdam\n    \
         env_file:\n    \
         - /home/op/.config/lemonfiber/env/komga.env\n    \
         ports:\n    \
         - 0.0.0.0:25600:25600\n    \
         volumes:\n    \
         - ./data:/data\n    \
         - ./config/komga:/config\n    \
         networks:\n    \
         - ask__komga__proxy.front__caddy\n    \
         - own__komga\n    \
         security_opt:\n    \
         - no-new-privileges:true\n    \
         cap_drop:\n    \
         - ALL\n    \
         cap_add:\n    \
         - CHOWN\n    \
         - DAC_OVERRIDE\n    \
         - FOWNER\n    \
         - SETGID\n    \
         - SETUID\n    \
         - KILL\n    \
         pids_limit: 1024\n";

fn naming_a_secret() -> Placed {
    with(Fields {
        env: BTreeMap::from([
            (
                "MODE".to_owned(),
                Value::Setting(Setting {
                    setting: "komga:mode".to_owned(),
                    default: Constant::Text("slow".to_owned()),
                    recorded_with: None,
                    was: None,
                    needs: lemonfiber_manifest::container::choices::Needs::new(),
                    one_of: None,
                }),
            ),
            (
                "KEY".to_owned(),
                Value::Secret(SecretOf {
                    secret: "key".to_owned(),
                }),
            ),
        ]),
        ..Fields::default()
    })
}

#[test]
fn a_service_naming_a_secret_reads_its_own_file_and_a_recorded_setting_is_written() {
    let setting = |_: &str, name: &str| (name == "komga:mode").then(|| "fast$1".to_owned());
    let file = |service: &str| {
        Some(std::path::PathBuf::from(format!(
            "/home/op/.config/lemonfiber/env/{service}.env"
        )))
    };
    let reading = Reading {
        setting: &setting,
        secrets_file: &file,
        ..Reading::bare()
    };
    assert_eq!(
        written(&installed(vec![naming_a_secret()]), &reading),
        SECRETS_ENTRY
    );
}

#[test]
fn a_service_naming_no_secret_has_no_file_and_one_naming_a_secret_has_it_held_or_not() {
    let file = |_: &str| Some(std::path::PathBuf::from("/x/komga.env"));
    let reading = Reading {
        secrets_file: &file,
        ..Reading::bare()
    };
    let plain = entry_of(&written(&installed(vec![placed()]), &reading));
    assert!(strings(&plain, "env_file").is_empty());
    let naming = entry_of(&written(&installed(vec![naming_a_secret()]), &reading));
    assert_eq!(strings(&naming, "env_file"), ["/x/komga.env"]);
    let nowhere = entry_of(&document(naming_a_secret()));
    assert!(strings(&nowhere, "env_file").is_empty());
}

#[test]
fn a_secret_is_read_from_the_store_unless_its_holder_lets_it_cross() {
    let one = installed(vec![naming_a_secret()]);
    let declared = Installed {
        declared: crate::plugin::Declaration {
            secrets: vec![crate::plugin::Secret {
                serves: None,
                id: "key".to_owned(),
                of: "sonarr".to_owned(),
                why: "reads the curator".to_owned(),
                minted: false,
                was: None,
                needs: lemonfiber_manifest::container::choices::Needs::new(),
                needed_to_start: false,
                lapses_s: None,
            }],
            ..crate::plugin::Declaration::default()
        },
        ..one.clone()
    };
    let crossing = |plugin: &str, of: &str| {
        (plugin == "komga" && of == "sonarr")
            .then(|| Holding::Crossing("SONARR_API_KEY".to_owned()))
    };
    let reading = Secrets {
        crossing: &crossing,
        ..Secrets::bare()
    };
    let placed = naming_a_secret();
    assert_eq!(
        variables(&one, &placed, &reading),
        [Variable {
            name: "KEY".to_owned(),
            holding: Holding::Store("komga:key".to_owned()),
        }]
    );
    assert_eq!(
        variables(&declared, &placed, &reading),
        [Variable {
            name: "KEY".to_owned(),
            holding: Holding::Crossing("SONARR_API_KEY".to_owned()),
        }]
    );
}

#[test]
fn a_crossing_secret_whose_holder_nothing_here_names_is_withheld_and_an_own_one_is_stored() {
    let placed = naming_a_secret();
    let declaring = |of: &str| {
        let one = installed(vec![naming_a_secret()]);
        Installed {
            declared: crate::plugin::Declaration {
                secrets: vec![crate::plugin::Secret {
                    serves: None,
                    id: "key".to_owned(),
                    of: of.to_owned(),
                    why: "reads it".to_owned(),
                    minted: false,
                    was: None,
                    needs: lemonfiber_manifest::container::choices::Needs::new(),
                    needed_to_start: false,
                    lapses_s: None,
                }],
                ..crate::plugin::Declaration::default()
            },
            ..one
        }
    };
    let reading = Secrets::bare();
    let held = |of: &str| {
        variables(&declaring(of), &placed, &reading)
            .into_iter()
            .map(|variable| variable.holding)
            .collect::<Vec<_>>()
    };

    assert_eq!(held("sonarr"), [Holding::Withheld]);
    assert_eq!(
        held(&placed.service),
        [Holding::Store("komga:key".to_owned())]
    );
    assert_eq!(held("komga"), [Holding::Store("komga:key".to_owned())]);
}
