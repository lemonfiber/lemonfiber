//! What setup's VPN step asks of the service filling `network.egress-guard`, and the
//! answer held to it.
//!
//! One walk does both. It asks each setting that declares choices, each after every
//! setting its choices need, offering the values the answers before it open; then each
//! setting and secret the guard needs to start whose needs those answers meet. A surface
//! that holds the conversation answers it as it goes, and an answer that arrived whole
//! is walked with what it gave, so what is asked and what is checked cannot come apart.

use std::collections::{BTreeMap, BTreeSet};

use lemonfiber_manifest::container::{choices, Value};
use lemonfiber_manifest::Manifest;

use crate::error::codes::setup::GUARD_UNANSWERED;
use crate::error::{Problem, Remedy, State};
use crate::model::GuardQuestion;
use crate::plugin::Installed;
use crate::settings::choosing::{self, Outside};
use crate::settings::readers::{choosable, texts};
use crate::wizard::Guarded;

/// The service filling `network.egress-guard`: its environment and the secrets it reads.
#[derive(Debug, Clone)]
pub struct Guard {
    /// Its environment.
    env: BTreeMap<String, Value>,
    /// Every secret it reads.
    secrets: Vec<Secret>,
}

/// One secret the guard reads.
#[derive(Debug, Clone)]
struct Secret {
    /// Its name in the secret store.
    name: String,
    /// What holding it is for.
    why: String,
    /// Whether only the operator holds it and the guard needs it to start.
    needed_to_start: bool,
    /// The values of settings under which it is asked for.
    needs: choices::Needs,
}

/// One question as the walk puts it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Asking<'a> {
    /// One of `offered`, the values the answers before it open; `default` where it is
    /// left unanswered.
    Choice {
        /// The setting.
        setting: &'a str,
        /// The values it may hold now.
        offered: Vec<String>,
        /// What it holds where it is left unanswered.
        default: Option<String>,
    },
    /// A setting's value, which only the operator holds.
    Value {
        /// The setting.
        setting: &'a str,
    },
    /// A secret, entered without being shown.
    Secret {
        /// The name the secret store holds it under.
        secret: &'a str,
        /// What holding it is for.
        why: &'a str,
    },
}

/// What a walk came to: what was given, and what it leaves wrong.
#[derive(Debug, Default)]
struct Walked {
    /// Every setting and secret given.
    given: Guarded,
    /// Each setting given a value outside the values open to it.
    outside: Vec<Outside>,
    /// Each setting and secret the guard needs and nothing was given for.
    missing: Vec<String>,
}

impl Guard {
    /// The guard `service`, as `manifest` or one of `installed` declares it.
    pub(crate) fn of(manifest: &Manifest, installed: &[Installed], service: &str) -> Option<Self> {
        crate::settings::waiting::readings(manifest, installed)
            .into_iter()
            .find(|reading| reading.service == service)
            .map(|reading| Self {
                env: reading.env.clone(),
                secrets: reading
                    .secrets
                    .iter()
                    .map(|read| Secret {
                        name: read.name.clone(),
                        why: read.why.to_owned(),
                        needed_to_start: read.needed_to_start,
                        needs: read.needs.clone(),
                    })
                    .collect(),
            })
    }

    /// Every question the VPN step may ask, in the order it asks them, each with what it
    /// is asked under.
    pub(crate) fn questions(&self) -> Vec<GuardQuestion> {
        let defaulted = |name: &str| choices::defaulted(&self.env, name);
        let chosen = self
            .chosen()
            .into_iter()
            .map(|(setting, one_of)| GuardQuestion::Choice {
                setting: setting.to_owned(),
                default: defaulted(setting),
                choices: choosable(one_of, &defaulted),
            });
        let values = self
            .valued()
            .into_iter()
            .map(|(setting, needs)| GuardQuestion::Value {
                setting: setting.to_owned(),
                needs: texts(needs),
            });
        let secrets = self.needed().map(|secret| GuardQuestion::Secret {
            secret: secret.name.clone(),
            why: secret.why.clone(),
            needs: texts(&secret.needs),
        });
        chosen.chain(values).chain(secrets).collect()
    }

    /// Ask each question as the answers before it leave it to be asked, through
    /// `answering`, and take what it gives: nothing leaves a question unanswered.
    pub fn asked(&self, answering: &mut dyn FnMut(&Asking<'_>) -> Option<String>) -> Guarded {
        self.walked(answering).given
    }

    /// What of `given` the guard takes, where it answers every question it leaves to be
    /// asked and nothing else: each setting and secret asked under it, and nothing it
    /// leaves blank.
    ///
    /// # Errors
    ///
    /// The refusal naming each setting `given` leaves outside the values open to it; and
    /// otherwise [`GUARD_UNANSWERED`] naming each setting and secret it leaves out and
    /// each it gives that is not asked under it, never by value.
    pub(crate) fn taken(&self, given: &Guarded) -> Result<Guarded, Box<Problem>> {
        let walked = self.walked(&mut |asking| {
            match asking {
                Asking::Choice { setting, .. } | Asking::Value { setting } => {
                    given.settings.get(*setting)
                }
                Asking::Secret { secret, .. } => given.secrets.get(*secret),
            }
            .cloned()
        });
        if !walked.outside.is_empty() {
            return Err(Box::new(choosing::refused(&walked.outside)));
        }
        let unasked: Vec<String> = unasked(&given.settings, &walked.given.settings)
            .chain(unasked(&given.secrets, &walked.given.secrets))
            .collect();
        if walked.missing.is_empty() && unasked.is_empty() {
            return Ok(walked.given);
        }
        Err(Box::new(unanswered(&walked.missing, &unasked)))
    }

    /// One walk through the questions, `answering` each as it is asked.
    fn walked(&self, answering: &mut dyn FnMut(&Asking<'_>) -> Option<String>) -> Walked {
        let mut walked = Walked::default();
        for (setting, _) in self.chosen() {
            let held = |name: &str| {
                walked
                    .given
                    .settings
                    .get(name)
                    .cloned()
                    .or_else(|| choices::defaulted(&self.env, name))
            };
            let offered = choices::offered(&self.env, setting, &held).unwrap_or_default();
            let default = choices::defaulted(&self.env, setting);
            let answer = answering(&Asking::Choice {
                setting,
                offered: offered.clone(),
                default: default.clone(),
            })
            .filter(|answer| !answer.is_empty());
            match (answer, default) {
                (Some(value), _) => {
                    if !offered.contains(&value) {
                        walked.outside.push(Outside {
                            setting: setting.to_owned(),
                            value: value.clone(),
                            choices: offered,
                        });
                    }
                    walked.given.settings.insert(setting.to_owned(), value);
                }
                (None, Some(default)) if offered.contains(&default) => {}
                (None, _) => walked.missing.push(setting.to_owned()),
            }
        }
        for (setting, needs) in self.valued() {
            if !self.met(needs, &walked.given) {
                continue;
            }
            match answering(&Asking::Value { setting }).filter(|answer| !answer.is_empty()) {
                Some(value) => {
                    walked.given.settings.insert(setting.to_owned(), value);
                }
                None => walked.missing.push(setting.to_owned()),
            }
        }
        for secret in self.needed() {
            if !self.met(&secret.needs, &walked.given) {
                continue;
            }
            let asking = Asking::Secret {
                secret: &secret.name,
                why: &secret.why,
            };
            match answering(&asking).filter(|answer| !answer.is_empty()) {
                Some(value) => {
                    walked.given.secrets.insert(secret.name.clone(), value);
                }
                None => walked.missing.push(secret.name.clone()),
            }
        }
        walked
    }

    /// Whether `needs` is met by what `given` gives, and each setting's default beside it.
    fn met(&self, needs: &choices::Needs, given: &Guarded) -> bool {
        choices::met(needs, &|name: &str| {
            given
                .settings
                .get(name)
                .cloned()
                .or_else(|| choices::defaulted(&self.env, name))
        })
    }

    /// Each setting that declares choices, with them, each after every setting its
    /// choices need; by name where nothing orders two.
    fn chosen(&self) -> Vec<(&str, &[choices::Choice])> {
        let mut waiting: BTreeMap<&str, &[choices::Choice]> = self
            .env
            .values()
            .filter_map(|value| match value {
                Value::Setting(read) => Some((read.setting.as_str(), read.one_of.as_deref()?)),
                _ => None,
            })
            .collect();
        let mut ordered = Vec::new();
        while !waiting.is_empty() {
            let ready: Vec<&str> = waiting
                .iter()
                .filter(|(_, one_of)| {
                    one_of
                        .iter()
                        .filter_map(choices::Choice::needs)
                        .flat_map(|needs| needs.keys())
                        .all(|needed| !waiting.contains_key(needed.as_str()))
                })
                .map(|(setting, _)| *setting)
                .collect();
            let taken = if ready.is_empty() {
                waiting.keys().copied().collect()
            } else {
                ready
            };
            for setting in taken {
                if let Some(one_of) = waiting.remove(setting) {
                    ordered.push((setting, one_of));
                }
            }
        }
        ordered
    }

    /// Each setting only the operator holds that the guard needs to start and that
    /// declares no choices, once, with the needs it is asked under.
    fn valued(&self) -> Vec<(&str, &choices::Needs)> {
        let mut seen = BTreeSet::new();
        self.env
            .values()
            .filter_map(|value| match value {
                Value::Setting(read) if read.recorded_with.is_some() && read.one_of.is_none() => {
                    Some((read.setting.as_str(), &read.needs))
                }
                _ => None,
            })
            .filter(|(setting, _)| seen.insert(*setting))
            .collect()
    }

    /// Each secret only the operator holds that the guard needs to start.
    fn needed(&self) -> impl Iterator<Item = &Secret> {
        self.secrets.iter().filter(|secret| secret.needed_to_start)
    }
}

/// What of `given` `guard` takes, as [`Guard::taken`] says; where no guard is there to
/// ask anything, nothing, and only from an answer that gives nothing.
///
/// # Errors
///
/// As [`Guard::taken`]; and with no guard, [`GUARD_UNANSWERED`] naming everything
/// `given` gives.
pub(crate) fn checking(guard: Option<&Guard>, given: &Guarded) -> Result<Guarded, Box<Problem>> {
    if let Some(guard) = guard {
        return guard.taken(given);
    }
    let none = BTreeMap::new();
    let unasked: Vec<String> = unasked(&given.settings, &none)
        .chain(unasked(&given.secrets, &none))
        .collect();
    if unasked.is_empty() {
        return Ok(Guarded::default());
    }
    Err(Box::new(unanswered(&[], &unasked)))
}

/// Each name `given` gives a value that `taken` does not hold.
fn unasked<'a>(
    given: &'a BTreeMap<String, String>,
    taken: &'a BTreeMap<String, String>,
) -> impl Iterator<Item = String> + 'a {
    given
        .iter()
        .filter(|(name, value)| !value.is_empty() && !taken.contains_key(*name))
        .map(|(name, _)| name.clone())
}

/// The refusal of a VPN answer that leaves out `missing` and gives `unasked`.
fn unanswered(missing: &[String], unasked: &[String]) -> Problem {
    let said = [
        (!missing.is_empty()).then(|| format!("It leaves out {}", missing.join(", "))),
        (!unasked.is_empty()).then(|| {
            format!(
                "it gives {}, which the answers given do not ask for",
                unasked.join(", ")
            )
        }),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join("; and ");
    Problem::new(
        GUARD_UNANSWERED,
        "The VPN answer does not give the egress guard what it needs to start",
        format!("{said}. Nothing was recorded, and the VPN step is asked again."),
        Remedy::new("Answer the VPN step again with every setting and secret it names"),
    )
    .in_state(State::Guided)
}

#[cfg(test)]
pub(super) mod tests;
