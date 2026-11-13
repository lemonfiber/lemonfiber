//! What a plugin author asks of a plugin's own source, answered with no stack.

use std::path::{Path, PathBuf};

use crate::error::codes::plugin::KEY_UNREADABLE;
use crate::error::{Problem, Remedy, State};
use crate::plugin::{Key, Recorder, Retaking};

use super::{Ctx, Outcome};

/// What an author asks of the plugin whose source is at `path`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Asked {
    /// What the source claims, held to what this build publishes.
    Claims {
        /// The plugin's directory, or the `plugin.toml` inside it.
        path: PathBuf,
    },
    /// What its conformance recordings come to against the contracts it speaks.
    Conform {
        /// The plugin's directory, or the `plugin.toml` inside it.
        path: PathBuf,
    },
    /// What anybody has said about the images it pins, verified against `keys`.
    Provenance {
        /// The plugin's directory, or the `plugin.toml` inside it.
        path: PathBuf,
        /// The PEM public keys a signature is verified against.
        keys: Vec<PathBuf>,
    },
    /// Each run proof, run and recorded.
    Record {
        /// The plugin's directory, or the `plugin.toml` inside it.
        path: PathBuf,
        /// Whether what is taken is written or held to what is there.
        taking: Retaking,
    },
}

/// Answer what an author asked.
///
/// # Errors
///
/// The [`Problem`] where there is no plugin at the path this build can read, or a key
/// named could not be read.
pub(crate) async fn answered(ctx: &Ctx, asked: Asked) -> Result<Outcome, Box<Problem>> {
    match asked {
        Asked::Claims { path } => crate::plugin::claimed(&path)
            .map(Outcome::PluginClaims)
            .map_err(unreadable),
        Asked::Conform { path } => crate::plugin::conformed(&path)
            .await
            .map(Outcome::PluginConformance)
            .map_err(unreadable),
        Asked::Provenance { path, keys } => vouched(ctx, &path, &keys).await,
        Asked::Record { path, taking } => recorded(ctx, &path, &taking).await,
    }
}

/// What anybody has said about the images the plugin at `path` pins.
async fn vouched(ctx: &Ctx, path: &Path, keys: &[PathBuf]) -> Result<Outcome, Box<Problem>> {
    let manifest = crate::plugin::read(path).map_err(unreadable)?;
    let held = held_keys(ctx, keys).await?;
    let read = crate::plugin::vouched(&manifest, &held, ctx.seams.registry.as_ref()).await;
    Ok(Outcome::PluginProvenance(read))
}

/// Each run proof of the plugin at `path`, run and recorded.
async fn recorded(ctx: &Ctx, path: &Path, taking: &Retaking) -> Result<Outcome, Box<Problem>> {
    let fresh = || ctx.seams.sessions.opened();
    let recorder = Recorder {
        program: ctx.seams.program.as_ref(),
        serving: ctx.seams.serving.as_ref(),
        transport: &fresh,
        random: ctx.seams.random.as_ref(),
        taking,
        confined: ctx.seams.confined.as_ref(),
        patience: ctx.patience,
    };
    crate::plugin::record(path, recorder)
        .await
        .map(Outcome::PluginRecording)
        .map_err(unreadable)
}

/// Nothing at the path the author named is a plugin this build can read.
fn unreadable(why: crate::plugin::Unreadable) -> Box<Problem> {
    Box::new(super::plugins::unreadable_source(&why))
}

/// The keys an author named, read from the paths they gave.
async fn held_keys(ctx: &Ctx, paths: &[PathBuf]) -> Result<Vec<Key>, Box<Problem>> {
    let mut held = Vec::new();
    for path in paths {
        let named = path.display().to_string();
        let Some(text) = ctx.seams.filesystem.read(path).await else {
            return Err(Box::new(key_unreadable(&named, "it could not be read")));
        };
        let key = Key::from_pem(&named, &text)
            .map_err(|unusable| Box::new(key_unreadable(&named, &unusable.why)))?;
        held.push(key);
    }
    Ok(held)
}

/// A key the author named that could not be used, and why.
fn key_unreadable(named: &str, why: &str) -> Problem {
    Problem::new(
        KEY_UNREADABLE,
        format!("The key at {named} could not be used"),
        "Nothing was verified: an answer checked against fewer keys than were named would \
         call an image somebody signed unproven.",
        Remedy::new("Name a PEM public key on the P-256 curve with --key, or leave the key out"),
    )
    .in_state(State::Guided)
    .with_detail(why.to_owned())
}

#[cfg(test)]
mod tests;
