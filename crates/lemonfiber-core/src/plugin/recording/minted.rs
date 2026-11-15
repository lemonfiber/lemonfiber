//! The value each secret a recording's containers are given holds for the run: the one
//! already written for it, its stand-in, or one minted now in its declared shape, so
//! every file, environment and adapter naming it holds the same.

use std::collections::BTreeMap;
use std::path::Path;

use lemonfiber_manifest::container::Value;

use super::stood_in::StandIns;
use crate::plugin::installed::Placed;
use crate::plugin::seeding::{shapes, Shapes, UNMINTED};
use crate::ports::random::Random;

/// What a run mints with: the randomness, and the shape each secret is declared in.
pub(super) struct Minting<'a> {
    /// Where minted values come from.
    pub(super) random: &'a dyn Random,
    /// The plugin and the shape each secret it declares is minted in.
    pub(super) shapes: Shapes,
}

impl<'a> Minting<'a> {
    /// Minting from `random` in the shapes the plugin at `root` declares.
    pub(super) fn at(root: &Path, random: &'a dyn Random) -> Self {
        Self {
            random,
            shapes: shapes(root),
        }
    }
}

/// Each secret `written` holds a value for, then each `stand_ins` stands in for that
/// nothing wrote, with its value.
pub(super) fn known<'a>(
    written: impl Iterator<Item = (&'a str, &'a str)>,
    stand_ins: &StandIns,
) -> BTreeMap<String, String> {
    let mut known: BTreeMap<String, String> = written
        .map(|(secret, value)| (secret.to_owned(), value.to_owned()))
        .collect();
    for (secret, value) in stand_ins.held() {
        known
            .entry(secret.to_owned())
            .or_insert_with(|| value.to_owned());
    }
    known
}

/// The value of `secret` for this run: the one already known for it, or one minted now
/// in its declared shape, kept in `secrets` and in `made`.
pub(super) fn valued(
    minting: &Minting<'_>,
    secrets: &mut BTreeMap<String, String>,
    secret: &str,
    made: &mut Vec<String>,
) -> Result<String, String> {
    if let Some(held) = secrets.get(secret) {
        return Ok(held.clone());
    }
    let fresh = minting
        .shapes
        .minted(minting.random, secret)
        .ok_or_else(|| UNMINTED.to_owned())?;
    made.push(fresh.clone());
    secrets.insert(secret.to_owned(), fresh.clone());
    Ok(fresh)
}

/// Each variable of `placed`'s environment that names a secret, as `NAME=value`.
pub(super) fn environment(
    minting: &Minting<'_>,
    secrets: &mut BTreeMap<String, String>,
    placed: &Placed,
    made: &mut Vec<String>,
) -> Result<Vec<String>, String> {
    let mut env = Vec::new();
    for (name, value) in &placed.fields.env {
        if let Value::Secret(of) = value {
            env.push(format!(
                "{name}={}",
                valued(minting, secrets, &of.secret, made)?
            ));
        }
    }
    Ok(env)
}
