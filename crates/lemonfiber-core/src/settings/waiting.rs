//! The services held back until the operator gives what they need to start: a setting
//! only the operator holds, a secret nobody has entered, or a setting holding a value
//! outside its choices.

use std::collections::BTreeMap;

use lemonfiber_manifest::container::{choices, Value};
use lemonfiber_manifest::Manifest;

use crate::model::{Waiting, Wanted};
use crate::plugin::Installed;
use crate::topology::{Placed, Topology};

/// One secret a service reads, as the rules for waiting and setup's questions read it.
pub(crate) struct Read<'a> {
    /// Its name in the secret store.
    pub(crate) name: String,
    /// What holding it is for.
    pub(crate) why: &'a str,
    /// Whether only the operator holds it and the service needs it to start.
    pub(crate) needed_to_start: bool,
    /// The values of other settings under which it is asked for.
    pub(crate) needs: &'a choices::Needs,
}

/// One service's settings and secrets, as the rules for waiting and setup's questions
/// read them.
pub(crate) struct Reading<'a> {
    /// The service's id.
    pub(crate) service: &'a str,
    /// Its environment.
    pub(crate) env: &'a BTreeMap<String, Value>,
    /// Every secret it reads.
    pub(crate) secrets: Vec<Read<'a>>,
}

/// Every service `manifest` and `installed` hold, each with its settings and secrets: the
/// stack's own, then each installed plugin's.
pub(crate) fn readings<'a>(manifest: &'a Manifest, installed: &'a [Installed]) -> Vec<Reading<'a>> {
    let stack = manifest.services.iter().map(|service| {
        let container = service.container();
        let secrets = service
            .secret
            .iter()
            .map(|secret| Read {
                name: super::named(container.namespace, &secret.id),
                why: &secret.why,
                needed_to_start: secret.recorded_with.is_some(),
                needs: &secret.needs,
            })
            .collect::<Vec<_>>();
        Reading {
            service: container.id,
            env: container.env,
            secrets,
        }
    });
    let plugins = installed.iter().flat_map(|one| {
        one.services.iter().map(move |placed| {
            let secrets = one
                .declared
                .secrets
                .iter()
                .filter(|secret| reads(&placed.fields.env, &secret.id))
                .map(|secret| Read {
                    name: super::named(&one.plugin, &secret.id),
                    why: &secret.why,
                    needed_to_start: secret.needed_to_start,
                    needs: &secret.needs,
                })
                .collect::<Vec<_>>();
            Reading {
                service: placed.service.as_str(),
                env: &placed.fields.env,
                secrets,
            }
        })
    });
    stack.chain(plugins).collect()
}

/// Every service `manifest` and `installed` hold that waits for something, each with
/// what it waits for, where `recorded` says what each setting holds and `held` whether a
/// secret, by its name in the secret store, holds a value.
pub(crate) fn waiting(
    manifest: &Manifest,
    installed: &[Installed],
    recorded: &dyn Fn(&str) -> Option<String>,
    held: &dyn Fn(&str) -> bool,
) -> Vec<Waiting> {
    readings(manifest, installed)
        .into_iter()
        .filter_map(|reading| {
            let wants = wanted(reading.env, &reading.secrets, recorded, held);
            (!wants.is_empty()).then(|| Waiting {
                service: reading.service.to_owned(),
                wants,
            })
        })
        .collect()
}

/// Whether `env` reads the secret `id`.
fn reads(env: &BTreeMap<String, Value>, id: &str) -> bool {
    env.values()
        .any(|value| matches!(value, Value::Secret(read) if read.secret == id))
}

/// What a service whose environment is `env` and whose secrets are `secrets` waits for.
fn wanted(
    env: &BTreeMap<String, Value>,
    secrets: &[Read<'_>],
    recorded: &dyn Fn(&str) -> Option<String>,
    held: &dyn Fn(&str) -> bool,
) -> Vec<Wanted> {
    let holds = |name: &str| recorded(name).or_else(|| choices::defaulted(env, name));
    let mut wants: Vec<Wanted> = Vec::new();
    for value in env.values() {
        let Value::Setting(read) = value else {
            continue;
        };
        let offered = choices::offered(env, &read.setting, &holds);
        let held_back = match recorded(&read.setting) {
            None => read.recorded_with.is_some() && choices::met(&read.needs, &holds),
            Some(value) => offered.as_ref().is_some_and(|one| !one.contains(&value)),
        };
        let want = Wanted::Setting {
            setting: read.setting.clone(),
            choices: offered,
        };
        if held_back && !wants.contains(&want) {
            wants.push(want);
        }
    }
    wants.extend(
        secrets
            .iter()
            .filter(|secret| {
                secret.needed_to_start && choices::met(secret.needs, &holds) && !held(&secret.name)
            })
            .map(|secret| Wanted::Secret {
                secret: secret.name.clone(),
            }),
    );
    wants
}

/// Every service held back, each with what it waits for, and every service placed inside
/// one of them as `topology` places it, waiting for that one.
pub(crate) fn held_back(
    manifest: &Manifest,
    installed: &[Installed],
    recorded: &dyn Fn(&str) -> Option<String>,
    held: &dyn Fn(&str) -> bool,
    topology: &Topology,
) -> Vec<Waiting> {
    through(waiting(manifest, installed, recorded, held), topology)
}

/// `waiting`, with every service placed inside one of them, as `topology` places it,
/// held back too, waiting for the service it runs through.
pub(crate) fn through(mut waiting: Vec<Waiting>, topology: &Topology) -> Vec<Waiting> {
    loop {
        let more: Vec<Waiting> = topology
            .placed
            .iter()
            .filter_map(|(service, placed)| match placed {
                Placed::Inside(host) => Some((service, host)),
                _ => None,
            })
            .filter(|(service, host)| {
                waiting.iter().any(|one| one.service == **host)
                    && !waiting.iter().any(|one| one.service == **service)
            })
            .map(|(service, host)| Waiting {
                service: service.clone(),
                wants: vec![Wanted::Service {
                    service: host.clone(),
                }],
            })
            .collect();
        if more.is_empty() {
            return waiting;
        }
        waiting.extend(more);
    }
}
