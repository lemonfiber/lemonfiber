//! What both containers of a recording are given before either starts: the throwaway
//! copies of their starting files, the adapter's key and its upstream's credentials, each
//! secret one value wherever it is written.

use std::collections::BTreeMap;
use std::path::Path;

use lemonfiber_manifest::container::Value;

use lemonfiber_contract::adapter::KEY_FILE;

use super::{refused, Faulted, Fronting, Placed, Recorder, Starting, Throwaway};
use crate::ports::random::Random;

/// What both containers are given before either starts.
pub(super) struct Prepared {
    /// The throwaway copy of the upstream's starting files.
    pub(super) copy: Throwaway,
    /// Its throwaway library, where it takes one.
    pub(super) data: Option<std::path::PathBuf>,
    /// The throwaway copy of the adapter's starting files, which holds its key.
    pub(super) held: Throwaway,
    /// The key lemonfiber minted for the adapter.
    pub(super) key: String,
    /// Every credential minted for the run so far, which every answer is scrubbed of.
    pub(super) made: Vec<String>,
    /// Each variable of the upstream's environment that names a secret, as `NAME=value`.
    pub(super) env: Vec<String>,
}

/// The value of `secret` for this run: the one already written for it, or one minted now
/// and kept, so the upstream's files, its environment and the adapter all hold the same.
fn valued(
    random: &dyn Random,
    secrets: &mut BTreeMap<String, String>,
    secret: &str,
    made: &mut Vec<String>,
) -> Result<String, String> {
    if let Some(held) = secrets.get(secret) {
        return Ok(held.clone());
    }
    let fresh = crate::plugin::seeding::mint(random)
        .ok_or_else(|| crate::plugin::seeding::UNMINTED.to_owned())?;
    made.push(fresh.clone());
    secrets.insert(secret.to_owned(), fresh.clone());
    Ok(fresh)
}

/// Each variable of `upstream`'s environment that names a secret, as `NAME=value`.
fn environment(
    random: &dyn Random,
    secrets: &mut BTreeMap<String, String>,
    upstream: &Placed,
    made: &mut Vec<String>,
) -> Result<Vec<String>, String> {
    let mut env = Vec::new();
    for (name, value) in &upstream.fields.env {
        if let Value::Secret(of) = value {
            env.push(format!(
                "{name}={}",
                valued(random, secrets, &of.secret, made)?
            ));
        }
    }
    Ok(env)
}

/// Each field of the adapter's `upstream`, holding the value its upstream was given.
fn handed(
    random: &dyn Random,
    secrets: &mut BTreeMap<String, String>,
    adapter: &Placed,
    made: &mut Vec<String>,
) -> Result<Vec<(String, String)>, String> {
    let mut handed = Vec::new();
    for (field, secret) in &adapter.upstream {
        handed.push((field.clone(), valued(random, secrets, secret, made)?));
    }
    Ok(handed)
}

impl Prepared {
    /// Both copies and the adapter's key, or why one could not be made.
    pub(super) fn of(
        recorder: Recorder<'_>,
        root: &Path,
        upstream: &Placed,
        adapter: Fronting<'_>,
    ) -> Result<Self, Faulted> {
        let copy = Throwaway::of(
            Some(Starting {
                root,
                service: &upstream.service,
            }),
            recorder.random,
        )
        .map_err(refused("starting files"))?;
        let data = super::super::served::library(upstream, &copy).map_err(refused("data"))?;
        let held = Throwaway::of(
            Some(Starting {
                root,
                service: &adapter.placed.service,
            }),
            recorder.random,
        )
        .map_err(refused("adapter's starting files"))?;
        let key = crate::secret::generate(recorder.random).ok_or_else(|| {
            refused("adapter's key")(crate::secret::NO_RANDOMNESS_FOR_PASSWORD.to_owned())
        })?;
        held.hold(KEY_FILE, &key)
            .map_err(refused("adapter's key"))?;
        let mut made: Vec<String> = std::iter::once(key.clone())
            .chain(copy.made().map(str::to_owned))
            .chain(held.made().map(str::to_owned))
            .collect();
        let mut secrets: BTreeMap<String, String> = held
            .written()
            .chain(copy.written())
            .map(|(secret, value)| (secret.to_owned(), value.to_owned()))
            .collect();
        let env = environment(recorder.random, &mut secrets, upstream, &mut made)
            .map_err(refused("upstream's environment"))?;
        let handed = handed(recorder.random, &mut secrets, adapter.placed, &mut made)
            .map_err(refused("adapter's upstream credentials"))?;
        if !handed.is_empty() {
            super::filling::handed_over(&held, &handed)
                .map_err(refused("adapter's upstream credentials"))?;
        }
        Ok(Self {
            copy,
            data,
            held,
            key,
            made,
            env,
        })
    }
}

#[cfg(test)]
mod tests;
