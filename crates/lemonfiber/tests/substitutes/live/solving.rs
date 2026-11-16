//! A challenge solver standing in, run live: the stack's indexer told the solver by its
//! seed alone, under the tag that sends a site through it, and testing it; and the
//! solver's adapter, asked as the core asks it, fetching a page out on the internet and
//! refusing one inside the stack before the solver is handed it.

use std::sync::Arc;

use lemonfiber_contract::capabilities::indexer::{proxy, SOLVER_TAG};
use lemonfiber_contract::Contracted;
use lemonfiber_core::ports::service::{Failure, Solving as _};

use super::core::Core;
use super::{http, Substitution, INDEXER, USENET};

/// The forms the stack is started with: the indexer and the solver it fetches through,
/// and the library, whose media server, request gate and pass service the seed wires too.
const FORMS: [&str; 2] = ["search", "library"];

/// Where the indexer lists the solvers it holds.
const PROXIES: &str = "/api/v1/indexerproxy";

/// Where the indexer lists its tags.
const TAGS: &str = "/api/v1/tag";

/// Where the indexer tests a solver it is handed.
const TESTED: &str = "/api/v1/indexerproxy/test";

/// The field of a solver the indexer holds that names where it reaches the solver.
const HOST: &str = "host";

/// A page out on the internet kept for examples, and words it carries.
const OUTSIDE: (&str, &str) = ("https://example.com/", "Example Domain");

/// The status a page fetched whole is answered with.
const FETCHED: u16 = 200;

/// The interface the engine publishes an adapter's contract port on.
const LOOPBACK: &str = "127.0.0.1";

/// The job's setting naming the challenged site: a site out on the internet that answers
/// a request without its clearance cookie with a challenge, hands that cookie only to a
/// browser that runs its page, and answers Newznab only to a request carrying it.
const CHALLENGED: &str = "SUBSTITUTES_CHALLENGE_SITE";

/// The media type whose release the challenged site holds.
const CHALLENGED_MEDIA: &str = "movies";

/// The JSON the indexer answers at `path`, asked under `key` on `port`, or why it did not.
fn read(key: &str, port: u16, path: &str) -> Result<serde_json::Value, String> {
    let answered = http::call("GET", port, path, key, None)?;
    if answered.status / 100 != 2 {
        return Err(format!("{path}: {} {}", answered.status, answered.body));
    }
    serde_json::from_str(&answered.body).map_err(|why| format!("{path}: {why}"))
}

/// The id the indexer holds [`SOLVER_TAG`] under.
fn solver_tag(key: &str, port: u16) -> Result<i64, String> {
    let tags = read(key, port, TAGS)?;
    tags.as_array()
        .into_iter()
        .flatten()
        .find(|tag| tag.get("label").and_then(serde_json::Value::as_str) == Some(SOLVER_TAG))
        .and_then(|tag| tag.get("id").and_then(serde_json::Value::as_i64))
        .ok_or_else(|| format!("the indexer holds no tag {SOLVER_TAG}: {tags}"))
}

/// The solver the indexer holds at `at` under [`SOLVER_TAG`], as it lists it.
fn held_at(key: &str, port: u16, at: &str) -> Result<serde_json::Value, String> {
    let tag = solver_tag(key, port)?;
    let proxies = read(key, port, PROXIES)?;
    proxies
        .as_array()
        .into_iter()
        .flatten()
        .find(|proxy| {
            let tagged = proxy
                .get("tags")
                .and_then(serde_json::Value::as_array)
                .is_some_and(|tags| tags.iter().any(|one| one.as_i64() == Some(tag)));
            let host = proxy
                .get("fields")
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flatten()
                .find(|field| field.get("name").and_then(serde_json::Value::as_str) == Some(HOST))
                .and_then(|field| field.get("value").and_then(serde_json::Value::as_str));
            tagged && host.map(|host| host.trim_end_matches('/')) == Some(at)
        })
        .cloned()
        .ok_or_else(|| format!("the indexer holds no solver at {at} under {SOLVER_TAG}: {proxies}"))
}

/// The challenged site at `site` added to the indexer under [`SOLVER_TAG`], and searched
/// for the release it holds: found only where the indexer fetched it through the solver.
fn challenged(key: &str, port: u16, site: &str) -> Result<(), String> {
    let asked = super::ASKED
        .iter()
        .find(|asked| asked.media == CHALLENGED_MEDIA)
        .ok_or_else(|| format!("no release is asked for {CHALLENGED_MEDIA}"))?;
    let id = super::site_added(key, port, site, &[solver_tag(key, port)?])?;
    let path = format!(
        "/api/v1/search?query={}&indexerIds={id}&type=search",
        asked.traced.replace(' ', "%20")
    );
    let found = read(key, port, &path)?;
    let titles: Vec<&str> = found
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|release| release.get("title").and_then(serde_json::Value::as_str))
        .collect();
    if titles.contains(&asked.release) {
        Ok(())
    } else {
        Err(format!("{site} answered {titles:?}, not {}", asked.release))
    }
}

/// Where the indexer reaches the upstream the adapter of `plugin` filling `capability`
/// fronts: its name on the stack's network and the port it listens on there.
fn upstream_at(plugin: &str, capability: &str) -> Option<String> {
    let adapter = super::providing(plugin, capability)?;
    let upstream = super::declared(plugin, &adapter, "fronts")?;
    let upstream = upstream.as_str()?;
    let listens = super::port_of(plugin, upstream, "listens")?;
    Some(format!("http://{upstream}:{listens}"))
}

/// The host port the engine publishes `listens` of `service` on, on loopback.
fn published_on_loopback(service: &str, listens: u16) -> Option<u16> {
    let output = std::process::Command::new("docker")
        .args(["port", &super::container(service), &listens.to_string()])
        .output()
        .ok()?;
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| {
            line.trim()
                .strip_prefix(&format!("{LOOPBACK}:"))?
                .parse()
                .ok()
        })
}

/// The adapter of `plugin` filling `capability`, asked over its contract as the core asks
/// it: on the loopback port the engine published, under the key written beside it.
fn adapter(core: &Core, plugin: &str, capability: &str) -> Result<proxy::Adapter, String> {
    let service = super::providing(plugin, capability)
        .ok_or_else(|| format!("no service of {plugin} provides {capability}"))?;
    let listens = super::port_of(plugin, &service, "listens")
        .ok_or_else(|| format!("{service} declares no port it listens on"))?;
    let port = published_on_loopback(&service, listens)
        .ok_or_else(|| format!("{service} publishes no port on loopback for {listens}"))?;
    let at = lemonfiber_core::plugin::key_file(&core.paths().stack(), &service);
    let key = std::fs::read_to_string(&at).map_err(|why| format!("{}: {why}", at.display()))?;
    Ok(proxy::Adapter(Contracted::new(
        Arc::new(lemonfiber_adapters::Web::new()),
        format!("http://{LOOPBACK}:{port}"),
        service,
        key.trim(),
    )))
}

/// Run `substitution`, a challenge solver, live through a stack this build sets up
/// beneath `root`.
///
/// # Panics
///
/// At the first step that does not come to what it should, naming it and what was said.
pub fn run(root: &std::path::Path, substitution: &Substitution) {
    let Some(core) = super::started(root, &FORMS, &USENET, Some(substitution)) else {
        return;
    };
    let Some((key, port)) = super::indexer(&core) else {
        return;
    };
    let at = upstream_at(&substitution.plugin, &substitution.capability);
    assert!(
        at.is_some(),
        "{substitution:?}: no upstream its adapter fronts"
    );
    let Some(at) = at else {
        return;
    };
    let held = held_at(&key, port, &at);
    assert!(held.is_ok(), "{held:?}");
    let Ok(held) = held else {
        return;
    };
    let tested = http::call("POST", port, TESTED, &key, Some(&held.to_string()));
    assert!(
        tested.as_ref().is_ok_and(|tested| tested.status / 100 == 2),
        "the indexer's test of the solver at {at}: {tested:?}"
    );

    let solver = adapter(&core, &substitution.plugin, &substitution.capability);
    assert!(solver.is_ok(), "{:?}", solver.as_ref().err());
    let Ok(solver) = solver else {
        return;
    };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build();
    assert!(runtime.is_ok(), "{:?}", runtime.as_ref().err());
    let Ok(runtime) = runtime else {
        return;
    };
    let fetched = runtime.block_on(solver.solve(OUTSIDE.0));
    assert!(
        fetched
            .as_ref()
            .is_ok_and(|page| page.status == FETCHED && page.page.contains(OUTSIDE.1)),
        "{}: {:?}",
        OUTSIDE.0,
        fetched.map(|page| (page.status, page.url))
    );
    let listens = super::port_of(INDEXER, INDEXER, "listens");
    assert!(
        listens.is_some(),
        "{INDEXER} declares no port it listens on"
    );
    let Some(listens) = listens else {
        return;
    };
    let inside = format!("http://{INDEXER}:{listens}/");
    let refused = runtime.block_on(solver.solve(&inside));
    assert!(
        matches!(refused, Err(Failure::Disallowed { .. })),
        "{inside}: {:?}",
        refused.map(|page| (page.status, page.url))
    );

    match std::env::var(CHALLENGED)
        .ok()
        .filter(|site| !site.is_empty())
    {
        Some(site) => {
            let fetched = challenged(&key, port, &site);
            assert!(fetched.is_ok(), "{fetched:?}");
        }
        None => println!("The challenged site is not asked: {CHALLENGED} names no site."),
    }

    super::undone(&core);
}
