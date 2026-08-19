//! Wiring the services to each other.
//!
//! Two gates before anything is written: a service that is not answering is
//! skipped rather than failed, so the whole run stays resumable; and a value the
//! operator changed themselves is preserved rather than reverted.
//!
//! That second gate is what makes seeding safe to run against a stack somebody
//! has tuned by hand, and it is the resolution of the standing tension between
//! reproducible and customised.
//!
//! Every write is read back to confirm it landed, and recorded as a change. That
//! record is groundwork, not a live undo: it captures what a future service-side
//! reversal would need — the resource created and the id the service gave it — but
//! seeding itself is idempotent, so re-running it is how a partial run is
//! recovered, and nothing reverses a seed today. The journal each pass records
//! into is therefore not persisted; `recover`'s reversal is for an interrupted
//! setup apply, and cannot remove a resource a service created in any case.
//!
//! An interruption — a killed run, a service dropping mid-pass — therefore leaves
//! every connection already made intact and valid: each was written straight to
//! the service and read back before it was called done, and each is independent of
//! the rest, so a later run finds the completed ones already present, matched by
//! their own details, and leaves them untouched while it makes the ones the
//! interruption never reached. No connection depends on a later one having landed.
//!
//! A pass against a healthy stack is quick, well inside the minute the setup owes:
//! it is a bounded set of calls, one small group per service, with no wait or retry
//! inside a pass — a service that is not ready is skipped and left to the next run,
//! not polled until it comes up. Each call is bounded by the transport's own connect
//! and request timeouts, so even a service that hangs costs only a bounded wait —
//! a timeout per call it does not answer — before it is set aside rather than
//! stalling the run without end. Against a healthy stack every call answers at once,
//! so a pass's time tracks the number of services; only an unhealthy one pays those
//! timeouts to discover it is not answering.
//!
//! The policy — given what was observed about a connection, what seed intends —
//! is pure and settled without a service. The driver carries it out: it observes
//! the service through the port, registers what is missing, reads it back before
//! calling it done, and records each write as a change. The driver reaches the
//! outside only through the port, so it too runs against a fake.

mod clients;
mod drift;
mod report;
mod roots;
mod services;

pub use clients::{client_field, wire_download_clients, Baselines, CLIENT};
pub(crate) use drift::observe_client;
use drift::{canonical_root, same_base_url, same_path};
pub use drift::{intent, reconcile, same_endpoint, wholesale_drift, Intent, Observed};
use report::{observe_or_skip, record_outcome, unreached};
pub use report::{Assessment, Report, Severity, State, Wiring};
pub use roots::{contested_roots, wire_root_folders};
pub use services::{wire_applications, wire_jellyfin_identity, wire_qbittorrent_password};

use std::collections::BTreeMap;
use std::future::Future;

use crate::baseline::Baseline;
use crate::journal::{Change, Journal, Kind};
use crate::ports::random::Random;
use crate::ports::service::{
    AppSync, Application, Client, DownloadClient, Failure, MediaServer, RegisteredClient, Requests,
    RootFolder,
};
use crate::qbittorrent::Qbittorrent;

/// The administrator account name lemonfiber creates on the media server and
/// signs Seerr in with — one source of truth, so a trace's later library read
/// authenticates under the same name it was created with.
const ADMIN: &str = crate::config::JELLYFIN_ADMIN_USER;

/// Register one connection, confirm it landed by reading the list back, and
/// record it as a change — the shared body of wiring a folder, a download client,
/// or an application.
///
/// The three differ only in the calls that register and read the list back, how a
/// landed row is matched to what was wanted and its id read off, and the nouns for
/// the journal and the read-back failure; those are the parameters, so the
/// register → read-back → confirm → record shape is written once. The two futures
/// are made by the caller and awaited here — register first, the read-back only if
/// it succeeded — which is free, because a future is inert until it is polled and
/// this keeps the ordering, and the two `unreached` gates, in one place.
async fn wire_one<T>(
    register: impl Future<Output = Result<(), Failure>>,
    read_back: impl Future<Output = Result<Vec<T>, Failure>>,
    landed: impl FnOnce(&[T]) -> Option<String>,
    naming: Naming<'_>,
    journal: &mut Journal,
    at: &str,
) -> State {
    if let Err(failure) = register.await {
        return unreached(&failure);
    }
    let registered = match read_back.await {
        Ok(rows) => rows,
        Err(failure) => return unreached(&failure),
    };
    match landed(&registered) {
        Some(id) => {
            journal.record(Change {
                at: at.to_owned(),
                operation: "seed".to_owned(),
                target: naming.service.to_owned(),
                kind: Kind::Created {
                    resource: naming.resource.to_owned(),
                    id,
                },
            });
            State::Wired
        }
        None => State::Failed {
            detail: format!(
                "the {} was accepted but did not appear when read back",
                naming.noun
            ),
        },
    }
}

/// How a wired connection is named where it is recorded and where it is missed —
/// the three labels that are all that differ between wiring a folder, a client and
/// an application once the register-and-read-back shape is shared.
struct Naming<'a> {
    /// The service the connection is recorded against.
    service: &'a str,
    /// The resource kind, as the journal stores it — `rootfolder`, and so on.
    resource: &'a str,
    /// The noun a read-back miss names the connection by, for the operator.
    noun: &'a str,
}
