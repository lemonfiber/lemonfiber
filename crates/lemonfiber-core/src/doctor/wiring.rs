//! Whether each download client still sits where lemonfiber wired it.
//!
//! The one place an operator and lemonfiber write to the same field. lemonfiber tells each
//! \*arr which category to file its downloads under; the operator can open the same page
//! and change it. Both are legitimate, and the difference between them is not visible in
//! the value — only in what lemonfiber last recorded for it.
//!
//! So this reads three things, not two: what lemonfiber recorded, what the service holds
//! now, and what lemonfiber would write. Two of them can only say *different*; three say
//! which side moved. lemonfiber's own value fallen behind its intent is lemonfiber's to
//! bring up to date. The operator's edit is theirs, and stays theirs — reported here so
//! they know the stack is not filing where the baseline says, never quietly reverted.
//!
//! Seeding does the same comparison when it wires. This is the read-only half, run by a
//! diagnosis that changes nothing, and the two share the comparison itself rather than
//! each having an opinion about the same field.

use std::sync::Arc;

use async_trait::async_trait;

use super::credentials::Target;
use super::{Category, Check, Finding, Mend, Verdict};
use crate::baseline::Record;
use crate::error::{Code, Problem, Remedy, Severity};
use crate::ports::filesystem::FileSystem;
use crate::ports::http::Http;
use crate::ports::service::{Client as _, DownloadClient, RegisteredClient};
use crate::seed::{observe_client, same_endpoint, Observed};

mod mender;

pub(crate) use mender::WiringMender;

/// Raised when a download client no longer files where lemonfiber wired it.
pub const DRIFTED: Code = Code::new("WIRING-1");

/// The stem every wiring finding is named from. The service and the client follow it, so
/// two clients drifting in one \*arr are two findings to answer rather than one.
const CHECK: &str = "config.download-client";

/// The download-client wiring of one \*arr: how to reach it, and the clients lemonfiber
/// manages there.
///
/// Held rather than read at assembly, so the check opens the service and reads what it
/// holds at the moment it runs. A check that captured the value when it was built would
/// compare a repair's work against the very reading the repair changed.
pub struct Managed {
    /// The \*arr, and how to authenticate to it.
    pub target: Target,
    /// The clients lemonfiber wired into it, with what it last recorded for each.
    pub clients: Vec<Wired>,
}

/// One managed download client: what lemonfiber would write there, and what it last
/// recorded having written.
pub struct Wired {
    /// The client as lemonfiber would register it.
    pub want: DownloadClient,
    /// What lemonfiber last recorded for it, absent where it has recorded nothing.
    pub recorded: Option<Record>,
}

impl Wired {
    /// The name this wiring's finding and its repair share.
    ///
    /// Carries the service and the client, so a repair offered for one drift is not
    /// answered by another \*arr's.
    #[must_use]
    pub fn check(&self, service: &str) -> String {
        format!("{CHECK}:{service}:{}", self.want.name.to_lowercase())
    }
}

/// How a wiring is read: the transport to reach a service on, and the files its key is
/// written to.
///
/// Held as one thing because every read of a wiring needs both, and because the check and
/// the repair read the same way — the repair deliberately re-reading rather than trusting
/// what the diagnosis saw.
#[derive(Clone)]
pub(crate) struct Reading {
    http: Arc<dyn Http>,
    filesystem: Arc<dyn FileSystem>,
}

impl Reading {
    /// The clients a service holds, or nothing where it could not be asked.
    ///
    /// Nothing covers both "the key is not written yet" and "the service would not
    /// answer". Neither is a drift, and neither is a pass; the caller reports both as
    /// unverified.
    pub(crate) async fn held(&self, managed: &Managed) -> Option<Vec<RegisteredClient>> {
        self.open(managed).await?.download_clients().await.ok()
    }

    /// The service itself, authenticated, or nothing where its key cannot be read yet.
    pub(crate) async fn open(&self, managed: &Managed) -> Option<crate::servarr::Servarr> {
        managed
            .target
            .open(&self.http, self.filesystem.as_ref())
            .await
    }
}

/// Reports where a download client has stopped filing where lemonfiber wired it, and which
/// side of the field moved.
pub struct WiringCheck {
    reading: Reading,
    managed: Arc<Vec<Managed>>,
    mender: WiringMender,
}

impl WiringCheck {
    /// A check over the wirings given, reading each through `http` and the key files on
    /// `filesystem`.
    #[must_use]
    pub fn new(
        http: Arc<dyn Http>,
        filesystem: Arc<dyn FileSystem>,
        managed: Vec<Managed>,
        stamp: String,
    ) -> Self {
        let managed = Arc::new(managed);
        let reading = Reading { http, filesystem };
        Self {
            mender: WiringMender::new(reading.clone(), Arc::clone(&managed), stamp),
            reading,
            managed,
        }
    }

    /// What one \*arr's managed clients look like now.
    ///
    /// The service is opened and asked once, however many clients lemonfiber manages
    /// there: they are all rows of the same answer, and asking per client would be the
    /// same question repeated.
    async fn examine(&self, managed: &Managed) -> Vec<Finding> {
        // Opened once and asked twice: what it holds, and — only where that shows an edit
        // — whether it can still reach the client. Opening reads a key off disk, so a
        // second open would be a second read for an answer already in hand.
        let service = self.reading.open(managed).await;
        let held = match &service {
            Some(service) => service.download_clients().await.ok(),
            None => None,
        };
        // A drift is the operator's own choice and reads as information; what raises it to
        // a fault is the service saying it can no longer reach the client. Asked only where
        // an edit was actually found — a check that ran the service's own test on every
        // healthy run would be spending it to learn nothing.
        let broken = match (&service, held.as_deref()) {
            (Some(service), Some(held)) => broken_clients(managed, held, service).await,
            _ => Vec::new(),
        };
        managed
            .clients
            .iter()
            .map(|wired| {
                let name = wired.check(&managed.target.id);
                let title = format!(
                    "{} files {} downloads where lemonfiber wired it",
                    managed.target.name, wired.want.name
                );
                Finding::in_category(
                    Category::Config,
                    &name,
                    &title,
                    verdict(managed, wired, held.as_deref(), &broken),
                )
                // The service whose wiring this is about — so a drifted setting on a
                // service that is itself in trouble reads as one problem, and a failure
                // here is quoted with what that service said for itself.
                .about(&managed.target.id)
            })
            .collect()
    }
}

#[async_trait]
impl Check for WiringCheck {
    fn category(&self) -> Category {
        Category::Config
    }

    async fn run(&self) -> Vec<Finding> {
        let mut findings = Vec::new();
        for managed in self.managed.iter() {
            findings.extend(self.examine(managed).await);
        }
        findings
    }

    fn mender(&self) -> Option<&dyn Mend> {
        Some(&self.mender)
    }
}

/// The ids of the edited clients this service can no longer reach.
///
/// Empty where no client was edited, or where the service will not run the test — in both,
/// nothing has been *proven* broken, and an edit stays the information it already was.
async fn broken_clients(
    managed: &Managed,
    held: &[RegisteredClient],
    service: &crate::servarr::Servarr,
) -> Vec<String> {
    let edited: Vec<&Wired> = managed
        .clients
        .iter()
        .filter(|wired| {
            matches!(
                observed(wired, held),
                Observed::Drifted | Observed::Conflicted
            )
        })
        .collect();
    if edited.is_empty() {
        return Vec::new();
    }
    let Ok(probes) = service.test_download_clients().await else {
        return Vec::new();
    };
    edited
        .into_iter()
        .filter_map(|wired| holding(held, &wired.want))
        .filter(|have| {
            probes
                .iter()
                .any(|probe| probe.id == have.id && !probe.reachable)
        })
        .map(|have| have.id.clone())
        .collect()
}

/// What the service holds for one wanted client — the client it matches by endpoint, and
/// the category on it.
///
/// Matched by the endpoint it reaches rather than by its label, as seeding does, so a
/// client the operator renamed is the same connection rather than a missing one.
pub(crate) fn holding<'a>(
    held: &'a [RegisteredClient],
    want: &DownloadClient,
) -> Option<&'a RegisteredClient> {
    held.iter().find(|have| same_endpoint(have, want))
}

/// What lemonfiber sees for one managed client, read from the three values.
///
/// The same observation seeding makes, rather than a second one: it knows a category is a
/// category, so a value the service normalises on write is not read as an edit the
/// operator never made — and two comparisons of the same field would eventually disagree
/// about exactly the thing an operator is trusting lemonfiber not to get wrong.
fn observed(wired: &Wired, held: &[RegisteredClient]) -> Observed {
    observe_client(
        holding(held, &wired.want),
        &wired.want,
        wired.recorded.as_ref(),
    )
}

/// How one managed client turned out.
fn verdict(
    managed: &Managed,
    wired: &Wired,
    held: Option<&[RegisteredClient]>,
    broken: &[String],
) -> Verdict {
    let Some(held) = held else {
        return Verdict::Unverified {
            reason: format!("{} could not be asked what it holds", managed.target.name),
            remedy: Remedy::new("Check the service is up and has finished starting")
                .with_detail("lemonfiber status"),
        };
    };
    match observed(wired, held) {
        // Nothing wired there yet, so there is no managed value to have drifted. Wiring it
        // is seeding's errand, and reporting it here would be a second voice on it.
        //
        // `Unavailable` cannot arrive here — it is what a seed pass says about a service
        // that would not answer, and a service that will not answer this one has already
        // been reported unverified above.
        Observed::Absent | Observed::Unavailable => Verdict::Skipped {
            reason: format!(
                "{} is not wired into {} yet",
                wired.want.name, managed.target.name
            ),
        },
        Observed::Present => Verdict::Pass {
            note: Some(format!(
                "filing under {}",
                wired.want.category.value.clone()
            )),
        },
        Observed::Adopted | Observed::Unmanaged => Verdict::Pass {
            note: Some(format!(
                "filing where you set it, in {}",
                managed.target.name
            )),
        },
        Observed::Stale => Verdict::Warn(stale(managed, wired)),
        // The operator's own edit. Said, and no more than said — until the service tells
        // us it can no longer reach the client, at which point what they changed has
        // stopped the stack working and is worth putting to them as a fault.
        Observed::Drifted | Observed::Conflicted => {
            if holding(held, &wired.want).is_some_and(|have| broken.contains(&have.id)) {
                Verdict::Warn(unreachable(managed, wired))
            } else {
                Verdict::Pass {
                    note: Some(format!(
                        "filing under the category you set, not lemonfiber's {}",
                        wired.want.category.value
                    )),
                }
            }
        }
    }
}

/// lemonfiber's own value, fallen behind what lemonfiber now intends.
fn stale(managed: &Managed, wired: &Wired) -> Problem {
    Problem::new(
        DRIFTED,
        Severity::Warning,
        format!(
            "{} files {} downloads under a category lemonfiber has since moved on from",
            managed.target.name, wired.want.name
        ),
        "New downloads land under the old category, so anything filed since is somewhere \
         the rest of the stack no longer looks",
        Remedy::new("Bring the category up to what lemonfiber now files under")
            .with_detail(crate::repair::ASK_FOR_REPAIRS),
    )
}

/// The operator's own change, where the service can no longer reach what it points at.
///
/// The one shape of edit worth raising. Their value stands either way — but a client the
/// service cannot reach downloads nothing, and an operator watching a queue that never
/// empties is owed the connection between the two.
fn unreachable(managed: &Managed, wired: &Wired) -> Problem {
    Problem::new(
        DRIFTED,
        Severity::Warning,
        format!(
            "{} cannot reach {}, which you moved off lemonfiber's category",
            managed.target.name, wired.want.name
        ),
        "Nothing downloads through a client the service cannot reach, so the queue fills \
         and never empties",
        Remedy::new("Keep your value and make the client reachable, or put lemonfiber's back")
            .with_detail(
            "lemonfiber seed --adopt keeps yours; lemonfiber reset --confirm restores lemonfiber's",
        ),
    )
}
