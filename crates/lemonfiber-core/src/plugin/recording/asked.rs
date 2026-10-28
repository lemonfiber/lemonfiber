//! Every request a manifest names a recording for, asked of its service and written.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use lemonfiber_manifest::container::Secret;
use lemonfiber_plugin::vocabulary::Credential;
use lemonfiber_plugin::{Manifest, Request};
use tokio::time::Instant;

use super::fronted::{self, Fronting};
use super::served::{answers, POLL};
use super::stood_in;
use super::written::{landing, written};
use super::{Failed, Recorder, Taken, Took};
use crate::plugin::container::LOOPBACK;
use crate::plugin::installed::{Installed, Placed};
use crate::plugin::recorded::{Answer, Asked, Of};
use crate::ports::http::{Http, Response};

/// How long one request may take to be answered.
const ANSWERED_WITHIN: Duration = Duration::from_secs(30);

/// The one header a recording keeps.
const CONTENT_TYPE: &str = "content-type";

/// One request a manifest names a recording for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Item<'a> {
    /// What names it.
    pub(super) named: String,
    /// The service it asks, where the manifest settles one.
    pub(super) service: Option<String>,
    /// The request.
    pub(super) request: &'a Request,
    /// The recording it names.
    pub(super) fixture: Option<&'a str>,
    /// Whether it is asked with the operator's credential.
    pub(super) keyed: bool,
}

/// Every request recording `manifest` names: each request proof's, each claim probe's
/// that is a request and each contributed check's, in that order.
pub(super) fn items(manifest: &Manifest) -> Vec<Item<'_>> {
    let mut items = Vec::new();
    for proof in &manifest.proofs {
        if let Some(request) = &proof.request {
            items.push(Item {
                named: format!("proof {}", proof.id),
                service: manifest
                    .asks(proof.service.as_deref())
                    .map(|one| one.id.clone()),
                request,
                fixture: proof.fixture.as_deref(),
                keyed: false,
            });
        }
    }
    for claim in &manifest.claims {
        let service = manifest
            .services
            .iter()
            .find(|service| service.provides.contains(&claim.capability))
            .map(|service| service.id.clone());
        for probe in &claim.probes {
            if let Some(request) = &probe.request {
                items.push(Item {
                    named: format!("claim {} probe {}", claim.capability, probe.id),
                    service: service.clone(),
                    request,
                    fixture: Some(probe.fixture.as_str()),
                    keyed: operators(&claim.capability, &probe.id),
                });
            }
        }
    }
    for check in &manifest.contributions {
        if let Some(request) = &check.request {
            items.push(Item {
                named: format!("check {}", check.id),
                service: manifest
                    .asks(check.service.as_deref())
                    .map(|one| one.id.clone()),
                request,
                fixture: check.fixture.as_deref(),
                keyed: false,
            });
        }
    }
    items
}

/// Whether the vocabulary asks the probe `probe` of `capability` as the operator.
fn operators(capability: &str, probe: &str) -> bool {
    lemonfiber_plugin::vocabulary::carried()
        .iter()
        .filter(|carried| carried.name == capability)
        .flat_map(|carried| carried.probes)
        .any(|one| one.id == probe && one.credential == Credential::Operator)
}

/// Every request recording the manifest names, and every conformance case of each
/// contract an adapter of the plugin speaks, each service's taken whole or not at all;
/// and each service that failed.
pub(super) async fn taken(
    manifest: &Manifest,
    root: &Path,
    recorder: Recorder<'_>,
) -> (Vec<Taken>, Vec<Failed>) {
    let items = items(manifest);
    let placements = Installed::of(manifest).services;
    let pairs: Vec<(&lemonfiber_plugin::Service, &Placed)> =
        manifest.services.iter().zip(placements.iter()).collect();
    let mut answered: BTreeMap<&str, std::vec::IntoIter<Took>> = BTreeMap::new();
    let mut conformance = Vec::new();
    let mut failed = Vec::new();
    for (service, placed) in &pairs {
        let mine: Vec<&Item<'_>> = items
            .iter()
            .filter(|item| recordable(item) == Some(service.id.as_str()))
            .collect();
        let fronting = pairs
            .iter()
            .find(|(speaker, _)| {
                !speaker.speaks.is_empty()
                    && manifest
                        .upstream_of(speaker)
                        .is_some_and(|upstream| upstream.id == service.id)
            })
            .map(|(adapter, placed)| Fronting {
                service: adapter,
                placed,
            });
        if mine.is_empty() && fronting.is_none() {
            continue;
        }
        let ask = Service {
            placed,
            health: service.health.as_ref(),
            fronting,
            secrets: &manifest.secrets,
        };
        let took = match recorded(root, recorder, ask, &mine).await {
            Ok((written, cases)) => {
                conformance.extend(cases);
                written
            }
            Err(failure) => {
                let unrecorded = Took::Unrecorded {
                    why: format!(
                        "{} was not recorded: it failed at {}",
                        failure.service, failure.step
                    ),
                };
                conformance.extend(fronting.iter().flat_map(|fronting| {
                    fronted::fixtures(fronting.service, &placed.media_types)
                        .into_iter()
                        .map(|(named, fixture)| Taken {
                            named,
                            fixture: Some(fixture),
                            took: unrecorded.clone(),
                        })
                }));
                failed.push(failure);
                std::iter::repeat_n(unrecorded, mine.len()).collect()
            }
        };
        answered.insert(service.id.as_str(), took.into_iter());
    }
    let mut taken: Vec<Taken> = items
        .iter()
        .map(|item| Taken {
            named: item.named.clone(),
            fixture: item.fixture.map(str::to_owned),
            took: recordable(item)
                .and_then(|service| answered.get_mut(service))
                .and_then(Iterator::next)
                .unwrap_or_else(|| Took::Unrecorded {
                    why: unrecorded(item).to_owned(),
                }),
        })
        .collect();
    taken.extend(conformance);
    (taken, failed)
}

/// The service `item` is recorded from, where it names a recording and settles one.
fn recordable<'i>(item: &'i Item<'_>) -> Option<&'i str> {
    item.fixture.and(item.service.as_deref())
}

/// Why `item` is not recorded at all.
const fn unrecorded(item: &Item<'_>) -> &'static str {
    if item.fixture.is_none() {
        "names no fixture, so there is nowhere to write what it answered"
    } else {
        "does not settle which of this plugin's services it asks"
    }
}

/// One service whose recordings are taken together.
#[derive(Clone, Copy)]
struct Service<'a> {
    /// How installing it places it.
    placed: &'a Placed,
    /// The health its manifest declares.
    health: Option<&'a lemonfiber_plugin::Health>,
    /// The adapter of the plugin that fronts it, where one does.
    fronting: Option<Fronting<'a>>,
    /// The secrets the plugin declares.
    secrets: &'a [Secret],
}

/// One service's request recordings and the conformance cases of the adapter fronting
/// it, each written once every one was taken.
async fn recorded(
    root: &Path,
    recorder: Recorder<'_>,
    service: Service<'_>,
    mine: &[&Item<'_>],
) -> Result<(Vec<Took>, Vec<Taken>), Failed> {
    let placed = service.placed;
    let fixtures: Vec<String> = mine
        .iter()
        .filter_map(|item| item.fixture.map(str::to_owned))
        .chain(service.fronting.iter().flat_map(|fronting| {
            fronted::fixtures(fronting.service, &placed.media_types)
                .into_iter()
                .map(|(_, fixture)| fixture)
        }))
        .collect();
    for fixture in &fixtures {
        landing(root, fixture).map_err(|why| Failed {
            service: placed.service.clone(),
            step: format!("writing {fixture}"),
            why,
            log: Vec::new(),
        })?;
    }
    let (answered, cases) = match service.fronting {
        Some(fronting) => {
            fronted::answers(
                recorder,
                root,
                placed,
                service.health,
                fronting,
                mine,
                service.secrets,
            )
            .await?
        }
        None => (
            answers(
                recorder,
                root,
                placed,
                service.health,
                mine,
                service.secrets,
            )
            .await?,
            Vec::new(),
        ),
    };
    let pinned = format!("{}@{}", placed.image, placed.digest);
    let stood_in = stood_in::named(placed, service.secrets);
    let took = mine
        .iter()
        .zip(answered)
        .map(|(item, response)| {
            written(
                root,
                item.fixture.unwrap_or_default(),
                &pinned,
                Of::Request {
                    request: Asked {
                        method: item.request.method.clone(),
                        path: item.request.path.clone(),
                        accept: item.request.accept.clone(),
                    },
                    response,
                },
                recorder.taking,
                &stood_in,
            )
        })
        .collect();
    let cases = cases
        .into_iter()
        .map(|case| {
            let fixture = case.fixture();
            Taken {
                named: case.named(),
                took: written(
                    root,
                    &fixture,
                    &pinned,
                    Of::Request {
                        request: case.asked,
                        response: case.answer,
                    },
                    recorder.taking,
                    &stood_in,
                ),
                fixture: Some(fixture),
            }
        })
        .collect();
    Ok((took, cases))
}

/// What the served service answered `item` through `http`, on the loopback interface at
/// `port`, presenting no credential.
pub(super) async fn answered(
    http: &dyn Http,
    port: u16,
    item: &Item<'_>,
) -> Result<Answer, String> {
    let asked = item.request;
    let method = crate::plugin::judging::method(&asked.method)
        .ok_or_else(|| format!("{} is not a method lemonfiber can send", asked.method))?;
    if !lemonfiber_plugin::refusing::carried::is_route(&asked.path) {
        return Err(format!("{} is not a route on the service", asked.path));
    }
    let url = format!("http://{LOOPBACK}:{port}{}", asked.path);
    let mut headers = Vec::new();
    if let Some(accept) = &asked.accept {
        headers.push(("Accept".to_owned(), accept.clone()));
    }
    let request = crate::ports::http::Request {
        method,
        url,
        headers,
        body: None,
        pinned: None,
    };
    let until = Instant::now() + ANSWERED_WITHIN;
    loop {
        match http.send(&request).await {
            Ok(response) => return Ok(kept(&response)),
            Err(unreachable) if Instant::now() >= until => {
                return Err(format!(
                    "it did not answer within {} seconds: {}",
                    ANSWERED_WITHIN.as_secs(),
                    unreachable.reason
                ))
            }
            Err(_) => tokio::time::sleep(POLL).await,
        }
    }
}

/// What a recording keeps of an answer: its status, its content type, and its body as
/// parsed JSON or as its beginning.
pub(super) fn kept(response: &Response) -> Answer {
    let headers = response
        .headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(CONTENT_TYPE))
        .map(|(_, value)| (CONTENT_TYPE.to_owned(), value.clone()))
        .into_iter()
        .collect();
    let json = serde_json::from_str(&response.body)
        .ok()
        .filter(|json: &serde_json::Value| !json.is_null());
    Answer {
        status: response.status,
        headers,
        body_starts_with: (json.is_none() && !response.body.is_empty())
            .then(|| crate::plugin::program::cut(&response.body).to_owned()),
        json,
    }
}

#[cfg(test)]
mod tests;
