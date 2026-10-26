mod changing;
pub(crate) mod fences;
pub(crate) mod files;
pub(crate) mod filing;
pub(crate) mod kept;
pub(crate) mod legacy;
mod listing;
pub(crate) mod own;
pub(crate) mod readers;
mod recorded;
mod resolved;
mod restarting;
pub(crate) mod run;
mod secrets;
pub(crate) mod store;
pub(crate) mod undoing;

pub(crate) use files::{text, written};
pub(crate) use resolved::{holding, Resolved, OPERATOR_KEYS};
pub(crate) use restarting::{restarted, running};
pub(crate) use secrets::{filed, kept, Keeping, Secrets, Source};
pub(crate) use store::Settings;

use crate::app::Ctx;
use crate::config::paths::{SECRETS, SETTINGS};
use crate::plugin::Installed;

pub(crate) const OPERATION: &str = "settings";

/// Every refusal a change to a setting answers with before it is weighed.
pub const REFUSALS: [crate::error::Code; 5] = [
    crate::error::codes::setting::NO_SUCH_SETTING,
    crate::error::codes::setting::A_SECRET,
    crate::error::codes::setting::TAKEN_ELSEWHERE,
    crate::error::codes::setting::UNWRITABLE_VALUE,
    crate::error::codes::setting::ALREADY_HOLDS,
];

pub(crate) fn named(namespace: &str, id: &str) -> String {
    format!(
        "{namespace}{}{id}",
        lemonfiber_manifest::container::NAMESPACED
    )
}

pub(crate) fn is_named(name: &str) -> bool {
    name.contains(lemonfiber_manifest::container::NAMESPACED)
}

pub(crate) fn is_stored(name: &str) -> bool {
    is_named(name)
        || own::is_core(name)
        || lemonfiber_manifest::container::PUBLISHED.contains(&name)
}

pub(crate) fn in_namespace(name: &str, namespace: &str) -> bool {
    name.split_once(lemonfiber_manifest::container::NAMESPACED)
        .is_some_and(|(space, _)| space == namespace)
}

pub(crate) fn secrets_at(ctx: &Ctx) -> Option<std::path::PathBuf> {
    ctx.settings
        .env_file
        .as_deref()
        .map(|env| env.with_file_name(SECRETS))
}

/// Where the dashboard's search box sends a search: the address recorded, or the default
/// where none is.
///
/// # Errors
///
/// Why the address recorded is not one, naming the setting.
pub(crate) fn dashboard_search(ctx: &Ctx) -> Result<String, String> {
    let recorded = recorded::Recorded::of(ctx);
    let Some(value) = recorded.value(own::DASHBOARD_SEARCH) else {
        return Ok(own::DEFAULT_SEARCH.to_owned());
    };
    own::refusal(own::DASHBOARD_SEARCH, value).map_or_else(|| Ok(value.to_owned()), Err)
}

pub(crate) fn forgotten(ctx: &Ctx, plugin: &Installed) {
    files::forgotten(ctx, plugin);
    let Some(env) = ctx.settings.env_file.as_deref() else {
        return;
    };
    let (secrets, settings) = (env.with_file_name(SECRETS), env.with_file_name(SETTINGS));
    if secrets.is_file() {
        let _ = secrets::forgotten(&secrets, &plugin.plugin);
    }
    if settings.is_file() {
        let _ = store::forgotten(&settings, &plugin.plugin);
    }
}

#[cfg(test)]
mod tests;

/// Every name of the operator's environment file something reads: each the project is
/// written with, and each filed into the settings and secret stores under the name a
/// service reads.
pub(crate) fn read_by_the_stack() -> impl Iterator<Item = &'static str> {
    OPERATOR_KEYS
        .into_iter()
        .chain(legacy::TABLE.iter().map(|row| row.key))
}
