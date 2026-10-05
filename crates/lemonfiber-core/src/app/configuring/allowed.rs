//! Which settings may be changed by name, and to what.
//!
//! A setting changed here is handed to Compose with the rest of the file, and some of
//! them reach the host: where the containers' data is mounted from, which user they run
//! as, which further Compose file is layered over the stack. So a change is refused
//! before it is weighed where nothing reads a setting by that name — a name nobody reads
//! is a name only Compose or the engine would act on — and where a setting that reaches
//! the host would hand the containers the machine rather than a directory of their own.

use std::path::{Component, Path};

use crate::config::{env::EnvFile, DATA_ROOT_KEY, OVERLAY_KEY, PGID_KEY, PUID_KEY, SETTINGS};
use crate::stack::Source;

use super::Ctx;

/// The file a stack declares the settings it reads in.
const SETTINGS_FILE: &str = ".env.example";

/// Why `key` may not be set to `value`, where it may not.
pub(super) async fn refusal(ctx: &Ctx, held: &EnvFile, key: &str, value: &str) -> Option<String> {
    if !known(ctx.stack, held, key).await {
        return Some(format!(
            "neither lemonfiber nor the stack reads a setting called {key}, so it is not written"
        ));
    }
    if value.is_empty() {
        return None;
    }
    match key {
        DATA_ROOT_KEY => super::data_root::refusal(ctx, value).await,
        PUID_KEY | PGID_KEY => identity(key, value),
        OVERLAY_KEY => overlay(ctx, value),
        _ => None,
    }
}

/// Whether anything reads `key`: lemonfiber itself, the stack's own declaration of what
/// it reads, or the file already, where seeding or a plugin put it.
///
/// The stack's declaration is read only where neither of the others answers.
async fn known(stack: Source, held: &EnvFile, key: &str) -> bool {
    SETTINGS.contains(&key)
        || held.get(key).is_some()
        || declared(stack).await.iter().any(|one| one == key)
}

/// Every setting the stack declares it reads.
async fn declared(stack: Source) -> Vec<String> {
    let text = match stack {
        Source::Embedded(dir) => dir
            .get_file(SETTINGS_FILE)
            .and_then(include_dir::File::contents_utf8)
            .map(str::to_owned),
        Source::External(path) => tokio::fs::read_to_string(path.join(SETTINGS_FILE))
            .await
            .ok(),
    };
    text.map(|text| {
        EnvFile::parse(&text)
            .keys()
            .into_iter()
            .map(str::to_owned)
            .collect()
    })
    .unwrap_or_default()
}

/// Why `value` will not do as the user or group the services run as: it is not a number,
/// or it is root's.
fn identity(key: &str, value: &str) -> Option<String> {
    match value.parse::<u32>() {
        Ok(id) if id != 0 => None,
        _ => Some(format!(
            "{key} has to be the number of an ordinary user or group: 0 is root, and the \
             services would run with root's rights over everything they are given"
        )),
    }
}

/// Why `value` will not do as the Compose file layered over the stack: it lies outside
/// the directories lemonfiber keeps its own files in.
fn overlay(ctx: &Ctx, value: &str) -> Option<String> {
    let path = Path::new(value);
    let climbs = path.components().any(|part| part == Component::ParentDir);
    let ours = crate::app::targets::layout(ctx).is_some_and(|paths| {
        path.starts_with(paths.config_dir()) || path.starts_with(paths.data_dir())
    });
    (climbs || !ours).then(|| {
        format!(
            "{value} is outside lemonfiber's own directories, and a Compose file layered over \
             the stack is kept beside the settings it belongs to"
        )
    })
}

#[cfg(test)]
mod tests;
