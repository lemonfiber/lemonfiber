//! What the core keeps of where a message goes, as the read answers it.

use url::Url;

use super::backends::{self, Backend};
use super::configuring::{nowhere, read_failure};
use crate::app::Ctx;
use crate::error::Problem;
use crate::model::{
    BackendProof, BackendReport, DeliveryOutcome, HostedBackend, NotifyReport, NotifyState,
};

/// What leaves for a hosted back-end, in the operator's words.
const LEAVES: [&str; 2] = [
    "the kind of each alert and its summary, as the stack words it",
    "this machine's address, each time a message is delivered",
];

/// Every back-end the stack delivers to, its phones' push endpoints among them.
///
/// # Errors
///
/// A [`Problem`] where there is nowhere to keep back-ends, or their record cannot be read.
pub(crate) fn report(ctx: &Ctx) -> Result<NotifyReport, Box<Problem>> {
    let path = backends::at(ctx).ok_or_else(nowhere)?;
    let kept = backends::kept(&path).map_err(|why| read_failure(&why))?;
    let mut listed: Vec<BackendReport> = kept.backends.iter().map(listed).collect();
    listed.extend(crate::push::run::as_backends(ctx));
    Ok(NotifyReport {
        state: state(&listed),
        backends: listed,
        unread: Vec::new(),
        rehearsed: ctx.dry_run,
    })
}

/// One back-end as the read lists it.
fn listed(backend: &Backend) -> BackendReport {
    let reaches = match &backend.topic {
        Some(topic) => format!("{}/{topic}", backend.url),
        None => backend.url.clone(),
    };
    BackendReport {
        id: backend.id.clone(),
        kind: backend.kind,
        reaches,
        chosen_by: backend.chosen_by.clone(),
        hosted: backend.hosted.then(|| HostedBackend {
            host: Url::parse(&backend.url)
                .ok()
                .and_then(|url| url.host_str().map(str::to_owned))
                .unwrap_or_default(),
            leaves: LEAVES.iter().map(|one| (*one).to_owned()).collect(),
        }),
        proof: backend.proof.clone(),
        last: backend.last.clone(),
        failures: backend.failures.clone(),
        retry: backend.retry.clone(),
    }
}

/// Where delivery as a whole stands, from every back-end listed.
pub(crate) fn state(listed: &[BackendReport]) -> NotifyState {
    if listed.is_empty() {
        return NotifyState::Unconfigured;
    }
    let failing = |one: &BackendReport| {
        matches!(one.proof, BackendProof::Held(_))
            || one
                .last
                .as_ref()
                .is_some_and(|last| matches!(last.outcome, DeliveryOutcome::Failed(_)))
    };
    if listed.iter().all(failing) {
        return NotifyState::Unreachable;
    }
    let proven = listed
        .iter()
        .any(|one| matches!(one.proof, BackendProof::Proven(_)) && !failing(one));
    if proven && !listed.iter().any(failing) {
        NotifyState::Ready
    } else {
        NotifyState::Degraded
    }
}
