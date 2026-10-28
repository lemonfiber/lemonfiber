//! What both containers of a recording are given before either starts: the throwaway
//! copies of their starting files, the adapter's key and its upstream's credentials, each
//! secret one value wherever it is written.

use std::collections::BTreeMap;
use std::path::Path;

use lemonfiber_manifest::container::{Minted, Secret, Value};

use lemonfiber_contract::adapter::KEY_FILE;

use super::super::stood_in::{StandIns, STAND_INS};
use super::{refused, Faulted, Fronting, Placed, Recorder, Starting, Throwaway};
use crate::ports::random::Random;

/// What both containers are given before either starts.
pub(super) struct Prepared {
    /// The throwaway copy of the upstream's starting files.
    pub(super) copy: Throwaway,
    /// Its throwaway library, where it takes one.
    pub(super) data: Option<std::path::PathBuf>,
    /// The throwaway copy of the adapter's starting files, which holds its key, where it
    /// is not its own upstream and so has a copy apart from [`Self::copy`].
    apart: Option<Throwaway>,
    /// The key lemonfiber minted for the adapter.
    pub(super) key: String,
    /// Every credential minted for the run so far, which every answer is scrubbed of.
    pub(super) made: Vec<String>,
    /// Each variable of the upstream's environment that names a secret or a setting
    /// declaring a stand-in, as `NAME=value`.
    pub(super) env: Vec<String>,
}

/// Each secret either copy's files name, with the value written for it.
fn written(copy: &Throwaway, held: Option<&Throwaway>) -> BTreeMap<String, String> {
    held.into_iter()
        .flat_map(Throwaway::written)
        .chain(copy.written())
        .map(|(secret, value)| (secret.to_owned(), value.to_owned()))
        .collect()
}

/// What a run mints with: the randomness, and the shape each secret is declared in.
struct Minting<'a> {
    /// Where minted values come from.
    random: &'a dyn Random,
    /// The shape each secret the plugin declares is minted in, by its id.
    shapes: BTreeMap<String, Minted>,
}

/// The value of `secret` for this run: the one already written for it, or one minted now
/// in its declared shape and kept, so the upstream's files, its environment and the adapter
/// all hold the same.
fn valued(
    minting: &Minting<'_>,
    secrets: &mut BTreeMap<String, String>,
    secret: &str,
    made: &mut Vec<String>,
) -> Result<String, String> {
    if let Some(held) = secrets.get(secret) {
        return Ok(held.clone());
    }
    let fresh = crate::plugin::seeding::minted_as(
        minting.random,
        crate::plugin::seeding::shape_of(&minting.shapes, secret),
    )
    .ok_or_else(|| crate::plugin::seeding::UNMINTED.to_owned())?;
    made.push(fresh.clone());
    secrets.insert(secret.to_owned(), fresh.clone());
    Ok(fresh)
}

/// Each variable of `upstream`'s environment that names a secret, as `NAME=value`.
fn environment(
    minting: &Minting<'_>,
    secrets: &mut BTreeMap<String, String>,
    upstream: &Placed,
    made: &mut Vec<String>,
) -> Result<Vec<String>, String> {
    let mut env = Vec::new();
    for (name, value) in &upstream.fields.env {
        if let Value::Secret(of) = value {
            env.push(format!(
                "{name}={}",
                valued(minting, secrets, &of.secret, made)?
            ));
        }
    }
    Ok(env)
}

/// Each field of the adapter's `upstream`, holding the value its upstream was given.
fn handed(
    minting: &Minting<'_>,
    secrets: &mut BTreeMap<String, String>,
    adapter: &Placed,
    made: &mut Vec<String>,
) -> Result<Vec<(String, String)>, String> {
    let mut handed = Vec::new();
    for (field, secret) in &adapter.upstream {
        handed.push((field.clone(), valued(minting, secrets, secret, made)?));
    }
    Ok(handed)
}

impl Prepared {
    /// The copy that holds the adapter's key: its own, or its upstream's where it is its
    /// own upstream.
    pub(super) fn held(&self) -> &Throwaway {
        self.apart.as_ref().unwrap_or(&self.copy)
    }

    /// Both copies and the adapter's key, or why one could not be made.
    pub(super) fn of(
        recorder: Recorder<'_>,
        root: &Path,
        upstream: &Placed,
        adapter: Fronting<'_>,
        declared: &[Secret],
    ) -> Result<Self, Faulted> {
        let mut stand_ins = StandIns::of(declared, recorder.random).map_err(refused(STAND_INS))?;
        let stood = stand_ins
            .settings(upstream, recorder.random)
            .map_err(refused(STAND_INS))?;
        let copy = Throwaway::of(
            Some(Starting {
                root,
                service: &upstream.service,
            }),
            recorder.random,
        )
        .map_err(refused("starting files"))?;
        let data = super::super::served::library(upstream, &copy).map_err(refused("data"))?;
        super::super::served::certified(upstream, &copy).map_err(refused("certificate"))?;
        let apart = (adapter.placed.service != upstream.service)
            .then(|| {
                Throwaway::of(
                    Some(Starting {
                        root,
                        service: &adapter.placed.service,
                    }),
                    recorder.random,
                )
            })
            .transpose()
            .map_err(refused("adapter's starting files"))?;
        let held = apart.as_ref().unwrap_or(&copy);
        let key = crate::secret::generate(recorder.random).ok_or_else(|| {
            refused("adapter's key")(crate::secret::NO_RANDOMNESS_FOR_PASSWORD.to_owned())
        })?;
        held.hold(KEY_FILE, &key)
            .map_err(refused("adapter's key"))?;
        let mut made: Vec<String> = std::iter::once(key.clone())
            .chain(copy.made().map(str::to_owned))
            .chain(apart.iter().flat_map(Throwaway::made).map(str::to_owned))
            .chain(stand_ins.made.iter().cloned())
            .collect();
        let mut secrets = written(&copy, apart.as_ref());
        for (secret, value) in stand_ins.held() {
            secrets
                .entry(secret.to_owned())
                .or_insert_with(|| value.to_owned());
        }
        let minting = Minting {
            random: recorder.random,
            shapes: crate::plugin::seeding::shapes(root),
        };
        let mut env = environment(&minting, &mut secrets, upstream, &mut made)
            .map_err(refused("upstream's environment"))?;
        env.extend(stood);
        let handed = handed(&minting, &mut secrets, adapter.placed, &mut made)
            .map_err(refused("adapter's upstream credentials"))?;
        if !handed.is_empty() {
            super::filling::handed_over(held, &handed)
                .map_err(refused("adapter's upstream credentials"))?;
        }
        Ok(Self {
            copy,
            data,
            apart,
            key,
            made,
            env,
        })
    }
}

#[cfg(test)]
mod tests;
