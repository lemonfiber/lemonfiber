//! A service's own key, read from the file it wrote it into, by the shape of that file.
//!
//! One reader per shape a stack service's declaration may name as its `key_source`, each
//! taking the key from where the service writing that shape keeps its own.

use lemonfiber_manifest::container::Format;
use lemonfiber_manifest::KeySource;

/// Takes a key out of the contents of a file of one shape, where it is written there.
pub(crate) type Reader = fn(&str) -> Option<String>;

/// The reader that takes a service's own key out of a file of `source`'s shape, where
/// keys are read from a file of that shape.
pub(crate) fn reader(source: KeySource) -> Option<Reader> {
    match source {
        KeySource::ConfigXml => Some(xml_key),
        KeySource::ConfigJson => Some(json_key),
        KeySource::ConfigYaml => Some(yaml_key),
        KeySource::ConfigIni => Some(ini_key),
        KeySource::ApiSettings | KeySource::Generated | KeySource::None => None,
    }
}

/// Where a servarr-shaped XML configuration keeps the service's own key.
const XML_KEY: &str = "Config.ApiKey";

/// Where a JSON configuration of the request service's shape keeps it.
const JSON_KEY: &str = "main.apiKey";

/// Where a YAML configuration of the subtitle finder's shape keeps it.
const YAML_KEY: &str = "auth.apikey";

/// Where an INI configuration of the Usenet client's shape keeps it.
const INI_KEY: &str = "misc.api_key";

/// The key in the `<ApiKey>` element of an XML configuration.
fn xml_key(text: &str) -> Option<String> {
    at(Format::Xml, XML_KEY, text)
}

/// The key under `apiKey` in the `main` object of a JSON configuration.
fn json_key(text: &str) -> Option<String> {
    at(Format::Json, JSON_KEY, text)
}

/// The key under `apikey` in the `auth` section of a YAML configuration.
fn yaml_key(text: &str) -> Option<String> {
    at(Format::Yaml, YAML_KEY, text)
}

/// The key the `api_key` entry of an INI configuration's `misc` section sets.
fn ini_key(text: &str) -> Option<String> {
    at(Format::Ini, INI_KEY, text)
}

/// The key at the dotted path `key` of `text`, a file written in `format`, where it holds
/// one spelled as a key.
pub(crate) fn at(format: Format, key: &str, text: &str) -> Option<String> {
    let path: Vec<&str> = key.split('.').collect();
    if path.iter().any(|segment| segment.is_empty()) {
        return None;
    }
    let found = match format {
        Format::Xml => in_xml(&path, text),
        Format::Json => in_json(&path, text),
        Format::Yaml => in_yaml(&path, text),
        Format::Ini => in_ini(&path, text),
    }?;
    crate::generated::key(&found)
}

/// The text of the element `path` names, each segment an element inside the one before.
///
/// Scanned rather than parsed: the file is machine-written and nothing else in it is
/// wanted.
fn in_xml(path: &[&str], text: &str) -> Option<String> {
    let mut within = text;
    for name in path {
        let opened = within.find(&format!("<{name}>"))? + name.len() + 2;
        let rest = within.get(opened..)?;
        within = rest.get(..rest.find(&format!("</{name}>"))?)?;
    }
    Some(within.trim().to_owned())
}

/// The string `path` names, each segment an object's entry inside the one before.
fn in_json(path: &[&str], text: &str) -> Option<String> {
    let root: serde_json::Value = serde_json::from_str(text).ok()?;
    let mut value = &root;
    for name in path {
        value = value.get(name)?;
    }
    value.as_str().map(str::to_owned)
}

/// The scalar `path` names, each segment a mapping's key indented beneath the one before.
///
/// Scanned rather than parsed: a line that is a list item, a comment or no mapping entry
/// is passed over, and an entry is matched only where every mapping holding it is the
/// path's.
fn in_yaml(path: &[&str], text: &str) -> Option<String> {
    let mut open: Vec<(usize, &str)> = Vec::new();
    for line in text.lines() {
        let body = line.trim_start();
        if body.is_empty() || body.starts_with('#') || body.starts_with('-') {
            continue;
        }
        let Some((name, value)) = body.split_once(':') else {
            continue;
        };
        let indent = line.len() - body.len();
        while open.last().is_some_and(|(held, _)| *held >= indent) {
            open.pop();
        }
        let depth = open.len();
        let along = depth < path.len()
            && open.iter().zip(path).all(|((_, held), want)| held == want)
            && path.get(depth) == Some(&name.trim());
        let value = value.trim();
        if along && depth + 1 == path.len() {
            return Some(value.trim_matches('\'').trim_matches('"').to_owned());
        }
        if value.is_empty() {
            open.push((indent, name.trim()));
        }
    }
    None
}

/// The value of the entry `path` names: its section, then the entry within it.
///
/// Matched by exact names, so a neighbouring entry, another section's or a commented line
/// is not read as it.
fn in_ini(path: &[&str], text: &str) -> Option<String> {
    let [section, entry] = path else {
        return None;
    };
    let mut current = None;
    for line in text.lines().map(str::trim) {
        if let Some(opened) = line
            .strip_prefix('[')
            .and_then(|rest| rest.strip_suffix(']'))
        {
            current = Some(opened.trim());
            continue;
        }
        let Some((name, value)) = line.split_once('=') else {
            continue;
        };
        if current == Some(*section) && name.trim() == *entry {
            return Some(value.trim().to_owned());
        }
    }
    None
}

#[cfg(test)]
mod tests;
