//! Every request a manifest names a recording for, asked of its service and written.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use lemonfiber_plugin::vocabulary::Credential;
use lemonfiber_plugin::{Manifest, Request};
use tokio::time::Instant;

use super::first::Presented;
use super::served::{answers, POLL};
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
/// and each contributed check's, in that order.
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
            items.push(Item {
                named: format!("claim {} probe {}", claim.capability, probe.id),
                service: service.clone(),
                request: &probe.request,
                fixture: Some(probe.fixture.as_str()),
                keyed: operators(&claim.capability, &probe.id),
            });
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

/// Every request recording the manifest names, each service's taken whole or not at all,
/// and each service that failed.
pub(super) async fn taken(
    manifest: &Manifest,
    root: &Path,
    recorder: Recorder<'_>,
) -> (Vec<Taken>, Vec<Failed>) {
    let items = items(manifest);
    let mut answered: BTreeMap<&str, std::vec::IntoIter<Took>> = BTreeMap::new();
    let mut failed = Vec::new();
    for (service, placed) in manifest
        .services
        .iter()
        .zip(Installed::of(manifest).services)
    {
        let mine: Vec<&Item<'_>> = items
            .iter()
            .filter(|item| recordable(item) == Some(service.id.as_str()))
            .collect();
        if mine.is_empty() {
            continue;
        }
        let took = match recorded(root, recorder, &placed, service.health.as_ref(), &mine).await {
            Ok(written) => written,
            Err(failure) => {
                let unrecorded = Took::Unrecorded {
                    why: format!(
                        "{} was not recorded: it failed at {}",
                        failure.service, failure.step
                    ),
                };
                failed.push(failure);
                std::iter::repeat_n(unrecorded, mine.len()).collect()
            }
        };
        answered.insert(service.id.as_str(), took.into_iter());
    }
    let taken = items
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

/// One service's request recordings, each written once every one was taken.
async fn recorded(
    root: &Path,
    recorder: Recorder<'_>,
    placed: &Placed,
    health: Option<&lemonfiber_plugin::Health>,
    mine: &[&Item<'_>],
) -> Result<Vec<Took>, Failed> {
    for fixture in mine.iter().filter_map(|item| item.fixture) {
        landing(root, fixture).map_err(|why| Failed {
            service: placed.service.clone(),
            step: format!("writing {fixture}"),
            why,
            log: Vec::new(),
        })?;
    }
    let answered = answers(recorder, root, placed, health, mine).await?;
    let pinned = format!("{}@{}", placed.image, placed.digest);
    Ok(mine
        .iter()
        .zip(answered)
        .map(|(item, response)| {
            let fixture = item.fixture.unwrap_or_default();
            written(
                root,
                fixture,
                &pinned,
                Of::Request {
                    request: Asked {
                        method: item.request.method.clone(),
                        path: item.request.path.clone(),
                        accept: item.request.accept.clone(),
                    },
                    response,
                },
            )
        })
        .collect())
}

/// What the served service answered `item` through `http`, on the loopback interface at
/// `port`, presenting `presented` where there is a credential to present.
pub(super) async fn answered(
    http: &dyn Http,
    port: u16,
    item: &Item<'_>,
    presented: Option<&Presented>,
) -> Result<Answer, String> {
    let asked = item.request;
    let method = crate::plugin::judging::method(&asked.method)
        .ok_or_else(|| format!("{} is not a method lemonfiber can send", asked.method))?;
    if !lemonfiber_plugin::refusing::carried::is_route(&asked.path) {
        return Err(format!("{} is not a route on the service", asked.path));
    }
    let mut url = format!("http://{LOOPBACK}:{port}{}", asked.path);
    let mut headers = Vec::new();
    if let Some(accept) = &asked.accept {
        headers.push(("Accept".to_owned(), accept.clone()));
    }
    match presented {
        Some(Presented::Header(name, value)) => headers.push(((*name).to_owned(), value.clone())),
        Some(Presented::Query(name, value)) => {
            let joined = if asked.path.contains('?') { '&' } else { '?' };
            url = format!(
                "{url}{joined}{name}={}",
                crate::endpoint::query_encoded(value)
            );
        }
        Some(Presented::Session) | None => {}
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
    let json = serde_json::from_str(&response.body).ok();
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
