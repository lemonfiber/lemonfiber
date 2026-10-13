//! What a schema 2 bundle and its copies keep: each part's tree and declaration, and
//! the stack's own tables naming only what the pinned parts declare.

use std::collections::{BTreeMap, BTreeSet};

use lemonfiber_manifest::Date;
use toml::Value;

use crate::copies::Copy;
use crate::rules::{is_id, manifested};
use crate::tree::{self, CONFIG};
use crate::{Bundle, Pin, Stage, MANIFEST};

/// How many starting files one service may ship.
const MOST_FILES: usize = 64;

/// How many bytes one service's starting files may weigh in all.
const MOST_BYTES: usize = 1024 * 1024;

/// One service of a part, as its manifest declares it.
#[derive(Debug, Default)]
pub(crate) struct Declared {
    pub(crate) id: String,
    provides: Vec<String>,
    speaks: Vec<String>,
    replaces: Option<String>,
    reaches: Option<String>,
    asks_for: Option<String>,
}

/// One ask of a part: the capability, the asking service where named, and whether it
/// reaches every filler.
#[derive(Debug)]
struct Asked {
    capability: String,
    service: Option<String>,
    each: bool,
}

/// What the bundle reads of a part's manifest.
#[derive(Debug, Default)]
pub(crate) struct Part {
    pub(crate) services: Vec<Declared>,
    asks: Vec<Asked>,
    forms: Vec<String>,
    pub(crate) fixtures: Vec<String>,
}

/// Every rule a schema 2 `bundle` and its `copies` break, added to `broken`.
pub(crate) fn broken(bundle: &Bundle, copies: &BTreeMap<String, Copy>, broken: &mut Vec<String>) {
    let mut parts = BTreeMap::new();
    for pin in &bundle.plugin {
        let Some(part) = copied(pin, copies.get(&pin.id), broken) else {
            continue;
        };
        staged(pin, &part, bundle, broken);
        parts.insert(pin.id.as_str(), part);
    }
    tables(bundle, &parts, broken);
}

/// The part `pin`'s copy declares, where it holds a manifest, with every rule its tree
/// breaks added to `broken`.
fn copied(pin: &Pin, copy: Option<&Copy>, broken: &mut Vec<String>) -> Option<Part> {
    let id = &pin.id;
    let Some(copy) = copy else {
        broken.push(format!("plugin {id} has no copy at {id}/"));
        return None;
    };
    for link in &copy.links {
        broken.push(format!(
            "{id}/{link} is a symbolic link, which no copy may hold"
        ));
    }
    let mut folded = BTreeMap::new();
    for path in copy
        .manifest
        .iter()
        .map(|_| MANIFEST)
        .chain(copy.files.keys().map(String::as_str))
    {
        if let Some(earlier) = folded.insert(path.to_lowercase(), path) {
            broken.push(format!(
                "{id}/{earlier} and {id}/{path} differ only in case, which one file holds where \
                 case is not told apart"
            ));
        }
    }
    let Some(manifest) = &copy.manifest else {
        broken.push(format!("plugin {id} has no {id}/{MANIFEST}"));
        return None;
    };
    manifested(pin, manifest, &format!("{id}/{MANIFEST}"), broken);
    let part = declared(manifest);
    let services: Vec<String> = part.services.iter().map(|one| one.id.clone()).collect();
    for path in copy.files.keys() {
        if !tree::holds(path, &services, &part.fixtures) {
            broken.push(format!("{id}/{path} is not part of plugin {id}'s tree"));
        }
    }
    for service in &services {
        let starting: Vec<&Vec<u8>> = copy
            .files
            .iter()
            .filter(|(path, _)| path.starts_with(&format!("{CONFIG}/{service}/")))
            .map(|(_, bytes)| bytes)
            .collect();
        let weight: usize = starting.iter().map(|bytes| bytes.len()).sum();
        if starting.len() > MOST_FILES || weight > MOST_BYTES {
            broken.push(format!(
                "plugin {id} service {service} ships {} starting files of {weight} bytes, past \
                 {MOST_FILES} files or {MOST_BYTES} bytes",
                starting.len()
            ));
        }
    }
    let digest = tree::digest(manifest, &copy.files);
    if pin.tree.as_deref().is_some_and(|pinned| pinned != digest) {
        broken.push(format!(
            "{id}/ does not match plugin {id}.tree: the copy is not the pinned tree"
        ));
    }
    Some(part)
}

/// Every rule a part breaks against its pin's stage, profile and forms, and the fields
/// schema 2 requires of a pin.
fn staged(pin: &Pin, part: &Part, bundle: &Bundle, broken: &mut Vec<String>) {
    let id = &pin.id;
    for (field, given) in [
        ("tree", pin.tree.is_some()),
        ("stage", pin.stage.is_some()),
        ("profile", pin.profile.is_some()),
        ("last_release", pin.last_release.is_some()),
    ] {
        if !given {
            broken.push(format!(
                "plugin {id}.{field} is missing, which schema 2 requires"
            ));
        }
    }
    if let Some(day) = &pin.last_release {
        if Date::parse(day).is_none() {
            broken.push(format!(
                "plugin {id}.last_release {day:?} is not a day, YYYY-MM-DD"
            ));
        }
    }
    match pin.stage {
        Some(Stage::Assembled) => {
            if part.services.len() != 1 || part.services.iter().any(|one| !one.speaks.is_empty()) {
                broken.push(format!(
                    "plugin {id} is assembled and so declares exactly one service, speaking no \
                     contract"
                ));
            }
        }
        Some(Stage::Installed)
            if !part
                .services
                .iter()
                .any(|one| one.id == *id && one.replaces.as_deref() == Some(id.as_str())) =>
        {
            broken.push(format!(
                "plugin {id} is installed and so declares service {id} with replaces = \"{id}\""
            ));
        }
        Some(Stage::Installed) | None => {}
    }
    for service in &part.services {
        let asked = service
            .asks_for
            .as_deref()
            .is_some_and(|text| !text.trim().is_empty());
        if service.reaches.is_none() || !asked {
            broken.push(format!(
                "plugin {id} service {} says nothing of where it reaches or what it asks for, \
                 which every first-party service says",
                service.id
            ));
        }
    }
    let Some(profile) = &pin.profile else {
        return;
    };
    if !bundle
        .profile
        .iter()
        .any(|declared| declared.id == *profile)
    {
        broken.push(format!(
            "plugin {id}.profile {profile} is not a declared profile"
        ));
    }
    let holding: BTreeSet<&str> = bundle
        .form
        .iter()
        .filter(|form| form.profiles.iter().any(|one| one == profile))
        .map(|form| form.id.as_str())
        .collect();
    let named: BTreeSet<&str> = part.forms.iter().map(String::as_str).collect();
    if holding != named {
        broken.push(format!(
            "plugin {id}'s [plugin].forms names {named:?}, and the forms holding profile \
             {profile} are {holding:?}"
        ));
    }
}

/// Every rule the stack's own tables break against the pinned parts.
fn tables(bundle: &Bundle, parts: &BTreeMap<&str, Part>, broken: &mut Vec<String>) {
    let mut seen = BTreeSet::new();
    for profile in &bundle.profile {
        if !is_id(&profile.id) || !seen.insert(profile.id.as_str()) {
            broken.push(format!(
                "profile {:?} is not an id or is declared twice",
                profile.id
            ));
        }
    }
    let profiles = seen;
    let mut seen = BTreeSet::new();
    for form in &bundle.form {
        if !is_id(&form.id) || !seen.insert(form.id.as_str()) {
            broken.push(format!(
                "form {:?} is not an id or is declared twice",
                form.id
            ));
        }
        for profile in form
            .profiles
            .iter()
            .filter(|one| !profiles.contains(one.as_str()))
        {
            broken.push(format!(
                "form {} names profile {profile}, which is not declared",
                form.id
            ));
        }
    }
    let services: BTreeMap<&str, &Declared> = parts
        .values()
        .flat_map(|part| part.services.iter())
        .map(|one| (one.id.as_str(), one))
        .collect();
    for removed in &bundle.removed {
        if parts.contains_key(removed.id.as_str()) {
            broken.push(format!("removed {} is a pinned plugin", removed.id));
        }
        if let Some(by) = &removed.replaced_by {
            if !parts.contains_key(by.as_str()) {
                broken.push(format!(
                    "removed {} is replaced by {by}, which is not a pinned plugin",
                    removed.id
                ));
            }
        }
    }
    for wiring in &bundle.wiring {
        wired(wiring, parts, &services, broken);
    }
}

/// Every rule one bundle wiring breaks: it is a choice of filler or a link by name, and
/// names only the pinned parts' services.
fn wired(
    wiring: &lemonfiber_manifest::Wiring,
    parts: &BTreeMap<&str, Part>,
    services: &BTreeMap<&str, &Declared>,
    broken: &mut Vec<String>,
) {
    let at = format!("wiring by {}", wiring.by);
    if !services.contains_key(wiring.by.as_str()) {
        broken.push(format!(
            "{at} names {}, which no pinned plugin declares",
            wiring.by
        ));
    }
    if wiring
        .why
        .as_deref()
        .is_none_or(|why| why.trim().is_empty())
    {
        broken.push(format!("{at} gives no why"));
    }
    if wiring.each {
        broken.push(format!(
            "{at} declares each, which only a plugin's own [[ask]] may"
        ));
    }
    match (&wiring.asks, &wiring.filled_by, &wiring.to) {
        (Some(capability), Some(filler), None) => {
            let asked = parts.values().any(|part| {
                part.asks.iter().any(|ask| {
                    ask.capability == *capability
                        && !ask.each
                        && asker(part, ask).is_some_and(|by| by == wiring.by)
                })
            });
            if !asked {
                broken.push(format!(
                    "{at} chooses for {capability}, which {} does not ask once in its [[ask]]",
                    wiring.by
                ));
            }
            let fills = services.get(filler.as_str()).is_some_and(|one| {
                one.provides.iter().any(|named| named == capability)
                    || one
                        .speaks
                        .iter()
                        .any(|spoken| spoken.split('@').next() == Some(capability.as_str()))
            });
            if !fills {
                broken.push(format!(
                    "{at} chooses {filler} for {capability}, which no pinned service of that id \
                     provides or speaks"
                ));
            }
        }
        (None, None, Some(to)) => {
            if !services.contains_key(to.as_str()) {
                broken.push(format!("{at} names {to}, which no pinned plugin declares"));
            }
        }
        _ => broken.push(format!(
            "{at} is neither a choice of filler (asks, filled_by, why) nor a link by name (to, why)"
        )),
    }
}

/// The service that makes `ask`: the one it names, or the part's one service.
fn asker<'a>(part: &'a Part, ask: &'a Asked) -> Option<&'a str> {
    match &ask.service {
        Some(named) => Some(named.as_str()),
        None if part.services.len() == 1 => part.services.first().map(|one| one.id.as_str()),
        None => None,
    }
}

/// What the bundle reads of a manifest's text; nothing where it is not TOML, which the
/// pin's digest and id have already refused.
pub(crate) fn declared(manifest: &str) -> Part {
    let Ok(table) = toml::from_str::<toml::Table>(manifest) else {
        return Part::default();
    };
    let text = |value: &Value, key: &str| value.get(key).and_then(Value::as_str).map(str::to_owned);
    let words = |value: Option<&Value>| -> Vec<String> {
        value
            .and_then(Value::as_array)
            .map(|list| {
                list.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default()
    };
    let entries = |key: &str| -> Vec<Value> {
        table
            .get(key)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    };
    let services = entries("service")
        .iter()
        .map(|one| Declared {
            id: text(one, "id").unwrap_or_default(),
            provides: words(one.get("provides")),
            speaks: words(one.get("speaks")),
            replaces: text(one, "replaces"),
            reaches: text(one, "reaches"),
            asks_for: text(one, "asks_for"),
        })
        .collect();
    let asks = entries("ask")
        .iter()
        .map(|one| Asked {
            capability: text(one, "capability").unwrap_or_default(),
            service: text(one, "service"),
            each: one.get("each").and_then(Value::as_bool).unwrap_or(false),
        })
        .collect();
    let mut fixtures = Vec::new();
    fixtured(&Value::Table(table.clone()), &mut fixtures);
    Part {
        services,
        asks,
        forms: words(table.get("plugin").and_then(|plugin| plugin.get("forms"))),
        fixtures,
    }
}

/// Every `fixture` value anywhere in `value`.
fn fixtured(value: &Value, found: &mut Vec<String>) {
    match value {
        Value::Table(table) => {
            for (key, inner) in table {
                match (key.as_str(), inner) {
                    ("fixture", Value::String(path)) => found.push(path.clone()),
                    _ => fixtured(inner, found),
                }
            }
        }
        Value::Array(list) => list.iter().for_each(|inner| fixtured(inner, found)),
        _ => {}
    }
}
