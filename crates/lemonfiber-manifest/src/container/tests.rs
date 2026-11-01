use std::collections::BTreeMap;

use super::{is_plain, IdVariables, UrlOf, UserMode, Value};
use super::{
    Constant, Container, Minted, Mount, Network, Posture, Publish, RecordedWith, Secret, SecretOf,
    Setting, Shape,
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
        lapses_s: None,
        recorded_with: None,
        was: None,
        needs: crate::container::choices::Needs::new(),
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
        shares_config: None,
        rebuilt: &[],
        fronts: None,
        tmpfs: &[],
        env,
        secrets: &*DECLARED,
        user_mode: None,
        posture: Posture::Standard,
        mem_limit_mib: None,
        network: None,
        shape: None,
        certificate: None,
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
            recorded_with: None,
            was: None,
            needs: crate::container::choices::Needs::new(),
            one_of: None,
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
            recorded_with: None,
            was: None,
            needs: crate::container::choices::Needs::new(),
            one_of: None,
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
    let image = UserMode::Image;
    adapter.user_mode = Some(&image);
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
        via: Some("network.egress-guard".to_owned()),
        fenced: false,
    };
    let other = Network {
        via: Some("media.serve".to_owned()),
        fenced: false,
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

/// A service may run inside a fence, or inside its filler's network, but not both and not
/// neither: each is a network namespace of its own.
#[test]
fn a_network_is_fenced_or_via_and_never_both_or_neither() {
    let none = NONE;
    let fenced = Network {
        via: None,
        fenced: true,
    };
    let both = Network {
        via: Some("network.egress-guard".to_owned()),
        fenced: true,
    };
    let neither = Network::default();
    let mut behind = plain(&none);
    behind.network = Some(&fenced);
    assert!(wrong(&behind).is_empty());
    behind.network = Some(&both);
    assert!(wrong(&behind)
        .iter()
        .any(|said| said.contains("network: says both via and fenced")));
    behind.network = Some(&neither);
    assert!(wrong(&behind)
        .iter()
        .any(|said| said.contains("network: says neither via nor fenced")));
    let read: Result<Network, _> = toml::from_str("fenced = true");
    assert_eq!(read.ok(), Some(fenced));
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
                recorded_with: None,
                was: None,
                needs: crate::container::choices::Needs::new(),
                one_of: None,
            }),
        ),
        (
            "FOREIGN",
            Value::Setting(Setting {
                setting: "gluetun:countries".to_owned(),
                default: Constant::Text("a\tb".to_owned()),
                recorded_with: None,
                was: None,
                needs: crate::container::choices::Needs::new(),
                one_of: None,
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
        Some(Some("network.egress-guard".to_owned()))
    );
    let modes: Result<BTreeMap<String, UserMode>, _> = toml::from_str(
        "a = \"image\"\nb = \"puid-env\"\nc = \"uid\"\nd = { uid_env = \"PLEX_UID\", gid_env = \"PLEX_GID\" }",
    );
    assert_eq!(
        modes
            .ok()
            .map(|read| read.into_values().collect::<Vec<_>>()),
        Some(vec![
            UserMode::Image,
            UserMode::IdsIn(IdVariables::puid()),
            UserMode::Uid,
            UserMode::IdsIn(IdVariables {
                uid_env: "PLEX_UID".to_owned(),
                gid_env: "PLEX_GID".to_owned(),
            }),
        ])
    );
    let unknown: Result<UserMode, _> =
        toml::from_str("uid_env = \"A\"\ngid_env = \"B\"\nhome = \"C\"");
    assert!(unknown.is_err());
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
        ("LISTENED", url(Some("audiobooks"), None)),
        ("SUBSCRIBED", url(Some("podcasts"), None)),
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
        (Minted::Email, Some("minted = \"email\"")),
    ] {
        let declared = Secret {
            serves: None,
            id: "admin-password".to_owned(),
            of: "jellyfin".to_owned(),
            why: "Its administrator's password".to_owned(),
            minted,
            carried_from: None,
            lapses_s: None,
            recorded_with: None,
            was: None,
            needs: crate::container::choices::Needs::new(),
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
        (Minted::Email, serde_json::json!("email")),
    ] {
        assert_eq!(serde_json::to_value(minted).ok(), Some(written.clone()));
        assert_eq!(serde_json::from_value::<Minted>(written).ok(), Some(minted));
    }
}

#[test]
fn a_stand_in_reads_minted_as_one_made_for_the_run_and_anything_else_as_a_constant() {
    let read = |text: &str| toml::from_str::<BTreeMap<String, Value>>(text).ok();
    let setting = |recorded_with: Option<RecordedWith>| {
        Value::Setting(Setting {
            setting: "gluetun:endpoint".to_owned(),
            default: Constant::Text(String::new()),
            recorded_with,
            was: None,
            needs: crate::container::choices::Needs::new(),
            one_of: None,
        })
    };
    let declared = read(
        "MADE = { setting = \"gluetun:endpoint\", default = \"\", recorded_with = \"minted\" }\n\
         ADDRESS = { setting = \"gluetun:endpoint\", default = \"\", recorded_with = \"192.0.2.1\" }\n\
         PORT = { setting = \"gluetun:endpoint\", default = \"\", recorded_with = 51820 }\n\
         NONE = { setting = \"gluetun:endpoint\", default = \"\" }",
    );
    assert_eq!(
        declared,
        Some(env(vec![
            ("MADE", setting(Some(RecordedWith::Minted))),
            (
                "ADDRESS",
                setting(Some(RecordedWith::Constant(Constant::Text(
                    "192.0.2.1".to_owned()
                ))))
            ),
            (
                "PORT",
                setting(Some(RecordedWith::Constant(Constant::Number(51820))))
            ),
            ("NONE", setting(None)),
        ]))
    );
    let written = declared
        .as_ref()
        .and_then(|declared| toml::to_string(declared).ok())
        .unwrap_or_default();
    assert_eq!(read(&written), declared, "{written}");
    assert!(written.contains("recorded_with = \"minted\""), "{written}");
    assert!(
        !written.contains("NONE = { default = \"\", recorded_with"),
        "{written}"
    );
}

#[test]
fn a_stand_in_holding_a_control_character_is_refused_and_one_made_for_the_run_is_not() {
    let stood_in = |recorded_with: RecordedWith| {
        Value::Setting(Setting {
            setting: "domain".to_owned(),
            default: Constant::Text(String::new()),
            recorded_with: Some(recorded_with),
            was: None,
            needs: crate::container::choices::Needs::new(),
            one_of: None,
        })
    };
    let named = env(vec![
        (
            "BROKEN",
            stood_in(RecordedWith::Constant(Constant::Text("a\nb".to_owned()))),
        ),
        ("MADE", stood_in(RecordedWith::Minted)),
    ]);
    let said = wrong(&plain(&named));
    assert_eq!(
        said,
        vec!["service jellyfin.env.BROKEN: \"a\\nb\" holds a control character".to_owned()]
    );
}

#[test]
fn an_adapter_shares_its_upstreams_configuration_only_where_it_fronts_one() {
    let none = NONE;
    let mut sharing = plain(&none);
    sharing.shares_config = Some("/upstream");
    assert_eq!(
        wrong(&sharing),
        [
            "service jellyfin.shares_config: shares the configuration of the service it fronts, \
             and fronts none"
        ]
    );
    sharing.fronts = Some("jellyfin-upstream");
    assert_eq!(wrong(&sharing), Vec::<String>::new());
    let plain_absolute = "is not one plain absolute path: not the root, with no \"..\", in \
                          letters, digits and \"._/-\" alone";
    for (to, said) in [
        (
            "/config",
            "/config is the target of another mount".to_owned(),
        ),
        ("/data/x", "/data/x is inside the data mount".to_owned()),
        ("upstream", format!("upstream {plain_absolute}")),
    ] {
        sharing.shares_config = Some(to);
        assert_eq!(
            wrong(&sharing),
            [format!("service jellyfin.shares_config: {said}")]
        );
    }
}

#[test]
fn what_a_service_rebuilds_lies_beneath_its_configuration_directory() {
    let none = NONE;
    let paths = [
        "cache".to_owned(),
        "../etc".to_owned(),
        "/var/log".to_owned(),
        "${LOGS}".to_owned(),
    ];
    let mut rebuilding = plain(&none);
    rebuilding.rebuilt = &paths;
    let found = wrong(&rebuilding);
    assert_eq!(found.len(), 3, "{found:?}");
    assert!(found
        .iter()
        .all(|one| one.ends_with("is not a relative path beneath its configuration directory")));
    assert!(!found.iter().any(|one| one.contains("rebuilt cache:")));
}

#[test]
fn a_certificate_is_held_to_its_rules_under_the_field_it_is_declared_in() {
    let none = NONE;
    let certificate = super::Certificate {
        key: "../key.pem".to_owned(),
        certificate: "certificate.pem".to_owned(),
        names: vec!["jellyfin".to_owned()],
    };
    let mut serving = plain(&none);
    serving.certificate = Some(&certificate);
    assert_eq!(
        serving.broken(),
        vec![(
            "service jellyfin.certificate".to_owned(),
            "../key.pem is not a relative path beneath its configuration directory".to_owned()
        )]
    );
}

#[test]
fn a_secret_a_recording_stands_in_for_is_needed_to_start_unless_it_is_a_code_that_lapses() {
    let [declared] = &*DECLARED;
    let secret = |recorded_with: Option<RecordedWith>, lapses_s: Option<u32>| Secret {
        recorded_with,
        lapses_s,
        ..declared.clone()
    };
    assert!(!secret(None, None).needed_to_start());
    assert!(secret(Some(RecordedWith::Minted), None).needed_to_start());
    assert!(!secret(Some(RecordedWith::Minted), Some(240)).needed_to_start());
}

#[test]
fn puid_env_is_the_pair_of_named_variables_and_is_written_back_by_its_name() {
    let named: Result<BTreeMap<String, UserMode>, _> = toml::from_str(
        "a = { uid_env = \"PUID\", gid_env = \"PGID\" }\nb = { uid_env = \"PLEX_UID\", gid_env = \"PLEX_GID\" }",
    );
    let written = named.ok().and_then(|read| toml::to_string(&read).ok());
    assert_eq!(
        written.as_deref(),
        Some("a = \"puid-env\"\n\n[b]\nuid_env = \"PLEX_UID\"\ngid_env = \"PLEX_GID\"\n")
    );
}

#[test]
fn named_id_variables_are_two_different_variables_lemonfiber_alone_writes() {
    let none = NONE;
    let declared = BTreeMap::from([(
        "PLEX_UID".to_owned(),
        Value::Secret(SecretOf {
            secret: "admin-key".to_owned(),
        }),
    )]);
    let cases = [
        (("PLEX_UID", "PLEX_GID"), &none, 0),
        (("plex_uid", "PLEX_GID"), &none, 1),
        (("TZ", "PLEX_GID"), &none, 1),
        (("PLEX_ID", "PLEX_ID"), &none, 1),
        (("PLEX_UID", "PLEX_GID"), &declared, 1),
    ];
    for ((uid_env, gid_env), declared, refused) in cases {
        let mode = UserMode::IdsIn(IdVariables {
            uid_env: uid_env.to_owned(),
            gid_env: gid_env.to_owned(),
        });
        let mut service = plain(declared);
        service.user_mode = Some(&mode);
        let said = wrong(&service);
        assert_eq!(said.len(), refused, "{uid_env} {gid_env}: {said:#?}");
    }
}
