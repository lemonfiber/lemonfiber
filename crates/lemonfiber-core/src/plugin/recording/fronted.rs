//! A service an adapter of the plugin fronts, recorded with that adapter beside it: the
//! upstream served fresh from its pin, the adapter started from its own pin, or the image
//! a check names in its place, on the same network with the key lemonfiber mints for it,
//! the first run performed through the adapter, the upstream's own requests asked, and
//! every case of each contract the adapter speaks asked of it in the order the contract
//! publishes them, a contract after every one whose answers its cases read and a case the
//! first run already asked answered by what the first run heard.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use lemonfiber_contract::adapter::{ready_path, UPSTREAM_PASSWORD, UPSTREAM_USERNAME};
use lemonfiber_contract::conformance::filed;
use lemonfiber_contract::Contracted;
use lemonfiber_manifest::container::Secret;
use lemonfiber_plugin::{Health, HealthKind};
use serde_json::Value;

use super::addressed::Addressed;
use super::asked::{answered, kept, Item};
use super::scrubbed::Scrub;
use super::served::{allowed, healthy, healthy_at, LOG_LINES, POLL};
use super::{Failed, Recorder};
use crate::plugin::conforming::ASKED_WITH;
use crate::plugin::container::LOOPBACK;
use crate::plugin::installed::Placed;
use crate::plugin::program::{Starting, Throwaway};
use crate::plugin::recorded::{Answer, Asked};
use crate::ports::http::{Http, Method, Request, Response, JSON};
use crate::ports::serving::Served;
use filling::Filling;
use preparing::Prepared;
use serving::{parties, serves};
use speaking::{contracts, fixture, named};
use tapped::Tapped;

mod claiming;
mod enveloping;
mod filling;
mod preparing;
mod serving;
mod speaking;
mod tapped;

pub(in crate::plugin) use filling::handed_over;
pub(super) use speaking::fixtures;

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

/// The adapter that fronts a service, and how it is placed.
#[derive(Clone, Copy)]
pub(super) struct Fronting<'a> {
    /// The adapter.
    pub(super) service: &'a lemonfiber_plugin::Service,
    /// How installing it places it.
    pub(super) placed: &'a Placed,
}

/// What the upstream answered each of its requests and the adapter each case, scrubbed.
pub(super) async fn answers(
    recorder: Recorder<'_>,
    root: &Path,
    upstream: &Placed,
    health: Option<&Health>,
    adapter: Fronting<'_>,
    items: &[&Item<'_>],
    secrets: &[Secret],
) -> Result<(Vec<Answer>, Vec<Case>), Failed> {
    let failing = |fault: Faulted| Failed {
        service: upstream.service.clone(),
        step: fault.step,
        why: crate::text::plain(&fault.why),
        log: Vec::new(),
    };
    let prepared = Prepared::of(recorder, root, upstream, adapter, secrets).map_err(failing)?;
    let envelope = enveloping::guards(adapter.placed)
        .then(enveloping::declared)
        .transpose()
        .map_err(failing)?;
    let (reaching, answering) = parties(recorder, adapter, upstream, envelope.is_some())
        .await
        .map_err(failing)?;
    let (serve, apart) = serves(recorder, upstream, adapter, &prepared, reaching);
    let alone = apart.is_none();
    let served = recorder
        .serving
        .serve(&serve)
        .await
        .map_err(|unran| failing(Faulted::new("container", unran.to_string(), Vec::new())))?;
    let beside = match &apart {
        None => Ok(served.clone()),
        Some(started) => recorder.serving.beside(&served, started).await,
    };
    let outcome = match &beside {
        Ok(fronting) => {
            Asking {
                recorder,
                upstream,
                health,
                served: &served,
                fronting,
                adapter,
                held: prepared.held(),
                key: &prepared.key,
                made: &prepared.made,
                envelope: envelope.as_ref(),
                certificate: prepared.certificate.as_deref(),
            }
            .asked(items)
            .await
        }
        Err(unran) => Err(Faulted::new(
            "adapter's container",
            unran.to_string(),
            prepared.made.clone(),
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
    if let (Ok(fronting), false) = (&beside, alone) {
        recorder.serving.removed(fronting).await;
    }
    recorder.serving.removed(&served).await;
    drop(answering);
    finished
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
    made: &'a [String],
    envelope: Option<&'a lemonfiber_ports::service::Declared>,
    certificate: Option<&'a str>,
}

impl Asking<'_> {
    /// Every answer, once both are healthy and the first run is done, scrubbed.
    async fn asked(&self, items: &[&Item<'_>]) -> Result<(Vec<Answer>, Vec<Case>), Faulted> {
        let mut made = self.made.to_vec();
        let reached = self
            .ready()
            .await
            .map_err(|(step, why)| Faulted::new(step, why, made.clone()))?;
        if let Some(declared) = self.envelope {
            self.enveloped(&reached, declared)
                .await
                .map_err(|why| Faulted::new(enveloping::STEP, why, made.clone()))?;
        }
        let contracts = contracts(self.adapter.service);
        let mut heard = BTreeMap::new();
        if contracts.iter().any(|(spoken, _)| *spoken == identity()) {
            let (password, first) = self
                .first_run(&reached)
                .await
                .map_err(|why| Faulted::new("first run", why, made.clone()))?;
            made.push(password);
            heard = first;
        }
        if contracts
            .iter()
            .any(|(spoken, _)| *spoken == claiming::owning())
        {
            heard.extend(
                self.claimed(&reached)
                    .await
                    .map_err(|why| Faulted::new("claim", why, made.clone()))?,
            );
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
            let answer = answered(reached.upstream.as_ref(), port, item)
                .await
                .map_err(|why| Faulted::new(&step, why, made.clone()))?;
            upstream.push(answer);
        }
        let mut filling = Filling {
            random: self.recorder.random,
            held: self.held,
            answers: BTreeMap::new(),
            made,
            certificate: self.certificate.map(str::to_owned),
        };
        let mut asked = Vec::new();
        for (spoken, capability) in &contracts {
            for case in filed(capability, &self.upstream.media_types) {
                let step = named(spoken, &case.case);
                let one = self
                    .case(&reached, &mut filling, spoken, &case, &heard)
                    .await
                    .map_err(|why| Faulted::new(&step, why, filling.made.clone()))?;
                asked.push(one);
            }
        }
        self.scrubbed(upstream, asked, filling.made)
    }

    /// The guard handed the envelope its own requests are asked under, over its contract.
    async fn enveloped(
        &self,
        reached: &Reached,
        declared: &lemonfiber_ports::service::Declared,
    ) -> Result<(), String> {
        use crate::ports::service::Guarding;
        let guard = lemonfiber_contract::capabilities::stream::guard::Adapter(Contracted::new(
            Arc::clone(&reached.adapter),
            &reached.base,
            &self.adapter.placed.service,
            self.key,
        ));
        guard
            .guard(&enveloping::guarded(declared.clone()))
            .await
            .map_err(|failure| failure.to_string())
    }

    /// The upstream healthy and the adapter ready, and where each is asked; or the step
    /// that failed and why.
    async fn ready(&self) -> Result<Reached, (&'static str, String)> {
        let upstream = Addressed::to(
            (self.recorder.transport)(),
            LOOPBACK,
            super::served::inside(self.upstream),
        );
        let serving = self.recorder.serving;
        healthy(serving, upstream.as_ref(), self.health, self.served)
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
        let contract = self.fronting.contract.or(self.fronting.port);
        healthy_at(
            serving,
            adapter.as_ref(),
            Some(&ready),
            self.fronting,
            contract,
        )
        .await
        .map_err(|why| ("adapter's health", why))?;
        let base = contract
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

    /// One case and what the adapter answered it: what the first run heard where it asked
    /// the case under the key already, and the case asked with its placeholders filled
    /// otherwise. Every credential the contract says the answer holds is kept among what
    /// the run made, so it is scrubbed.
    async fn case(
        &self,
        reached: &Reached,
        filling: &mut Filling<'_>,
        spoken: &str,
        case: &lemonfiber_contract::conformance::Case<'_>,
        heard: &BTreeMap<String, Response>,
    ) -> Result<Case, String> {
        let url = format!("{}{}", reached.base, case.operation.path());
        let unoffered = filling.unoffered(&case.asks, &self.adapter.service.speaks);
        let response = match (unoffered, heard.get(&url).filter(|_| case.keyed)) {
            (Some(why), _) => filling::unsupported(why),
            (None, Some(first)) => first.clone(),
            (None, None) => self.posted(reached, filling, case, url).await?,
        };
        let answer: Value = serde_json::from_str(&response.body).unwrap_or(Value::Null);
        filling.made.extend(issued(case.operation.secret, &answer));
        filling
            .answers
            .insert(format!("{spoken}/{}", case.case), answer);
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

    /// `case` posted to the adapter at `url` with its placeholders filled, and what it
    /// answered.
    async fn posted(
        &self,
        reached: &Reached,
        filling: &mut Filling<'_>,
        case: &lemonfiber_contract::conformance::Case<'_>,
        url: String,
    ) -> Result<Response, String> {
        let filled = filling.filled(&case.asks)?;
        let key = case.keyed.then(|| {
            (
                crate::endpoint::AUTHORIZATION.to_owned(),
                format!("Bearer {}", self.key),
            )
        });
        let request = Request {
            method: Method::Post,
            url,
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
        Ok(response)
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
    /// tried again while the upstream is not answering within its health's limit; the
    /// administrator's password it left in the adapter's `upstream.json`, and the last
    /// answer the adapter gave to each address the first run asked.
    async fn first_run(
        &self,
        reached: &Reached,
    ) -> Result<(String, BTreeMap<String, Response>), String> {
        let tapped = Arc::new(Tapped::through(Arc::clone(&reached.adapter)));
        let source = lemonfiber_contract::capabilities::identity::source::Adapter(Contracted::new(
            Arc::clone(&tapped) as Arc<dyn Http>,
            &reached.base,
            &self.adapter.placed.service,
            self.key,
        ));
        let keep = |password: &str| {
            filling::handed_over(
                self.held,
                &[
                    (
                        UPSTREAM_USERNAME.to_owned(),
                        crate::config::MEDIA_SERVER_ADMIN_USER.to_owned(),
                    ),
                    (UPSTREAM_PASSWORD.to_owned(), password.to_owned()),
                ],
            )
        };
        let until = tokio::time::Instant::now() + allowed(self.health);
        loop {
            match crate::seed::wire_media_server_admin(
                &source,
                self.recorder.random,
                None,
                false,
                &keep,
            )
            .await
            {
                Err(crate::seed::State::Skipped { reason })
                    if reason == crate::seed::NOT_ANSWERING
                        && tokio::time::Instant::now() < until =>
                {
                    tokio::time::sleep(POLL).await;
                }
                left => {
                    return left
                        .map(|password| (password, tapped.heard()))
                        .map_err(unfinished)
                }
            }
        }
    }
}

/// The string at each of `secret`'s pointers in `answer`, where it holds one.
fn issued(secret: &[&str], answer: &Value) -> Vec<String> {
    secret
        .iter()
        .filter_map(|pointer| answer.pointer(pointer).and_then(Value::as_str))
        .map(str::to_owned)
        .collect()
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
