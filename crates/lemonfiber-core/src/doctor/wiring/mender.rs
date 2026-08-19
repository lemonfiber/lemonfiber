//! Putting a download client back on the category lemonfiber files under.
//!
//! The first repair that touches lemonfiber's own configuration, and so the first that has
//! to answer the question the rest of this feature was built around: whether the field it
//! would write is lemonfiber's to write. It reads the baseline and the service, and only a
//! value lemonfiber wrote *and* the service still holds is one it will write again.
//!
//! What it changes it records, in the form a reversal reads — so an operator who watched
//! it happen can put it back.

use std::sync::Arc;

use async_trait::async_trait;

use crate::doctor::{Finding, Mend, Verdict};
use crate::error::Diagnose as _;
use crate::journal::{Change, Kind};
use crate::ports::service::Client as _;
use crate::repair::{may_write, Attempt, Repair, Writing, OPERATION};

use crate::seed::CLIENT;

use super::{holding, Managed, Reading, Wired};

/// Bringing one \*arr's download client back to the category lemonfiber files under.
///
/// Holds the same wirings the check does, and reads them the same way: the service is
/// asked afresh when the repair runs, because a category can move between looking and
/// acting and writing over what somebody changed in between is the one thing this must
/// not do.
pub(crate) struct WiringMender {
    reading: Reading,
    managed: Arc<Vec<Managed>>,
    /// When a change made here is recorded as happening — the run's own stamp, so every
    /// change one repair makes carries the same one and a reversal can tell them apart
    /// from the repair before.
    stamp: String,
}

impl WiringMender {
    /// A mender over the wirings given, writing through `http`.
    pub(crate) fn new(reading: Reading, managed: Arc<Vec<Managed>>, stamp: String) -> Self {
        Self {
            reading,
            managed,
            stamp,
        }
    }

    /// The wiring a repair names, or nothing where it names none of these.
    fn named(&self, repair: &Repair) -> Option<(&Managed, &Wired)> {
        self.managed.iter().find_map(|managed| {
            managed
                .clients
                .iter()
                .find(|wired| wired.check(&managed.target.id) == repair.check)
                .map(|wired| (managed, wired))
        })
    }
}

#[async_trait]
impl Mend for WiringMender {
    fn repairs(&self, found: &[Finding]) -> Vec<Repair> {
        self.managed
            .iter()
            .flat_map(|managed| {
                managed.clients.iter().map(move |wired| Repair {
                    check: wired.check(&managed.target.id),
                    does: format!(
                        "Put {}'s {} back on the category lemonfiber files under",
                        managed.target.name, wired.want.name
                    ),
                    effects: vec![format!(
                        "Downloads already filed under the old category stay where they are, \
                         so {} may need a rescan to find them",
                        managed.target.name
                    )],
                    // The change is journalled, so `lemonfiber doctor --undo` can put the
                    // category back exactly as it was.
                    reversible: true,
                })
            })
            .filter(|repair| {
                found.iter().any(|finding| {
                    finding.check == repair.check
                        && matches!(finding.verdict, Verdict::Warn(_) | Verdict::Fail(_))
                })
            })
            .collect()
    }

    /// Whether this field is lemonfiber's to write.
    ///
    /// Read from the service rather than from what the diagnosis saw. A run that looked,
    /// asked, and was answered gives the operator time to change the very thing it is
    /// about to write — and answering from the older reading would write over the change
    /// they made while being asked.
    async fn may_proceed(&self, repair: &Repair) -> Writing {
        let Some((managed, wired)) = self.named(repair) else {
            return Writing::TheirsAlone;
        };
        let Some(held) = self.reading.held(managed).await else {
            // The service will not say what it holds, so nothing can establish the value
            // is still lemonfiber's. Silence is not permission.
            return Writing::TheirsAlone;
        };
        let holds = holding(&held, &wired.want)
            .and_then(|have| have.category.as_ref())
            .map(|category| category.value.as_str());
        may_write(wired.recorded.as_ref(), holds)
    }

    async fn mend(&self, repair: &Repair) -> Attempt {
        let Some((managed, wired)) = self.named(repair) else {
            return Attempt::Stopped {
                leaving: "the wiring this repair names is no longer one lemonfiber manages"
                    .to_owned(),
            };
        };
        let Some(client) = self.reading.open(managed).await else {
            return Attempt::Stopped {
                leaving: format!(
                    "{} could not be authenticated to, so it was left as it was",
                    managed.target.name
                ),
            };
        };
        // Read again rather than trusting what the diagnosis saw: the client is written by
        // the id the service assigned it, and an id read a moment ago is an id that may
        // have been removed since.
        let Ok(held) = client.download_clients().await else {
            return Attempt::Stopped {
                leaving: format!(
                    "{} would not say what it holds, so nothing was written",
                    managed.target.name
                ),
            };
        };
        let Some(have) = holding(&held, &wired.want) else {
            return Attempt::Stopped {
                leaving: format!(
                    "{} no longer holds {}, so there was nothing to put back",
                    managed.target.name, wired.want.name
                ),
            };
        };
        let previous = have
            .category
            .as_ref()
            .map(|category| category.value.clone());
        match client.update_download_client(&have.id, &wired.want).await {
            // Recorded as a change *inside the service*, not as a setting in
            // lemonfiber's environment file. The two read alike and are reversed nothing
            // alike, and a reversal that took this for a `Set` would write the field's
            // name into the environment file and leave the service exactly as it was.
            Ok(()) => Attempt::recorded(vec![Change {
                at: self.stamp.clone(),
                operation: OPERATION.to_owned(),
                target: managed.target.id.clone(),
                kind: Kind::Configured {
                    resource: CLIENT.to_owned(),
                    id: have.id.clone(),
                    field: wired.want.category.field.clone(),
                    previous,
                    current: wired.want.category.value.clone(),
                },
            }]),
            Err(failure) => Attempt::Stopped {
                leaving: format!(
                    "{} would not take the category, and kept the one it had — {}",
                    managed.target.name,
                    failure.problem().summary
                ),
            },
        }
    }
}
