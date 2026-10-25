//! A service's own key, read from the file it wrote it into, by the shape of that file.
//!
//! One reader per shape a stack service's declaration may name as its `key_source`, each
//! taking the key from where the service writing that shape keeps its own.

use lemonfiber_manifest::KeySource;

/// The reader that takes a service's own key out of a file of `source`'s shape, where
/// keys are read from a file of that shape.
pub(crate) fn reader(source: KeySource) -> Option<fn(&str) -> Option<String>> {
    match source {
        KeySource::ConfigXml => Some(crate::servarr::api_key),
        KeySource::ConfigJson => Some(crate::seerr::api_key),
        KeySource::ConfigYaml => Some(yaml_key),
        KeySource::ConfigIni => Some(ini_key),
        KeySource::ApiSettings | KeySource::Generated | KeySource::None => None,
    }
}

/// The section of a YAML configuration the service keeps its own key in.
const YAML_SECTION: &str = "auth";

/// The entry of that section holding it.
const YAML_ENTRY: &str = "apikey";

/// The entry of an INI configuration holding the service's own key.
const INI_ENTRY: &str = "api_key";

/// The key under `apikey` in the `auth` section of a YAML configuration.
///
/// Only that section's: the file holds an `apikey` under each service it has been
/// pointed at too. A key in the first column opens a section. Scanned rather than parsed,
/// since nothing else in the file is wanted.
fn yaml_key(config_yaml: &str) -> Option<String> {
    let mut section = None;
    for line in config_yaml.lines() {
        if let Some(opened) = section_of(line) {
            section = Some(opened);
            continue;
        }
        if section != Some(YAML_SECTION) {
            continue;
        }
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        if name.trim() == YAML_ENTRY {
            return crate::generated::key(value.trim().trim_matches('\'').trim_matches('"'));
        }
    }
    None
}

/// The section `line` opens, where it is a key in the first column.
fn section_of(line: &str) -> Option<&str> {
    let first = line.chars().next()?;
    if first.is_whitespace() || first == '#' {
        return None;
    }
    Some(line.split_once(':')?.0.trim())
}

/// The key the `api_key` entry of an INI configuration sets.
///
/// Matched by its exact name, so a neighbouring `nzb_key` or a commented line is not
/// read as it.
fn ini_key(config_ini: &str) -> Option<String> {
    config_ini.lines().find_map(|line| {
        let (name, value) = line.split_once('=')?;
        (name.trim() == INI_ENTRY)
            .then(|| crate::generated::key(value.trim()))
            .flatten()
    })
}

#[cfg(test)]
mod tests;
