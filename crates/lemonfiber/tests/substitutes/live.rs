//! An arm run live: the stack set up and started by this build, the arm's curator wired by
//! its seed alone, a household member's request carried through the request service, the
//! gate and the curator to a grab, and the run undone.
//!
//! The one thing faked is the indexer site the operator gives the stack's indexer, which
//! [`site`] serves: the grab is proved where it serves the release's NZB, and again where
//! the core's own queue read lists the download.

mod core;
mod http;
mod site;

use std::time::{Duration, Instant};

use self::core::{strings_under, Core};
use self::site::{Release, Site};

/// The service whose key adds the site as an indexer, and where its port is declared.
const INDEXER: &str = "prowlarr";

/// The credential the indexer's key is held under.
const INDEXER_KEY: &str = "PROWLARR_API_KEY";

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

/// The member who asks.
const MEMBER: &str = "arm";

/// How long the request may take to reach a grab, and the grab the queue.
const GRAB_WITHIN: Duration = Duration::from_secs(20 * 60);

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
        release: "Big.Buck.Bunny.2008.1080p.BluRay.x264-LEMONFIBER",
        category: "2000",
    },
    Asked {
        media: "tv",
        forms: &["tv", "library"],
        kind: "tv",
        id: "1399",
        seasons: &["1"],
        release: "Game.of.Thrones.S01.1080p.BluRay.x264-LEMONFIBER",
        category: "5000",
    },
];

/// One step of the run, held to having succeeded.
fn step(ran: &self::core::Ran, what: &str) {
    assert!(ran.succeeded, "{what} failed:\n{}", ran.printed);
}

/// Wait until `done`, looking every [`LOOKING`] for at most `within`.
fn waited(within: Duration, mut done: impl FnMut() -> bool) -> bool {
    let started = Instant::now();
    while started.elapsed() < within {
        if done() {
            return true;
        }
        std::thread::sleep(LOOKING);
    }
    done()
}

/// The port the bundle's `service` of `plugin` publishes, as its copy declares it.
fn published(plugin: &str, service: &str) -> Option<u16> {
    let at = super::root().join(format!(
        "assets/plugins/bundle/plugins/{plugin}/plugin.toml"
    ));
    let manifest: toml::Value = super::read(&at).ok()?;
    manifest
        .get("service")?
        .as_array()?
        .iter()
        .find(|one| one.get("id").and_then(toml::Value::as_str) == Some(service))?
        .get("port")?
        .as_integer()
        .and_then(|port| u16::try_from(port).ok())
}

/// The gateway of the network the indexer's container sits on, which reaches this machine.
fn gateway() -> Option<String> {
    let output = std::process::Command::new("docker")
        .args([
            "inspect",
            "--format",
            "{{range .NetworkSettings.Networks}}{{.Gateway}} {{end}}",
            &format!("lemonfiber-{INDEXER}-1"),
        ])
        .output()
        .ok()?;
    String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .next()
        .map(str::to_owned)
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
            one.get("implementation")
                .and_then(serde_json::Value::as_str)
                == Some("Newznab")
                && one
                    .get("definitionName")
                    .and_then(serde_json::Value::as_str)
                    == Some("generic-newznab")
        })
        .cloned()
        .ok_or_else(|| format!("no generic Newznab definition in {}", schema.body))?;
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

/// Run `asked` live through a stack this build sets up beneath `root`, with the usenet
/// login `usenet` gives setup.
///
/// # Panics
///
/// At the first step that does not come to what it should, naming it and what was said.
pub fn run(root: &std::path::Path, asked: &Asked, usenet: &[&str]) {
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
        "--vpn",
        "false",
        "--household",
        "true",
        "--autostart",
        "false",
    ];
    setup.extend(usenet);
    step(&core.run(&setup, ""), "setup");
    let mut up = vec!["up"];
    up.extend(asked.forms);
    step(&core.agreed(&up), "up");
    step(&core.run(&["seed"], ""), "seed");

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

    step(&core.run(&["invite", MEMBER, "--confirm"], ""), "invite");
    let mut ask = vec!["household", "ask", MEMBER, asked.id, "--kind", asked.kind];
    for season in asked.seasons {
        ask.extend(["--season", season]);
    }
    step(&core.run(&ask, ""), "household ask");

    assert!(
        waited(GRAB_WITHIN, || site.grabbed()),
        "the site never served the release's NZB"
    );
    assert!(
        waited(GRAB_WITHIN, || {
            core.run(&["queue"], "").printed.contains(asked.release)
        }),
        "the queue never listed {}",
        asked.release
    );

    let history = core.run(&["history"], "");
    step(&history, "history");
    let stamp = strings_under(&history.said, "at").into_iter().next();
    assert!(stamp.is_some(), "nothing to undo:\n{}", history.printed);
    if let Some(stamp) = stamp {
        step(&core.agreed(&["undo", &stamp]), "undo");
    }
}
