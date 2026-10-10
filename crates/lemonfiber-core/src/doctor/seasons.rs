//! Whether every series the media server holds opens onto its seasons.

use std::sync::{Arc, OnceLock};

use async_trait::async_trait;
use lemonfiber_contract::capabilities::media::serve;

use super::{Category, Check, Finding, Mend, Verdict};
use crate::error::codes::library::UNSEASONED;
use crate::error::{Problem, Remedy};
use crate::patience::REFRESH;
use crate::ports::service::{Failure, ItemDetail, SeriesHeld, SERIES_MOST};
use crate::repair::{Attempt, Repair, ASK_FOR_REPAIRS};

/// The name this check's summary carries, and the family each series' finding sits in.
pub(crate) const CHECK: &str = "services.seasons";

/// The heading the summary is read under.
const TITLE: &str = "Every series opens onto its seasons";

/// The media server, as the check finds it.
enum Server {
    /// Nothing fills `media.serve`.
    Absent,
    /// Something fills it and cannot be asked.
    Unanswered,
    /// What fills it, asked over its contract.
    Served(Arc<dyn serve::Fills>),
}

/// Reports each series the media server holds episodes for and answers no seasons for,
/// and has the media server read one afresh.
pub struct SeasonsCheck {
    /// The media server.
    server: Server,
    /// The service filling `media.serve`, where one does.
    service: Option<String>,
    /// The series this check found with episodes and no seasons, once it has run.
    unseasoned: OnceLock<Vec<SeriesHeld>>,
}

impl SeasonsCheck {
    /// A check over the media server `service` fills, asked through `answering` where it
    /// answers.
    #[must_use]
    pub fn new(service: Option<String>, answering: Option<Arc<dyn serve::Fills>>) -> Self {
        let server = match (&service, answering) {
            (None, _) => Server::Absent,
            (Some(_), None) => Server::Unanswered,
            (Some(_), Some(answering)) => Server::Served(answering),
        };
        Self {
            server,
            service,
            unseasoned: OnceLock::new(),
        }
    }

    /// A finding about the media server under `check`.
    fn finding(&self, check: &str, title: &str, verdict: Verdict) -> Finding {
        let finding = Finding::in_category(Category::Services, check, title, verdict);
        match &self.service {
            Some(service) => finding.about(service),
            None => finding,
        }
    }

    /// The series a repair names, where this check found it.
    fn named(&self, repair: &Repair) -> Option<&SeriesHeld> {
        self.unseasoned
            .get()?
            .iter()
            .find(|series| reported_as(series) == repair.check)
    }
}

#[async_trait]
impl Check for SeasonsCheck {
    fn category(&self) -> Category {
        Category::Services
    }

    async fn run(&self) -> Vec<Finding> {
        ran(self).await
    }

    fn mender(&self) -> Option<&dyn Mend> {
        Some(self)
    }
}

#[async_trait]
impl Mend for SeasonsCheck {
    fn repairs(&self, found: &[Finding]) -> Vec<Repair> {
        self.unseasoned
            .get()
            .into_iter()
            .flatten()
            .filter(|series| {
                found.iter().any(|finding| {
                    finding.check == reported_as(series)
                        && matches!(finding.verdict, Verdict::Warn(_))
                })
            })
            .map(|series| Repair {
                check: reported_as(series),
                does: format!(
                    "Have the media server read {} afresh, and wait up to {} seconds for its \
                     seasons",
                    series.title,
                    REFRESH.longest().as_secs()
                ),
                effects: vec![
                    "The media server reads the series' files and details again, which takes \
                     it a while on a large series"
                        .to_owned(),
                ],
                reversible: false,
            })
            .collect()
    }

    fn writes_to(&self, _repair: &Repair) -> Vec<String> {
        self.service.iter().cloned().collect()
    }

    async fn mend(&self, repair: &Repair) -> Attempt {
        mended(self, repair).await
    }
}

/// The name one series' finding carries.
fn reported_as(series: &SeriesHeld) -> String {
    format!("services.seasons.{}", series.id)
}

/// What the media server says of its series.
async fn ran(check: &SeasonsCheck) -> Vec<Finding> {
    let server = match &check.server {
        Server::Absent => {
            return vec![check.finding(
                CHECK,
                TITLE,
                Verdict::Skipped {
                    reason: "nothing on this stack serves what the household watches".to_owned(),
                },
            )]
        }
        Server::Unanswered => {
            return vec![check.finding(
                CHECK,
                TITLE,
                unverified("the media server could not be reached"),
            )]
        }
        Server::Served(server) => server,
    };
    let held = match server.series_held(SERIES_MOST).await {
        Ok(held) => held,
        Err(failure) => {
            return vec![check.finding(
                CHECK,
                TITLE,
                unverified(&format!(
                    "the media server could not be asked how its series are filed: {failure}"
                )),
            )]
        }
    };
    let unseasoned: Vec<SeriesHeld> = held
        .iter()
        .filter(|series| series.seasons == 0 && series.episodes > 0)
        .cloned()
        .collect();
    let findings = if unseasoned.is_empty() {
        vec![check.finding(
            CHECK,
            TITLE,
            Verdict::Pass {
                note: Some(read(held.len())),
            },
        )]
    } else {
        unseasoned
            .iter()
            .map(|series| {
                check.finding(
                    &reported_as(series),
                    &format!("The seasons of {}", series.title),
                    Verdict::Warn(unseasoned_problem(series)),
                )
            })
            .collect()
    };
    let _ = check.unseasoned.set(unseasoned);
    findings
}

/// What a reading of every series that found nothing wrong says.
fn read(series: usize) -> String {
    match series {
        0 => "the media server holds no series".to_owned(),
        most if most >= SERIES_MOST as usize => format!(
            "each of the first {most} series the media server lists opens onto its seasons, \
             and it lists more than were read"
        ),
        all => format!("each of the {all} series the media server holds opens onto its seasons"),
    }
}

/// A check that could not be made, and why.
fn unverified(reason: &str) -> Verdict {
    Verdict::Unverified {
        reason: reason.to_owned(),
        remedy: Remedy::new("Check the media server is up and has finished starting")
            .with_detail("lemonfiber status"),
    }
}

/// A series holding episodes with no seasons the media server answers for it.
fn unseasoned_problem(series: &SeriesHeld) -> Problem {
    Problem::new(
        UNSEASONED,
        format!(
            "{} holds {} episode{} and the media server answers no seasons for it",
            series.title,
            series.episodes,
            crate::plural::s(series.episodes as usize)
        ),
        "Somebody choosing it sees the title and nothing to play under it",
        Remedy::new("Have the media server read the series afresh").with_detail(ASK_FOR_REPAIRS),
    )
}

/// Having the media server read one series afresh, and waiting for its seasons.
async fn mended(check: &SeasonsCheck, repair: &Repair) -> Attempt {
    let (Server::Served(server), Some(series)) = (&check.server, check.named(repair)) else {
        return Attempt::Stopped {
            leaving: "no series this check found answers to that repair, so nothing was asked \
                      of the media server"
                .to_owned(),
        };
    };
    if let Err(failure) = server.refresh(&series.id).await {
        return Attempt::Stopped {
            leaving: format!(
                "the media server would not take the request to read {} afresh, and nothing \
                 changed: {failure}",
                series.title
            ),
        };
    }
    let _ = REFRESH
        .until(|| server.title(None, &series.id), opens)
        .await;
    Attempt::carried()
}

/// Whether an answer about a title has seasons in it.
fn opens(answer: &Result<Option<ItemDetail>, Failure>) -> bool {
    matches!(answer, Ok(Some(title)) if !title.seasons.is_empty())
}

#[cfg(test)]
mod tests;
