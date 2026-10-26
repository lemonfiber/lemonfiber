use std::collections::BTreeMap;

use super::{is_plain, UrlOf, UserMode, Value};
use super::{
    Constant, Container, Minted, Mount, Network, Posture, Publish, Secret, SecretOf, Setting, Shape,
};

const NONE: BTreeMap<String, Value> = BTreeMap::new();

static DECLARED: std::sync::LazyLock<[Secret; 1]> = std::sync::LazyLock::new(|| {
    [Secret {
        serves: None,
        id: "admin-key".to_owned(),
        of: "jellyfin".to_owned(),
        why: "Mint keys for the stack's services".to_owned(),
        minted: Minted::Not,
        carried_from: None,
    }]
});

fn plain(env: &BTreeMap<String, Value>) -> Container<'_> {
    Container {
        id: "jellyfin",
        namespace: "jellyfin",
        port: Some(8096),
        listens: None,
        publish: &[],
        takes_data: true,
        config_path: None,
        config_ro: false,
        mounts: &[],
        tmpfs: &[],
        env,
        secrets: &*DECLARED,
        user_mode: None,
        posture: Posture::Standard,
        mem_limit_mib: None,
        network: None,
        shape: None,
        provides: &[],
        speaks: false,
    }
}

fn wrong(container: &Container<'_>) -> Vec<String> {
    container
        .broken()
        .into_iter()
        .map(|(at, what)| format!("{at}: {what}"))
        .collect()
}

fn env(pairs: Vec<(&str, Value)>) -> BTreeMap<String, Value> {
    pairs
        .into_iter()
        .map(|(name, value)| (name.to_owned(), value))
        .collect()
}

#[test]
fn a_service_declaring_nothing_further_breaks_nothing() {
    let none = NONE;
    assert_eq!(wrong(&plain(&none)), Vec::<String>::new());
}

#[test]
fn further_ports_need_a_port_and_none_is_published_twice() {
    let none = NONE;
    let twice = [Publish {
        port: 8096,
        listens: None,
    }];
    let mut doubled = plain(&none);
    doubled.publish = &twice;
    assert_eq!(
        wrong(&doubled),
        ["service jellyfin.publish: publishes 8096 twice"]
    );
    let further = [Publish {
        port: 8920,
        listens: None,
    }];
    let mut portless = plain(&none);
    portless.port = None;
    portless.publish = &further;
    assert_eq!(wrong(&portless).len(), 1);
}

#[test]
fn a_mount_comes_from_inside_its_own_directory_and_lands_outside_the_data() {
    let none = NONE;
    let mounts = [
        Mount {
            from: "cache".to_owned(),
            to: "/cache".to_owned(),
            ro: false,
        },
        Mount {
            from: "../etc".to_owned(),
            to: "/etc/x".to_owned(),
            ro: true,
        },
        Mount {
            from: "/etc".to_owned(),
            to: "/data/x".to_owned(),
            ro: false,
        },
        Mount {
            from: "again".to_owned(),
            to: "/cache".to_owned(),
            ro: false,
        },
    ];
    let mut mounting = plain(&none);
    mounting.mounts = &mounts;
    let said = wrong(&mounting);
    assert_eq!(said.len(), 4, "{said:#?}");
    assert!(said
        .iter()
        .any(|line| line.contains("../etc is not a relative path")));
    assert!(said
        .iter()
        .any(|line| line.contains("/data/x is inside the data mount")));
    assert!(said
        .iter()
        .any(|line| line.contains("/cache is the target of another mount")));
}

#[test]
fn a_configuration_directory_is_never_the_data_and_never_the_root() {
    let none = NONE;
    for path in [
        "/data",
        "/data/config",
        "/",
        "config",
        "/a/../b",
        "/${HOME}",
    ] {
        let mut placed = plain(&none);
        placed.config_path = Some(path);
        assert_eq!(wrong(&placed).len(), 1, "{path}");
    }
    let mut placed = plain(&none);
    placed.config_path = Some("/app/config");
    assert!(wrong(&placed).is_empty());
}

#[test]
fn scratch_space_is_held_to_the_targets_rules() {
    let none = NONE;
    let tmpfs = ["/tmp".to_owned(), "tmp".to_owned(), "/config".to_owned()];
    let mut scratch = plain(&none);
    scratch.tmpfs = &tmpfs;
    let said = wrong(&scratch);
    assert_eq!(said.len(), 2, "{said:#?}");
    assert!(said.iter().any(
        |line| line == "service jellyfin.tmpfs /config: /config is the target of another mount"
    ));
}

#[test]
fn a_variable_is_named_plainly_and_never_one_lemonfiber_writes() {
    let named = env(vec![
        ("lower", Value::Constant(Constant::Flag(true))),
        ("TZ", Value::Constant(Constant::Text("UTC".to_owned()))),
        ("9LIVES", Value::Constant(Constant::Number(9))),
        ("_OK", Value::Constant(Constant::Number(1))),
    ]);
    let said = wrong(&plain(&named));
    assert_eq!(said.len(), 3, "{said:#?}");
}

#[test]
fn a_setting_is_published_or_the_services_own() {
    let setting = |name: &str| {
        Value::Setting(Setting {
            setting: name.to_owned(),
            default: Constant::Text(String::new()),
        })
    };
    let named = env(vec![
        ("A", setting("domain")),
        ("B", setting("jellyfin:cache")),
        ("C", setting("gluetun:countries")),
        ("D", setting("jellyfin:")),
    ]);
    let said = wrong(&plain(&named));
    assert_eq!(said.len(), 2, "{said:#?}");
    assert!(said.iter().any(|line| line.contains("gluetun:countries")));
}

#[test]
fn a_setting_named_as_a_declared_secret_is_refused() {
    let named = env(vec![(
        "A",
        Value::Setting(Setting {
            setting: "jellyfin:admin-key".to_owned(),
            default: Constant::Text(String::new()),
        }),
    )]);
    assert_eq!(
        wrong(&plain(&named)),
        ["service jellyfin.env.A: names setting jellyfin:admin-key, which a secret it declares is also named"]
    );
}

#[test]
fn an_address_names_a_core_capability_and_a_secret_one_declared() {
    let named = env(vec![
        (
            "URL",
            Value::Url(UrlOf {
                url_of: "library.curate".to_owned(),
                files: Some("tv".to_owned()),
                native: Some("servarr".to_owned()),
            }),
        ),
        (
            "BAD_URL",
            Value::Url(UrlOf {
                url_of: "sonarr".to_owned(),
                files: None,
                native: None,
            }),
        ),
        (
            "KEY",
            Value::Secret(SecretOf {
                secret: "admin-key".to_owned(),
            }),
        ),
        (
            "OTHER",
            Value::Secret(SecretOf {
                secret: "other".to_owned(),
            }),
        ),
    ]);
    let said = wrong(&plain(&named));
    assert_eq!(said.len(), 2, "{said:#?}");
}

#[test]
fn an_adapter_or_a_confined_service_runs_as_the_operator() {
    let none = NONE;
    let mut adapter = plain(&none);
    adapter.speaks = true;
    assert_eq!(adapter.user(), UserMode::Uid);
    adapter.user_mode = Some(UserMode::Image);
    assert_eq!(wrong(&adapter).len(), 1);
    let mut confined = plain(&none);
    confined.posture = Posture::Confined;
    confined.shape = Some(Shape::Render);
    assert_eq!(confined.user(), UserMode::Uid);
    assert_eq!(wrong(&confined).len(), 1);
    assert_eq!(plain(&none).user(), UserMode::Image);
}

#[test]
fn only_the_egress_guard_is_a_network_to_run_inside_and_never_ones_own() {
    let none = NONE;
    let guard = Network {
        via: "network.egress-guard".to_owned(),
    };
    let other = Network {
        via: "media.serve".to_owned(),
    };
    let provides = ["network.egress-guard".to_owned()];
    let mut inside = plain(&none);
    inside.network = Some(&guard);
    assert!(wrong(&inside).is_empty());
    inside.provides = &provides;
    assert_eq!(wrong(&inside).len(), 1);
    let mut elsewhere = plain(&none);
    elsewhere.network = Some(&other);
    assert_eq!(wrong(&elsewhere).len(), 1);
}

#[test]
fn a_shape_adds_its_own_and_is_approved_by_name() {
    assert_eq!(Shape::EgressGuard.name(), "egress-guard");
    assert_eq!(Shape::EgressGuard.grants(), ["NET_ADMIN"]);
    assert_eq!(Shape::EgressGuard.devices(), ["/dev/net/tun"]);
    assert_eq!(Shape::EgressGuard.serves(), "network.egress-guard");
    assert_eq!(Shape::Render.name(), "render");
    assert!(Shape::Render.grants().is_empty());
    assert_eq!(Shape::Render.devices(), ["/dev/dri"]);
    assert_eq!(Shape::Render.serves(), "media.serve");
    assert_eq!(Shape::Render.approval("jellyfin"), "render@jellyfin");
}

#[test]
fn a_value_is_read_as_the_kind_it_is_written_as() {
    let read: BTreeMap<String, Value> = toml::from_str(
        r#"
        A = "x"
        B = 3
        C = true
        D = { setting = "domain", default = "home.local" }
        E = { url_of = "indexer.search" }
        F = { secret = "key" }
        "#,
    )
    .unwrap_or_default();
    assert!(matches!(
        read.get("A"),
        Some(Value::Constant(Constant::Text(_)))
    ));
    assert!(matches!(
        read.get("B"),
        Some(Value::Constant(Constant::Number(3)))
    ));
    assert!(matches!(
        read.get("C"),
        Some(Value::Constant(Constant::Flag(true)))
    ));
    assert!(matches!(read.get("D"), Some(Value::Setting(_))));
    assert!(matches!(read.get("E"), Some(Value::Url(_))));
    assert!(matches!(read.get("F"), Some(Value::Secret(_))));
    let unknown: Result<BTreeMap<String, Value>, _> = toml::from_str(r#"G = { file = "/etc" }"#);
    assert!(unknown.is_err());
}

#[test]
fn a_memory_ceiling_is_more_than_nothing() {
    let none = NONE;
    let mut capped = plain(&none);
    capped.mem_limit_mib = Some(0);
    assert_eq!(wrong(&capped).len(), 1);
}

#[test]
fn a_value_holding_a_control_character_is_refused_and_a_dollar_is_not() {
    let named = env(vec![
        (
            "BROKEN",
            Value::Constant(Constant::Text("a\n  - /:/host".to_owned())),
        ),
        (
            "DEFAULT",
            Value::Setting(Setting {
                setting: "domain".to_owned(),
                default: Constant::Text("x\ry".to_owned()),
            }),
        ),
        (
            "FOREIGN",
            Value::Setting(Setting {
                setting: "gluetun:countries".to_owned(),
                default: Constant::Text("a\tb".to_owned()),
            }),
        ),
        ("PRICE", Value::Constant(Constant::Text("$5".to_owned()))),
    ]);
    let said = wrong(&plain(&named));
    assert_eq!(said.len(), 4, "{said:#?}");
    assert_eq!(
        said.iter()
            .filter(|line| line.starts_with("service jellyfin.env.FOREIGN:"))
            .count(),
        2,
        "{said:#?}"
    );
}

#[test]
fn a_plain_path_is_written_in_letters_digits_and_four_marks() {
    assert!(is_plain("/app/data-1/x_y.z"));
    for bad in ["/a b", "/a:b", "/a$b", "/a\nb", "/a{b"] {
        assert!(!is_plain(bad), "{bad:?}");
    }
}

#[test]
fn each_field_is_read_as_written_and_defaulted_where_left_out() {
    let publish: Result<Publish, _> = toml::from_str("port = 8920");
    assert_eq!(
        publish.ok(),
        Some(Publish {
            port: 8920,
            listens: None
        })
    );
    let mount: Result<Mount, _> = toml::from_str("from = \"cache\"\nto = \"/cache\"");
    assert_eq!(mount.ok().map(|read| read.ro), Some(false));
    let network: Result<Network, _> = toml::from_str("via = \"network.egress-guard\"");
    assert_eq!(
        network.ok().map(|read| read.via),
        Some("network.egress-guard".to_owned())
    );
    let modes: Result<BTreeMap<String, UserMode>, _> =
        toml::from_str("a = \"image\"\nb = \"puid-env\"\nc = \"uid\"");
    assert_eq!(
        modes
            .ok()
            .map(|read| read.into_values().collect::<Vec<_>>()),
        Some(vec![UserMode::Image, UserMode::PuidEnv, UserMode::Uid])
    );
    let postures: Result<BTreeMap<String, Posture>, _> =
        toml::from_str("a = \"standard\"\nb = \"confined\"");
    assert_eq!(
        postures
            .ok()
            .map(|read| read.into_values().collect::<Vec<_>>()),
        Some(vec![Posture::Standard, Posture::Confined])
    );
    assert_eq!(Posture::default(), Posture::Standard);
}

#[test]
fn no_mount_source_lies_inside_another() {
    let none = NONE;
    let mounts = [
        Mount {
            from: "cache".to_owned(),
            to: "/cache".to_owned(),
            ro: false,
        },
        Mount {
            from: "cache/deep".to_owned(),
            to: "/deep".to_owned(),
            ro: false,
        },
        Mount {
            from: "cachet".to_owned(),
            to: "/cachet".to_owned(),
            ro: false,
        },
    ];
    let mut nested = plain(&none);
    nested.mounts = &mounts;
    let said = wrong(&nested);
    assert_eq!(
        said,
        ["service jellyfin.mount /deep: cache/deep lies inside cache, another of its sources"]
    );
}

#[test]
fn an_address_files_a_medium_of_the_vocabulary_and_names_its_api_in_one_word() {
    let url = |files: Option<&str>, native: Option<&str>| {
        Value::Url(UrlOf {
            url_of: "library.curate".to_owned(),
            files: files.map(str::to_owned),
            native: native.map(str::to_owned),
        })
    };
    let named = env(vec![
        ("GOOD", url(Some("comics"), Some("servarr2"))),
        ("MEDIUM", url(Some("anime"), None)),
        ("API", url(None, Some("Servarr"))),
        ("BOTH", url(Some("tv "), Some("2arr"))),
    ]);
    let said = wrong(&plain(&named));
    assert_eq!(said.len(), 3, "{said:#?}");
    assert!(said
        .iter()
        .any(|one| one.starts_with("service jellyfin.env.MEDIUM") && one.contains("files anime")));
    assert!(said
        .iter()
        .any(|one| one.starts_with("service jellyfin.env.API") && one.contains("native Servarr")));
    assert!(said
        .iter()
        .any(|one| one.starts_with("service jellyfin.env.BOTH")
            && one.contains("files tv ")
            && one.contains("native 2arr")));
}

#[test]
fn a_port_two_services_publish_names_both() {
    let further = [Publish {
        port: 8920,
        listens: Some(8920),
    }];
    let services = [
        (
            "jellyfin",
            super::published(Some(8096), &further).collect::<Vec<_>>(),
        ),
        ("emby", vec![8097, 8920]),
        ("plex", vec![8096]),
        ("same", vec![9000, 9000]),
    ];
    assert_eq!(
        super::doubled(
            services
                .iter()
                .map(|(id, ports)| (*id, ports.iter().copied()))
        ),
        [(8920, "jellyfin", "emby"), (8096, "jellyfin", "plex")]
    );
}

#[test]
fn a_minted_secret_is_written_back_as_the_manifest_said_it() {
    for (minted, written) in [
        (Minted::Not, None),
        (Minted::Key, Some("minted = true")),
        (Minted::Password, Some("minted = \"password\"")),
    ] {
        let declared = Secret {
            serves: None,
            id: "admin-password".to_owned(),
            of: "jellyfin".to_owned(),
            why: "Its administrator's password".to_owned(),
            minted,
            carried_from: None,
        };
        let text = toml::to_string(&declared).unwrap_or_default();
        assert_eq!(
            text.lines().find(|line| line.starts_with("minted")),
            written
        );
        assert_eq!(toml::from_str::<Secret>(&text).ok(), Some(declared));
    }
}

#[test]
fn a_minted_shape_reads_back_as_it_is_written_false_included() {
    for (minted, written) in [
        (Minted::Not, serde_json::json!(false)),
        (Minted::Key, serde_json::json!(true)),
        (Minted::Password, serde_json::json!("password")),
    ] {
        assert_eq!(serde_json::to_value(minted).ok(), Some(written.clone()));
        assert_eq!(serde_json::from_value::<Minted>(written).ok(), Some(minted));
    }
}
