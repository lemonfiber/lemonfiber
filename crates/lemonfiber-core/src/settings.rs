mod files;
pub(crate) mod filing;
mod kept;
mod resolved;
mod secrets;
mod store;
pub(crate) mod undoing;

pub(crate) use files::{text, written};
pub(crate) use resolved::{holding, Resolved};
pub(crate) use secrets::{kept, Keeping, Secrets, Source};
pub(crate) use store::Settings;

use crate::app::Ctx;
use crate::config::paths::{SECRETS, SETTINGS};
use crate::plugin::Installed;

pub(crate) const OPERATION: &str = "settings";

pub(crate) fn named(namespace: &str, id: &str) -> String {
    format!(
        "{namespace}{}{id}",
        lemonfiber_manifest::container::NAMESPACED
    )
}

pub(crate) fn is_named(name: &str) -> bool {
    name.contains(lemonfiber_manifest::container::NAMESPACED)
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
