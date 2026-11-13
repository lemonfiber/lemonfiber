//! The environment file Compose reads beside the project, holding only what plain Compose
//! needs to run there as lemonfiber does: the profiles to start, those of the stack's last
//! start, which hold the services of the first-party plugins too, and the own profile of
//! every other installed plugin joining a form that start ran, and the files to read, the
//! project and the operator's overlay after it where they wrote one.
//!
//! lemonfiber names its own environment file, its files and its profiles on every
//! invocation, so Compose never reads this one for lemonfiber. It is what `docker compose
//! up` in the project directory reads, with no lemonfiber running, to start what
//! lemonfiber last started. It is written as the project is: where it is lemonfiber's,
//! and kept where somebody edited it.

use std::collections::BTreeSet;
use std::path::PathBuf;

use super::super::Ctx;
use crate::stack::Failure;

/// The file, beneath the project directory.
pub(crate) const FILE: &str = ".env";

/// The variable Compose reads the profiles to start from.
const KEY: &str = "COMPOSE_PROFILES";

/// The variable Compose reads the files to merge from, in order.
const FILES: &str = "COMPOSE_FILE";

/// Write the profiles of a start of `started` beside the project `text`.
///
/// # Errors
///
/// Where the file cannot be read or written.
pub(crate) fn started(ctx: &Ctx, started: &BTreeSet<String>, text: &str) -> Result<(), Failure> {
    written(ctx, started, text)
}

/// Write the stack's profiles of the last start beside the project `text`, with the own
/// profile of every installed plugin joining a form that start ran.
///
/// # Errors
///
/// Where the file cannot be read or written.
pub(crate) fn kept(ctx: &Ctx, text: &str) -> Result<(), Failure> {
    let last = super::held_as(ctx, FILE)
        .map(|held| read(&held))
        .unwrap_or_default();
    let stack = last
        .into_iter()
        .filter(|profile| !crate::plugin::is_a_plugins(profile))
        .chain(joining(ctx))
        .collect();
    written(ctx, &stack, text)
}

/// The own profile of every installed plugin joining a form the last start ran, none where
/// the stack was stopped since.
fn joining(ctx: &Ctx) -> Vec<String> {
    let returning = crate::autostart::run::load(ctx);
    let (Some(manifest), Ok(register)) = (
        (!returning.halted())
            .then(|| ctx.stack.checked_manifest(ctx.today()).ok())
            .flatten(),
        crate::app::plugins::read(ctx),
    ) else {
        return Vec::new();
    };
    crate::plugin::joining(register.installed())
        .into_iter()
        .filter(|plugin| plugin.runs_with(returning.last_run(), &manifest))
        .map(|plugin| plugin.profile)
        .collect()
}

/// Write each profile of `stack` that `text` declares or that is no plugin's.
fn written(ctx: &Ctx, stack: &BTreeSet<String>, text: &str) -> Result<(), Failure> {
    let declared = crate::project::document::profiles(text);
    let profiles: Vec<String> = stack
        .iter()
        .filter(|profile| !crate::plugin::is_a_plugins(profile) || declared.contains(*profile))
        .cloned()
        .collect();
    let read = std::iter::once(PathBuf::from(crate::project::FILE)).chain(
        ctx.settings
            .overlays
            .iter()
            .filter(|overlay| overlay.is_file())
            .cloned(),
    );
    let read = std::env::join_paths(read).map_or_else(
        |_| crate::project::FILE.to_owned(),
        |joined| joined.to_string_lossy().into_owned(),
    );
    let text = format!("{KEY}={}\n{FILES}={read}\n", profiles.join(","));
    super::materialised_as(ctx, FILE, &text, super::Pass::Write).map(|_| ())
}

/// The profiles the file `text` names.
fn read(text: &str) -> BTreeSet<String> {
    text.lines()
        .filter_map(|line| line.strip_prefix(KEY)?.strip_prefix('='))
        .flat_map(|named| named.split(','))
        .map(str::trim)
        .filter(|profile| !profile.is_empty())
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
mod tests;
