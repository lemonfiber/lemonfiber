//! An arm run live: the stack set up and started by this build, the arm's curator wired by
//! its seed alone, a household member's request carried through the request service, the
//! gate and the curator to a grab, and the run undone.
//!
//! The one thing faked is the indexer site the operator gives the stack's indexer, which
//! [`site`] serves: the grab is proved where it serves the release's NZB or torrent, and
//! again where the core's own trace of the title reaches a grab. A download client standing
//! in is then paused and resumed with every other one.

pub mod core;
mod http;
mod member;
mod site;
pub mod solving;
mod substitute;

pub use self::substitute::Substitution;

use std::time::{Duration, Instant};

use self::core::{strings_under, Core};
use self::site::{Carried, Release, Site};

/// The service whose key adds the site as an indexer, and where its port is declared.
const INDEXER: &str = "prowlarr";

/// The credential the indexer's key is held under.
const INDEXER_KEY: &str = "prowlarr:upstream-key";

/// The Usenet login setup is given: a provider over TLS, which nothing in the run
/// downloads from, since the grab is proved where the download is queued.
const USENET_LOGIN: [&str; 10] = [
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

/// How a stack downloads what its curators grab, as setup is told and the indexer site
/// serves it.
#[derive(Debug, Clone, Copy)]
pub struct Downloads {
    /// The protocol setup is given.
    protocol: &'static str,
    /// The login setup is given for it.
    login: &'static [&'static str],
    /// The indexer's generic definition for a site of it: its implementation, and the
    /// name or the definition name it is listed under, which Prowlarr releases differ on.
    definition: (&'static str, &'static str, &'static str),
    /// What the site serves its release as.
    carried: Carried,
}

/// Downloading over Usenet, from a Newznab site.
pub const USENET: Downloads = Downloads {
    protocol: "usenet",
    login: &USENET_LOGIN,
    definition: ("Newznab", "Generic Newznab", "generic-newznab"),
    carried: Carried::Nzb,
};

/// Downloading torrents, from a Torznab site. Setup is told there is no VPN, which it warns
/// about and goes on with, since nothing in the run reaches a peer.
pub const TORRENT: Downloads = Downloads {
    protocol: "torrent",
    login: &[],
    definition: ("Torznab", "Generic Torznab", "generic-torznab"),
    carried: Carried::Torrent,
};

/// What a download client is read back as once paused, and once resumed.
const PAUSED: (&str, &str) = ("stopped", "fetching");

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

/// The size of a film's or a series' release, which its curator takes.
const VIDEO: u64 = 1_073_741_824;

/// The size of an album's release, which its curator takes.
const ALBUM: u64 = 314_572_800;

/// The size of a book's release, which its curator takes.
const BOOK: u64 = 2_097_152;

/// The title a member asks for.
#[derive(Debug, Clone, Copy)]
pub enum Title {
    /// By the request service's catalogue id, where a request service holds the ask.
    Catalogued(&'static str),
    /// As the member finds it through the curator's lookup, where the core holds the ask.
    Found {
        /// The unit asked for.
        unit: &'static str,
        /// The words searched for.
        term: &'static str,
        /// What the candidate asked for is called.
        named: &'static str,
    },
}

/// What an arm's household asks for, by the media its curator files.
#[derive(Debug, Clone, Copy)]
pub struct Asked {
    /// The media type.
    pub media: &'static str,
    /// The forms the stack is started with.
    pub forms: &'static [&'static str],
    /// The kind `household ask` is given.
    pub kind: &'static str,
    /// The title.
    pub title: Title,
    /// The seasons asked for, for a series.
    pub seasons: &'static [&'static str],
    /// What the operator calls it, which a trace follows it by.
    pub traced: &'static str,
    /// The release the site holds for it.
    pub release: &'static str,
    /// The Newznab category the release is filed under.
    pub category: &'static str,
    /// The release's size in bytes.
    pub size: u64,
}

/// What each media type's household asks for.
pub const ASKED: [Asked; 4] = [
    Asked {
        media: "movies",
        forms: &["movies", "library"],
        kind: "movies",
        title: Title::Catalogued("10378"),
        seasons: &[],
        traced: "Big Buck Bunny",
        release: "Big.Buck.Bunny.2008.1080p.BluRay.x264-LEMONFIBER",
        category: "2000",
        size: VIDEO,
    },
    Asked {
        media: "tv",
        forms: &["tv", "library"],
        kind: "tv",
        title: Title::Catalogued("1399"),
        seasons: &["1"],
        traced: "Game of Thrones",
        release: "Game.of.Thrones.S01.1080p.BluRay.x264-LEMONFIBER",
        category: "5000",
        size: VIDEO,
    },
    Asked {
        media: "music",
        forms: &["music", "library"],
        kind: "music",
        title: Title::Found {
            unit: "album",
            term: "Nine Inch Nails The Slip",
            named: "The Slip",
        },
        seasons: &[],
        traced: "The Slip",
        release: "Nine Inch Nails - The Slip (2008) [FLAC]",
        category: "3000",
        size: ALBUM,
    },
    Asked {
        media: "books",
        forms: &["books", "library"],
        kind: "books",
        title: Title::Found {
            unit: "book",
            term: "Pride and Prejudice Jane Austen",
            named: "Pride and Prejudice",
        },
        seasons: &[],
        traced: "Pride and Prejudice",
        release: "Jane Austen - Pride and Prejudice (1813) [EPUB]",
        category: "7020",
        size: BOOK,
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

/// What the bundle's copy of `plugin` declares for its `service` under `field`.
fn declared(plugin: &str, service: &str, field: &str) -> Option<toml::Value> {
    services_of(plugin)
        .iter()
        .find(|one| one.get("id").and_then(toml::Value::as_str) == Some(service))?
        .get(field)
        .cloned()
}

/// The port `field` of the bundle's `service` of `plugin` names, as its copy declares it.
fn port_of(plugin: &str, service: &str, field: &str) -> Option<u16> {
    declared(plugin, service, field)?
        .as_integer()
        .and_then(|port| u16::try_from(port).ok())
}

/// The port the bundle's `service` of `plugin` publishes, as its copy declares it.
fn published(plugin: &str, service: &str) -> Option<u16> {
    port_of(plugin, service, "port")
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

/// The container the engine runs `service` in.
fn container(service: &str) -> String {
    format!("lemonfiber-{service}-1")
}

/// The gateway of the network the indexer's container sits on, which reaches this machine.
fn gateway() -> Option<String> {
    let output = std::process::Command::new("docker")
        .args([
            "inspect",
            "--format",
            "{{range $name, $_ := .NetworkSettings.Networks}}{{$name}} {{end}}",
            &container(INDEXER),
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

/// The site added to the indexer as the operator adds one: the indexer's own generic
/// definition for `downloads` from its schema, pointed at the site and under `tags`, asked
/// under the key the core minted for it. The id the indexer gave it.
fn site_added(
    key: &str,
    port: u16,
    site: &str,
    (downloads, tags): (&Downloads, &[i64]),
) -> Result<i64, String> {
    let (implementation, name, definition_name) = downloads.definition;
    let schema = http::call("GET", port, "/api/v1/indexer/schema", key, None)?;
    let schemas: serde_json::Value =
        serde_json::from_str(&schema.body).map_err(|why| format!("{why}: {}", schema.body))?;
    let mut definition = schemas
        .as_array()
        .into_iter()
        .flatten()
        .find(|one| {
            let said = |field: &str| one.get(field).and_then(serde_json::Value::as_str);
            said("implementation") == Some(implementation)
                && (said("name") == Some(name) || said("definitionName") == Some(definition_name))
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
                        == Some(implementation)
                })
                .filter_map(|one| one.get("name").and_then(serde_json::Value::as_str))
                .collect();
            format!("no generic {implementation} definition among {listed:?}")
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
        fields.insert("tags".to_owned(), tags.into());
    }
    let added = http::call(
        "POST",
        port,
        "/api/v1/indexer",
        key,
        Some(&definition.to_string()),
    )?;
    if added.status / 100 != 2 {
        return Err(format!("{}: {}", added.status, added.body));
    }
    serde_json::from_str::<serde_json::Value>(&added.body)
        .ok()
        .and_then(|made| made.get("id").and_then(serde_json::Value::as_i64))
        .ok_or_else(|| format!("no id in {}", added.body))
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

/// Every download client `core` pauses, then resumes, each as it read back, held to the
/// one `client` standing in reading back each as it should, and every other client named
/// being one of `passed_over`, the services of the part it stands in for.
fn paused_and_resumed(core: &Core, client: &str, passed_over: &[String]) {
    for (asked, now) in [("pause", PAUSED.0), ("resume", PAUSED.1)] {
        let ran = core.run(&["downloads", asked], "");
        step(&ran, asked);
        let clients: Vec<&serde_json::Value> = ran
            .said
            .pointer("/data/clients")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .collect();
        let named = |one: &serde_json::Value, key: &str| {
            one.get(key)
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        };
        let standing_in: Vec<(Option<String>, Option<String>)> = clients
            .iter()
            .filter(|one| named(one, "client").as_deref() == Some(client))
            .map(|one| (named(one, "now"), named(one, "unreached")))
            .collect();
        assert_eq!(
            standing_in,
            vec![(Some(now.to_owned()), None)],
            "{asked}: {client} in\n{}",
            ran.printed
        );
        let others: Vec<String> = clients
            .iter()
            .filter_map(|one| named(one, "client"))
            .filter(|named| named != client && !passed_over.contains(named))
            .collect();
        assert!(
            others.is_empty(),
            "{asked} named {others:?}:\n{}",
            ran.printed
        );
    }
}

/// A stack this build sets up beneath `root` and starts with `forms`, downloading as
/// `downloads` says, seeded, and `substitution` put in place once it is first seeded,
/// seeded again and its adapters proved. Nothing where the household has no address to
/// reach it at.
///
/// # Panics
///
/// At the first step that does not come to what it should, naming it and what was said.
fn started(
    root: &std::path::Path,
    forms: &[&str],
    downloads: &Downloads,
    substitution: Option<&Substitution>,
) -> Option<Core> {
    let core = Core::beneath(root);
    let media = root.join("media");
    let media = media.to_string_lossy();
    let mut setup = vec![
        "setup",
        "--yes",
        "--protocols",
        downloads.protocol,
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
    setup.extend(downloads.login);
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
    let household = household?;
    let addressed = format!("{}={household}", lemonfiber_manifest::container::LAN_HOST);
    step(
        &core.agreed(&["settings", "set", &addressed]),
        "the household's address",
    );
    let mut up = vec!["up"];
    up.extend(forms);
    step(&core.agreed(&up), "up");
    step(&core.run(&["seed"], ""), "seed");
    if let Some(substitution) = substitution {
        let made = substitution.made(&core);
        assert!(made.is_ok(), "{made:?}");
        step(&core.run(&["seed"], ""), "seed after the substitution");
        step(
            &core.agreed(&["plugin", "prove", &substitution.plugin]),
            "conformance",
        );
    }
    Some(core)
}

/// The key the core minted for the indexer, and the port it is published on, or nothing
/// where either cannot be read.
///
/// # Panics
///
/// Where the key is not revealed, or the bundle names no port for the indexer.
fn indexer(core: &Core) -> Option<(String, u16)> {
    let revealed = core.run(&["credentials", "--reveal", INDEXER_KEY, "--confirm"], "");
    step(&revealed, "the indexer's key");
    let key = strings_under(&revealed.said, "value").into_iter().next();
    assert!(key.is_some(), "no key revealed:\n{}", revealed.printed);
    let port = published(INDEXER, INDEXER);
    assert!(port.is_some(), "the bundle names no port for {INDEXER}");
    Some((key?, port?))
}

/// The newest change the run made, undone.
///
/// # Panics
///
/// Where there is nothing to undo, or undoing it fails.
fn undone(core: &Core) {
    let history = core.run(&["history"], "");
    step(&history, "history");
    let stamp = history
        .said
        .pointer("/data/changes/0/at")
        .and_then(serde_json::Value::as_u64);
    assert!(stamp.is_some(), "nothing to undo:\n{}", history.printed);
    if let Some(stamp) = stamp {
        step(&core.agreed(&["undo", &stamp.to_string()]), "undo");
    }
}

/// Run `asked` live through a stack this build sets up beneath `root`, downloading as
/// `downloads` says, and `substitution` put in place once it is first seeded; a download
/// client standing in is paused and resumed once the grab is made.
///
/// # Panics
///
/// At the first step that does not come to what it should, naming it and what was said.
pub fn run(
    root: &std::path::Path,
    asked: &Asked,
    downloads: &Downloads,
    substitution: Option<&Substitution>,
) {
    let site = Site::serving(Release {
        title: asked.release.to_owned(),
        category: asked.category,
        size: asked.size,
        carried: downloads.carried,
    });
    let Some(core) = started(root, asked.forms, downloads, substitution) else {
        return;
    };
    let Some((key, port)) = indexer(&core) else {
        return;
    };
    let gateway = gateway();
    assert!(
        gateway.is_some(),
        "no gateway reaches this machine from {INDEXER}"
    );
    let Some(gateway) = gateway else {
        return;
    };
    let added = site_added(
        &key,
        port,
        &format!("http://{gateway}:{}", site.port),
        (downloads, &[]),
    );
    assert!(added.is_ok(), "{added:?}");
    step(&core.run(&["seed"], ""), "seed after the indexer");

    member_asked(&core, asked);

    assert!(
        waited(GRAB_WITHIN, || site.grabbed()),
        "the site never served the release's {:?}",
        downloads.carried
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
    if let Some(substitution) = substitution.filter(|one| one.downloads()) {
        let client = providing(&substitution.plugin, &substitution.capability);
        assert!(client.is_some(), "{substitution:?}");
        if let Some(client) = client {
            paused_and_resumed(&core, &client, &substitution.passed_over());
        }
    }

    undone(&core);
}
