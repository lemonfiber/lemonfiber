//! An arm run live: the stack set up and started by this build, the arm's curator wired by
//! its seed alone, a household member's request carried through the request service, the
//! gate and the curator to a grab, and the run undone.
//!
//! The one thing faked is the indexer site the operator gives the stack's indexer, which
//! [`site`] serves: the grab is proved where it serves the release's NZB, and again where
//! the core's own trace of the title reaches a grab.

mod core;
mod http;
mod member;
mod site;
mod substitute;

pub use self::substitute::Substitution;

use std::time::{Duration, Instant};

use self::core::{strings_under, Core};
use self::site::{Release, Site};

/// The service whose key adds the site as an indexer, and where its port is declared.
const INDEXER: &str = "prowlarr";

/// The credential the indexer's key is held under.
const INDEXER_KEY: &str = "prowlarr:upstream-key";

/// The Usenet login setup is given: a provider over TLS, which nothing in the run
/// downloads from, since the grab is proved where the download is queued.
pub const USENET: [&str; 10] = [
    "--usenet-host",
    "news.lemonfiber.invalid",
    "--usenet-port",
    "563",
    "--usenet-user",
    "arm",
    "--usenet-pass",
    "arm-password",
    "--usenet-tls",
    "true",
];

/// What setup answers once its answers are written. Its own start stops at the install a
/// stack's first start offers, which `up` then agrees to.
const APPLIED: &str = "applied";

/// The indexer's generic Newznab definition: its implementation, and the name or the
/// definition name it is listed under, which Prowlarr releases differ on.
const NEWZNAB: (&str, &str, &str) = ("Newznab", "Generic Newznab", "generic-newznab");

/// The policy the member asks under, so their ask reaches the curator without the
/// operator's yes.
const TRUSTED: &str = "trusted";

/// The member who asks.
const MEMBER: &str = "arm";

/// How long the request may take to reach a grab, and the grab the queue.
const GRAB_WITHIN: Duration = Duration::from_secs(20 * 60);

/// Every stage a trace reports once a release was sent to the download client.
const GRABBED: [&str; 6] = [
    "grabbed",
    "downloading",
    "downloaded",
    "importing",
    "imported",
    "available",
];

/// How often a wait looks again.
const LOOKING: Duration = Duration::from_secs(15);

/// What an arm's household asks for, by the media its curator files.
#[derive(Debug, Clone, Copy)]
pub struct Asked {
    /// The media type.
    pub media: &'static str,
    /// The forms the stack is started with.
    pub forms: &'static [&'static str],
    /// The kind `household ask` is given.
    pub kind: &'static str,
    /// The title, by the request service's catalogue id.
    pub id: &'static str,
    /// The seasons asked for, for a series.
    pub seasons: &'static [&'static str],
    /// What the operator calls it, which a trace follows it by.
    pub traced: &'static str,
    /// The release the site holds for it.
    pub release: &'static str,
    /// The Newznab category the release is filed under.
    pub category: &'static str,
}

/// What each media type's household asks for.
pub const ASKED: [Asked; 2] = [
    Asked {
        media: "movies",
        forms: &["movies", "library"],
        kind: "movies",
        id: "10378",
        seasons: &[],
        traced: "Big Buck Bunny",
        release: "Big.Buck.Bunny.2008.1080p.BluRay.x264-LEMONFIBER",
        category: "2000",
    },
    Asked {
        media: "tv",
        forms: &["tv", "library"],
        kind: "tv",
        id: "1399",
        seasons: &["1"],
        traced: "Game of Thrones",
        release: "Game.of.Thrones.S01.1080p.BluRay.x264-LEMONFIBER",
        category: "5000",
    },
];

/// One step of the run, held to having succeeded.
fn step(ran: &self::core::Ran, what: &str) {
    assert!(ran.succeeded, "{what} failed:\n{}", ran.printed);
}

/// Wait until `done`, looking every [`LOOKING`] for at most `within`.
fn waited(within: Duration, done: impl FnMut() -> bool) -> bool {
    waited_every(within, LOOKING, done)
}

/// Wait until `done`, looking every `every` for at most `within`.
fn waited_every(within: Duration, every: Duration, mut done: impl FnMut() -> bool) -> bool {
    let started = Instant::now();
    while started.elapsed() < within {
        if done() {
            return true;
        }
        std::thread::sleep(every);
    }
    done()
}

/// Every service the bundle's copy of `plugin` declares.
fn services_of(plugin: &str) -> Vec<toml::Value> {
    let at = super::root().join(format!(
        "assets/plugins/bundle/plugins/{plugin}/plugin.toml"
    ));
    super::read::<toml::Value>(&at)
        .ok()
        .and_then(|manifest| manifest.get("service")?.as_array().cloned())
        .unwrap_or_default()
}

/// The port the bundle's `service` of `plugin` publishes, as its copy declares it.
fn published(plugin: &str, service: &str) -> Option<u16> {
    services_of(plugin)
        .iter()
        .find(|one| one.get("id").and_then(toml::Value::as_str) == Some(service))?
        .get("port")?
        .as_integer()
        .and_then(|port| u16::try_from(port).ok())
}

/// The service of the bundle's copy of `plugin` that provides `capability`.
fn providing(plugin: &str, capability: &str) -> Option<String> {
    services_of(plugin)
        .iter()
        .find(|one| super::strings(one, "provides").contains(&capability))?
        .get("id")?
        .as_str()
        .map(str::to_owned)
}

/// The first address `docker` prints for `args`, of the words it prints.
fn docker_address(args: &[&str]) -> Option<String> {
    let output = std::process::Command::new("docker")
        .args(args)
        .output()
        .ok()?;
    String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .find(|word| word.parse::<std::net::IpAddr>().is_ok())
        .map(str::to_owned)
}

/// The gateway of the network the indexer's container sits on, which reaches this machine.
fn gateway() -> Option<String> {
    let output = std::process::Command::new("docker")
        .args([
            "inspect",
            "--format",
            "{{range $name, $_ := .NetworkSettings.Networks}}{{$name}} {{end}}",
            &format!("lemonfiber-{INDEXER}-1"),
        ])
        .output()
        .ok()?;
    String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .find_map(|network| {
            docker_address(&[
                "network",
                "inspect",
                network,
                "--format",
                "{{range .IPAM.Config}}{{.Gateway}} {{end}}",
            ])
        })
}

/// This machine's address on Docker's default bridge, which the household's links name.
fn bridged() -> Option<String> {
    docker_address(&[
        "network",
        "inspect",
        "bridge",
        "--format",
        "{{range .IPAM.Config}}{{.Gateway}} {{end}}",
    ])
}

/// The site added to the indexer as the operator adds one: its own Newznab definition
/// from the indexer's schema, pointed at the site, under the key the core minted for it.
fn site_added(key: &str, port: u16, site: &str) -> Result<(), String> {
    let schema = http::call("GET", port, "/api/v1/indexer/schema", key, None)?;
    let schemas: serde_json::Value =
        serde_json::from_str(&schema.body).map_err(|why| format!("{why}: {}", schema.body))?;
    let mut definition = schemas
        .as_array()
        .into_iter()
        .flatten()
        .find(|one| {
            let said = |field: &str| one.get(field).and_then(serde_json::Value::as_str);
            said("implementation") == Some(NEWZNAB.0)
                && (said("name") == Some(NEWZNAB.1) || said("definitionName") == Some(NEWZNAB.2))
        })
        .cloned()
        .ok_or_else(|| {
            let listed: Vec<&str> = schemas
                .as_array()
                .into_iter()
                .flatten()
                .filter(|one| {
                    one.get("implementation")
                        .and_then(serde_json::Value::as_str)
                        == Some(NEWZNAB.0)
                })
                .filter_map(|one| one.get("name").and_then(serde_json::Value::as_str))
                .collect();
            format!("no generic Newznab definition among {listed:?}")
        })?;
    if let Some(fields) = definition
        .get_mut("fields")
        .and_then(serde_json::Value::as_array_mut)
    {
        for field in fields {
            let value = match field.get("name").and_then(serde_json::Value::as_str) {
                Some("baseUrl") => site.to_owned(),
                Some("apiPath") => "/api".to_owned(),
                Some("apiKey") => site::KEY.to_owned(),
                _ => continue,
            };
            if let Some(field) = field.as_object_mut() {
                field.insert("value".to_owned(), value.into());
            }
        }
    }
    if let Some(fields) = definition.as_object_mut() {
        fields.insert("name".to_owned(), "lemonfiber arm".into());
        fields.insert("enable".to_owned(), true.into());
        fields.insert("appProfileId".to_owned(), 1.into());
    }
    let added = http::call(
        "POST",
        port,
        "/api/v1/indexer",
        key,
        Some(&definition.to_string()),
    )?;
    if added.status / 100 == 2 {
        Ok(())
    } else {
        Err(format!("{}: {}", added.status, added.body))
    }
}

/// The member invited, let ask under [`TRUSTED`], and asking for `asked` through the web
/// surface `core` serves.
fn member_asked(core: &Core, asked: &Asked) {
    step(&core.run(&["invite", MEMBER], ""), "invite");
    step(
        &core.agreed(&[
            "household",
            "allow",
            "--member",
            MEMBER,
            "--policy",
            TRUSTED,
        ]),
        "the member's policy",
    );
    let served = member::Served::by(core);
    assert!(served.is_ok(), "{:?}", served.err());
    let Ok(served) = served else {
        return;
    };
    let ask = member::asked(core, &served, MEMBER, asked);
    assert!(ask.is_ok(), "the member's ask: {ask:?}");
}

/// Run `asked` live through a stack this build sets up beneath `root`, with the usenet
/// login `usenet` gives setup, and `substitution` put in place once it is first seeded.
///
/// # Panics
///
/// At the first step that does not come to what it should, naming it and what was said.
pub fn run(
    root: &std::path::Path,
    asked: &Asked,
    usenet: &[&str],
    substitution: Option<&Substitution>,
) {
    let site = Site::serving(Release {
        title: asked.release.to_owned(),
        category: asked.category,
    });
    let core = Core::beneath(root);
    let media = root.join("media");
    let media = media.to_string_lossy();
    let mut setup = vec![
        "setup",
        "--yes",
        "--protocols",
        "usenet",
        "--data-location",
        &media,
        "--library",
        "docker",
        "--vpn",
        "false",
        "--household",
        "true",
        "--autostart",
        "false",
    ];
    setup.extend(usenet);
    let set_up = core.run(&setup, "");
    assert!(
        strings_under(&set_up.said, "outcome")
            .iter()
            .any(|outcome| outcome == APPLIED),
        "setup was not applied:\n{}",
        set_up.printed
    );
    let household = bridged();
    assert!(household.is_some(), "no address the household reaches");
    let Some(household) = household else {
        return;
    };
    let addressed = format!("{}={household}", lemonfiber_manifest::container::LAN_HOST);
    step(
        &core.agreed(&["settings", "set", &addressed]),
        "the household's address",
    );
    let mut up = vec!["up"];
    up.extend(asked.forms);
    step(&core.agreed(&up), "up");
    step(&core.run(&["seed"], ""), "seed");
    if let Some(substitution) = substitution {
        let made = substitution.made(&core);
        assert!(made.is_ok(), "{made:?}");
        step(&core.run(&["seed"], ""), "seed after the substitution");
    }

    let revealed = core.run(&["credentials", "--reveal", INDEXER_KEY, "--confirm"], "");
    step(&revealed, "the indexer's key");
    let key = strings_under(&revealed.said, "value").into_iter().next();
    assert!(key.is_some(), "no key revealed:\n{}", revealed.printed);
    let port = published(INDEXER, INDEXER);
    let gateway = gateway();
    assert!(port.is_some() && gateway.is_some(), "{port:?} {gateway:?}");
    let (Some(key), Some(port), Some(gateway)) = (key, port, gateway) else {
        return;
    };
    let added = site_added(&key, port, &format!("http://{gateway}:{}", site.port));
    assert!(added.is_ok(), "{added:?}");
    step(&core.run(&["seed"], ""), "seed after the indexer");

    member_asked(&core, asked);

    assert!(
        waited(GRAB_WITHIN, || site.grabbed()),
        "the site never served the release's NZB"
    );
    assert!(
        waited(GRAB_WITHIN, || {
            strings_under(&core.run(&["trace", asked.traced], "").said, "furthest")
                .iter()
                .any(|stage| GRABBED.contains(&stage.as_str()))
        }),
        "no trace of {} reached a grab",
        asked.traced
    );

    let history = core.run(&["history"], "");
    step(&history, "history");
    let stamp = strings_under(&history.said, "at").into_iter().next();
    assert!(stamp.is_some(), "nothing to undo:\n{}", history.printed);
    if let Some(stamp) = stamp {
        step(&core.agreed(&["undo", &stamp]), "undo");
    }
}
