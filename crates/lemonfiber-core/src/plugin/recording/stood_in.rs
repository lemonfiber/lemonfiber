//! The stand-ins a recording gives a service for each value only the operator holds: a
//! constant reaching no third party, or random bytes made for that run alone.

use std::collections::BTreeMap;

use base64::Engine as _;
use lemonfiber_manifest::container::{RecordedWith, Secret, Value, RECORDED_MINTED};

use crate::plugin::installed::Placed;
use crate::ports::random::Random;

/// The step a recording names where its stand-ins could not be made.
pub(super) const STAND_INS: &str = "stand-ins";

/// Why a stand-in to be made for the run was not.
const UNMADE: &str = "no random bytes could be had for a stand-in";

/// Each stand-in made for one run, by the secret it stands in for.
#[derive(Debug, Default)]
pub(super) struct StandIns {
    /// Each secret stood in for, with what stands in for it.
    secrets: BTreeMap<String, String>,
    /// Each stand-in made for the run rather than declared, which every answer is
    /// scrubbed of.
    pub(super) made: Vec<String>,
}

impl StandIns {
    /// A stand-in for each of `secrets` that declares one.
    pub(super) fn of(secrets: &[Secret], random: &dyn Random) -> Result<Self, String> {
        let mut stand_ins = Self::default();
        for secret in secrets {
            if let Some(recorded) = &secret.recorded_with {
                let value = stand_ins.standing(recorded, random)?;
                stand_ins.secrets.insert(secret.id.clone(), value);
            }
        }
        Ok(stand_ins)
    }

    /// What stands in for `secret`, where it declares a stand-in.
    pub(super) fn secret(&self, secret: &str) -> Option<&str> {
        self.secrets.get(secret).map(String::as_str)
    }

    /// Each variable of `placed`'s environment holding a setting that declares a stand-in,
    /// as `NAME=value`.
    pub(super) fn settings(
        &mut self,
        placed: &Placed,
        random: &dyn Random,
    ) -> Result<Vec<String>, String> {
        let mut env = Vec::new();
        for (name, value) in &placed.fields.env {
            if let Value::Setting(setting) = value {
                if let Some(recorded) = &setting.recorded_with {
                    env.push(format!("{name}={}", self.standing(recorded, random)?));
                }
            }
        }
        Ok(env)
    }

    /// Each variable of `placed`'s environment holding a secret stood in for, as
    /// `NAME=value`.
    pub(super) fn secrets(&self, placed: &Placed) -> Vec<String> {
        placed
            .fields
            .env
            .iter()
            .filter_map(|(name, value)| match value {
                Value::Secret(of) => self
                    .secret(&of.secret)
                    .map(|stood| format!("{name}={stood}")),
                Value::Constant(_) | Value::Setting(_) | Value::Url(_) => None,
            })
            .collect()
    }

    /// Each secret stood in for, with what stands in for it.
    pub(super) fn held(&self) -> impl Iterator<Item = (&str, &str)> {
        self.secrets
            .iter()
            .map(|(secret, value)| (secret.as_str(), value.as_str()))
    }

    /// What `recorded` stands in as for this run.
    fn standing(&mut self, recorded: &RecordedWith, random: &dyn Random) -> Result<String, String> {
        match recorded {
            RecordedWith::Constant(constant) => Ok(constant.text()),
            RecordedWith::Minted => {
                let bytes = random
                    .bytes(RECORDED_MINTED)
                    .ok_or_else(|| UNMADE.to_owned())?;
                let made = base64::engine::general_purpose::STANDARD.encode(bytes);
                self.made.push(made.clone());
                Ok(made)
            }
        }
    }
}

/// Each variable of `placed`'s environment a recording gives a stand-in, by name.
pub(super) fn named(placed: &Placed, secrets: &[Secret]) -> Vec<String> {
    let stood_in = |secret: &str| {
        secrets
            .iter()
            .any(|declared| declared.id == secret && declared.recorded_with.is_some())
    };
    placed
        .fields
        .env
        .iter()
        .filter(|(_, value)| match value {
            Value::Setting(setting) => setting.recorded_with.is_some(),
            Value::Secret(of) => stood_in(&of.secret),
            Value::Constant(_) | Value::Url(_) => false,
        })
        .map(|(name, _)| name.clone())
        .collect()
}

#[cfg(test)]
mod tests;
