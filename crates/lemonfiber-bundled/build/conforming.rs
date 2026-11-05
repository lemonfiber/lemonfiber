//! Every plugin the bundle pins, held to the recordings its copy ships before the bundle
//! is compiled in: a case of a contract it speaks that fails, or that no recording
//! answers, refuses the build, naming the plugin, the contract and the case.

use std::path::Path;

use lemonfiber_conformance::{judged_now, Speaker, Verdict};

/// Each case of each of `plugins`' copies beneath `copies` that does not pass, as
/// `plugin contract/case — verdict`; nothing where every one passes.
pub fn unpassed<'a>(copies: &Path, plugins: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let published = lemonfiber_contract::capabilities::all();
    plugins
        .into_iter()
        .flat_map(|plugin| {
            let copy = copies.join(plugin);
            match speakers(&copy).and_then(|speakers| {
                judged_now(&copy, &speakers, &published).map_err(|unmade| unmade.to_string())
            }) {
                Err(why) => vec![format!("{plugin}: {why}")],
                Ok(cases) => cases
                    .iter()
                    .filter(|judged| judged.verdict != Verdict::Passed)
                    .map(|judged| format!("{plugin} {judged}"))
                    .collect(),
            }
        })
        .collect()
}

/// Each service of the copy at `copy` that speaks a contract: what it speaks, and what
/// the upstream it answers for, the service it fronts or itself, is pinned at and files.
fn speakers(copy: &Path) -> Result<Vec<Speaker>, String> {
    let at = copy.join(lemonfiber_bundle::MANIFEST);
    let text = std::fs::read_to_string(&at)
        .map_err(|unread| format!("{} could not be read: {unread}", at.display()))?;
    let manifest: toml::Table = toml::from_str(&text)
        .map_err(|unread| format!("{} does not read: {unread}", at.display()))?;
    let services: Vec<&toml::Table> = manifest
        .get("service")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(toml::Value::as_table)
        .collect();
    let text_of = |table: &toml::Table, key: &str| {
        table
            .get(key)
            .and_then(toml::Value::as_str)
            .map(str::to_owned)
    };
    let words = |table: &toml::Table, key: &str| -> Vec<String> {
        table
            .get(key)
            .and_then(toml::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(toml::Value::as_str)
            .map(str::to_owned)
            .collect()
    };
    Ok(services
        .iter()
        .filter(|service| !words(service, "speaks").is_empty())
        .map(|service| {
            let upstream = match text_of(service, "fronts") {
                None => Some(*service),
                Some(fronted) => services
                    .iter()
                    .copied()
                    .find(|one| text_of(one, "id").as_deref() == Some(fronted.as_str())),
            };
            Speaker {
                service: text_of(service, "id").unwrap_or_default(),
                speaks: words(service, "speaks"),
                pinned: upstream.and_then(|one| {
                    Some(format!(
                        "{}@{}",
                        text_of(one, "image")?,
                        text_of(one, "digest")?
                    ))
                }),
                files: upstream
                    .map(|one| words(one, "media_types"))
                    .unwrap_or_default(),
            }
        })
        .collect())
}
