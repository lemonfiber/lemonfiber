//! A service an adapter of the plugin fronts, recorded with that adapter beside it: the
//! upstream served fresh from its pin, the adapter started from its own pin on the same
//! network with the key lemonfiber mints for it, the first run performed through the
//! adapter, the upstream's own requests asked, and every case of each contract the
//! adapter speaks asked of it in the order the contract publishes them.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use lemonfiber_contract::adapter::{
    ready_path, KEY_FILE, UPSTREAM_FILE, UPSTREAM_PASSWORD, UPSTREAM_USERNAME,
};
use lemonfiber_contract::conformance::cases;
use lemonfiber_contract::Contracted;
use lemonfiber_plugin::{Health, HealthKind};
use serde_json::Value;

use super::addressed::Addressed;
use super::asked::{answered, kept, Item};
use super::scrubbed::Scrub;
use super::served::{healthy, settled, settled_as, LOG_LINES};
use super::{Failed, Recorder};
use crate::plugin::conforming::{ASKED_WITH, RECORDINGS};
use crate::plugin::container::LOOPBACK;
use crate::plugin::installed::Placed;
use crate::plugin::program::{Operator, Starting, Throwaway};
use crate::plugin::recorded::{Answer, Asked};
use crate::ports::http::{Http, Method, Request, JSON};
use crate::ports::serving::{Serve, Served};
use filling::Filling;

mod filling;

/// The contract whose first run the adapter performs, as the seed performs it.
fn identity() -> String {
    use lemonfiber_contract::capabilities::identity::source;
    lemonfiber_contract::spoken(source::CAPABILITY, source::MAJOR)
}

/// One case of a contract, as it was asked and answered.
#[derive(Debug, Clone)]
pub(super) struct Case {
    /// The contract, as `<capability>@<major>`.
    pub(super) contract: String,
    /// The case.
    pub(super) called: String,
    /// What was asked.
    pub(super) asked: Asked,
    /// What the adapter answered, scrubbed.
    pub(super) answer: Answer,
}

impl Case {
    /// What names it.
    pub(super) fn named(&self) -> String {
        named(&self.contract, &self.called)
    }

    /// Where its recording sits beneath the plugin's source.
    pub(super) fn fixture(&self) -> String {
        fixture(&self.contract, &self.called)
    }
}

/// What names the case `case` of the contract `contract`.
fn named(contract: &str, case: &str) -> String {
    format!("conformance {contract}/{case}")
}

/// Where the recording of the case `case` of the contract `contract` sits.
fn fixture(contract: &str, case: &str) -> String {
    format!("{RECORDINGS}/{contract}/{case}.json")
}

/// What names each case of every contract `adapter` speaks, and where its recording sits.
pub(super) fn fixtures(adapter: &lemonfiber_plugin::Service) -> Vec<(String, String)> {
    contracts(adapter)
        .iter()
        .flat_map(|(spoken, capability)| {
            cases(capability)
                .into_iter()
                .map(|case| (named(spoken, &case.case), fixture(spoken, &case.case)))
                .collect::<Vec<_>>()
        })
        .collect()
}

/// The adapter that fronts a service, and how it is placed.
#[derive(Clone, Copy)]
pub(super) struct Fronting<'a> {
    /// The adapter.
    pub(super) service: &'a lemonfiber_plugin::Service,
    /// How installing it places it.
    pub(super) placed: &'a Placed,
}

/// Every contract's case `adapter` speaks, in the order each publishes them.
pub(super) fn contracts(
    adapter: &lemonfiber_plugin::Service,
) -> Vec<(String, lemonfiber_contract::Capability)> {
    let published = lemonfiber_contract::capabilities::all();
    adapter
        .speaks
        .iter()
        .filter_map(|spoken| {
            published
                .iter()
                .find(|one| lemonfiber_contract::spoken(one.name, one.major) == *spoken)
                .map(|one| (spoken.clone(), one.clone()))
        })
        .collect()
}

/// What the upstream answered each of its requests and the adapter each case, scrubbed.
pub(super) async fn answers(
    recorder: Recorder<'_>,
    root: &Path,
    upstream: &Placed,
    health: Option<&Health>,
    adapter: Fronting<'_>,
    items: &[&Item<'_>],
) -> Result<(Vec<Answer>, Vec<Case>), Failed> {
    let failing = |fault: Faulted| Failed {
        service: upstream.service.clone(),
        step: fault.step,
        why: crate::text::plain(&fault.why),
        log: Vec::new(),
    };
    let prepared = Prepared::of(recorder, root, upstream, adapter).map_err(failing)?;
    let serve = Serve {
        alias: Some(upstream.service.clone()),
        ..settled(upstream, &prepared.copy, prepared.data.as_deref())
    };
    let served = recorder
        .serving
        .serve(&serve)
        .await
        .map_err(|unran| failing(Faulted::new("container", unran.to_string(), Vec::new())))?;
    let operator = prepared
        .held
        .owner()
        .map_or_else(Operator::recording, |(uid, gid)| Operator {
            ids: (uid.to_string(), gid.to_string()),
            ..Operator::recording()
        });
    let beside = recorder
        .serving
        .beside(
            &served,
            &settled_as(adapter.placed, &prepared.held, None, &operator),
        )
        .await;
    let outcome = match &beside {
        Ok(fronting) => {
            Asking {
                recorder,
                upstream,
                health,
                served: &served,
                fronting,
                adapter,
                held: &prepared.held,
                key: &prepared.key,
            }
            .asked(items)
            .await
        }
        Err(unran) => Err(Faulted::new(
            "adapter's container",
            unran.to_string(),
            vec![prepared.key.clone()],
        )),
    };
    let finished = match outcome {
        Ok(answered) => Ok(answered),
        Err(fault) => {
            let log = recorder.serving.log(&served, LOG_LINES).await;
            let scrub = Scrub::of(&fault.made, Some(&served));
            let scrub = match &beside {
                Ok(fronting) => scrub.and_served(fronting),
                Err(_) => scrub,
            };
            Err(Failed {
                log: scrub.text(&log).lines().map(crate::text::plain).collect(),
                ..failing(Faulted::new(
                    &scrub.text(&fault.step),
                    scrub.text(&fault.why),
                    Vec::new(),
                ))
            })
        }
    };
    if let Ok(fronting) = &beside {
        recorder.serving.removed(fronting).await;
    }
    recorder.serving.removed(&served).await;
    finished
}

/// What both containers are given before either starts.
struct Prepared {
    /// The throwaway copy of the upstream's starting files.
    copy: Throwaway,
    /// Its throwaway library, where it takes one.
    data: Option<std::path::PathBuf>,
    /// The throwaway copy of the adapter's starting files, which holds its key.
    held: Throwaway,
    /// The key lemonfiber minted for the adapter.
    key: String,
}

impl Prepared {
    /// Both copies and the adapter's key, or why one could not be made.
    fn of(
        recorder: Recorder<'_>,
        root: &Path,
        upstream: &Placed,
        adapter: Fronting<'_>,
    ) -> Result<Self, Faulted> {
        let copy = Throwaway::of(Some(Starting {
            root,
            service: &upstream.service,
        }))
        .map_err(refused("starting files"))?;
        let data = if upstream.takes_data {
            Some(copy.emptied("data").map_err(refused("data"))?)
        } else {
            None
        };
        let held = Throwaway::of(Some(Starting {
            root,
            service: &adapter.placed.service,
        }))
        .map_err(refused("adapter's starting files"))?;
        let key = crate::secret::generate(recorder.random).ok_or_else(|| {
            refused("adapter's key")(crate::secret::NO_RANDOMNESS_FOR_PASSWORD.to_owned())
        })?;
        held.hold(KEY_FILE, &key)
            .map_err(refused("adapter's key"))?;
        Ok(Self {
            copy,
            data,
            held,
            key,
        })
    }
}

/// A step that failed, why, and every credential the run had made by then.
struct Faulted {
    step: String,
    why: String,
    made: Vec<String>,
}

impl Faulted {
    fn new(step: &str, why: String, made: Vec<String>) -> Self {
        Self {
            step: step.to_owned(),
            why,
            made,
        }
    }
}

/// What turns why `step` failed into its fault, before anything was made.
fn refused(step: &'static str) -> impl Fn(String) -> Faulted {
    move |why| Faulted::new(step, why, Vec::new())
}

/// Where the adapter is asked, and through what.
struct Reached {
    /// The adapter's address on the loopback interface.
    base: String,
    /// A transport to it.
    adapter: Arc<dyn Http>,
    /// A transport to the upstream, addressed as the stack addresses it.
    upstream: Arc<dyn Http>,
}

/// Everything asking the upstream and its adapter reaches them through.
struct Asking<'a> {
    recorder: Recorder<'a>,
    upstream: &'a Placed,
    health: Option<&'a Health>,
    served: &'a Served,
    fronting: &'a Served,
    adapter: Fronting<'a>,
    held: &'a Throwaway,
    key: &'a str,
}

impl Asking<'_> {
    /// Every answer, once both are healthy and the first run is done, scrubbed.
    async fn asked(&self, items: &[&Item<'_>]) -> Result<(Vec<Answer>, Vec<Case>), Faulted> {
        let mut made = vec![self.key.to_owned()];
        let reached = self
            .ready()
            .await
            .map_err(|(step, why)| Faulted::new(step, why, made.clone()))?;
        let contracts = contracts(self.adapter.service);
        if contracts.iter().any(|(spoken, _)| *spoken == identity()) {
            let password = self
                .first_run(&reached)
                .await
                .map_err(|why| Faulted::new("first run", why, made.clone()))?;
            made.push(password);
        }
        let mut upstream = Vec::new();
        for item in items {
            let step = format!("asking {}", item.named);
            let port = self.served.port.ok_or_else(|| {
                Faulted::new(
                    &step,
                    "it publishes no port, so there is nowhere to ask it".to_owned(),
                    made.clone(),
                )
            })?;
            let answer = answered(reached.upstream.as_ref(), port, item, None)
                .await
                .map_err(|why| Faulted::new(&step, why, made.clone()))?;
            upstream.push(answer);
        }
        let mut filling = Filling {
            random: self.recorder.random,
            held: self.held,
            answers: BTreeMap::new(),
            made,
        };
        let mut asked = Vec::new();
        for (spoken, capability) in &contracts {
            for case in cases(capability) {
                let step = named(spoken, &case.case);
                let one = self
                    .case(&reached, &mut filling, spoken, &case)
                    .await
                    .map_err(|why| Faulted::new(&step, why, filling.made.clone()))?;
                asked.push(one);
            }
        }
        self.scrubbed(upstream, asked, filling.made)
    }

    /// The upstream healthy and the adapter ready, and where each is asked; or the step
    /// that failed and why.
    async fn ready(&self) -> Result<Reached, (&'static str, String)> {
        let upstream = Addressed::to(
            (self.recorder.transport)(),
            LOOPBACK,
            super::served::inside(self.upstream),
        );
        healthy(self.recorder, upstream.as_ref(), self.health, self.served)
            .await
            .map_err(|why| ("health", why))?;
        let ready = Health {
            kind: HealthKind::Http,
            path: Some(ready_path()),
            timeout_s: self
                .adapter
                .service
                .health
                .as_ref()
                .and_then(|health| health.timeout_s),
        };
        let adapter = (self.recorder.transport)();
        healthy(self.recorder, adapter.as_ref(), Some(&ready), self.fronting)
            .await
            .map_err(|why| ("adapter's health", why))?;
        let base = self
            .fronting
            .port
            .map(|port| format!("http://{LOOPBACK}:{port}"))
            .ok_or((
                "adapter's health",
                "it publishes no port, so nothing can ask it".to_owned(),
            ))?;
        Ok(Reached {
            base,
            adapter,
            upstream,
        })
    }

    /// One case asked of the adapter with its placeholders filled, and what it answered.
    async fn case(
        &self,
        reached: &Reached,
        filling: &mut Filling<'_>,
        spoken: &str,
        case: &lemonfiber_contract::conformance::Case<'_>,
    ) -> Result<Case, String> {
        let filled = filling.filled(&case.asks)?;
        let key = case.keyed.then(|| {
            (
                crate::endpoint::AUTHORIZATION.to_owned(),
                format!("Bearer {}", self.key),
            )
        });
        let request = Request {
            method: Method::Post,
            url: format!("{}{}", reached.base, case.operation.path()),
            headers: [
                ("Content-Type".to_owned(), JSON.to_owned()),
                ("Accept".to_owned(), JSON.to_owned()),
            ]
            .into_iter()
            .chain(key)
            .collect(),
            body: Some(filled.value.to_string()),
            pinned: None,
        };
        let response = reached
            .adapter
            .send(&request)
            .await
            .map_err(|unreachable| unreachable.reason)?;
        if (200..300).contains(&response.status) {
            filling.keep_minted(&filled.minted)?;
        }
        filling.answers.insert(
            format!("{spoken}/{}", case.case),
            serde_json::from_str(&response.body).unwrap_or(Value::Null),
        );
        Ok(Case {
            contract: spoken.to_owned(),
            called: case.case.clone(),
            asked: Asked {
                method: ASKED_WITH.to_owned(),
                path: case.operation.path(),
                accept: Some(JSON.to_owned()),
            },
            answer: kept(&response),
        })
    }

    /// Every answer scrubbed of what the run made and both containers' identity, or
    /// nothing where one still holds any of it.
    fn scrubbed(
        &self,
        upstream: Vec<Answer>,
        asked: Vec<Case>,
        made: Vec<String>,
    ) -> Result<(Vec<Answer>, Vec<Case>), Faulted> {
        let scrub = Scrub::of(&made, Some(self.served)).and_served(self.fronting);
        let upstream: Vec<Answer> = upstream
            .into_iter()
            .map(|answer| scrub.answer(answer))
            .collect();
        let cases: Vec<Case> = asked
            .into_iter()
            .map(|case| Case {
                answer: scrub.answer(case.answer),
                ..case
            })
            .collect();
        let text: String = upstream
            .iter()
            .chain(cases.iter().map(|case| &case.answer))
            .map(|answer| serde_json::to_string(answer).unwrap_or_default())
            .collect();
        if scrub.holds(&text) {
            return Err(Faulted::new(
                "scrubbing",
                "what was answered still holds a credential or a container's identity once \
                 scrubbed, so nothing is written"
                    .to_owned(),
                made,
            ));
        }
        Ok((upstream, cases))
    }

    /// The upstream's first run, performed through the adapter as the seed performs it,
    /// and the administrator's password it left in the adapter's `upstream.json`.
    async fn first_run(&self, reached: &Reached) -> Result<String, String> {
        let source = lemonfiber_contract::capabilities::identity::source::Adapter(Contracted::new(
            Arc::clone(&reached.adapter),
            &reached.base,
            &self.adapter.placed.service,
            self.key,
        ));
        let keep = |password: &str| {
            let credential = serde_json::json!({
                UPSTREAM_USERNAME: crate::config::MEDIA_SERVER_ADMIN_USER,
                UPSTREAM_PASSWORD: password,
            });
            self.held.hold(UPSTREAM_FILE, &credential.to_string())
        };
        crate::seed::wire_media_server_admin(&source, self.recorder.random, None, false, &keep)
            .await
            .map_err(unfinished)
    }
}

/// Why a first run that left the upstream without its administrator did.
fn unfinished(state: crate::seed::State) -> String {
    match state {
        crate::seed::State::Skipped { reason } => reason,
        crate::seed::State::Failed { detail } => detail,
        other => format!("{other:?}"),
    }
}

#[cfg(test)]
mod tests;
