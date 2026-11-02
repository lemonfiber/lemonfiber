//! What the media server holds under the stack's own services' names that lemonfiber did
//! not make or no longer knows.
//!
//! The core mints, lists, dates and revokes a key only as one its filler made, so a key or
//! account somebody else made under one of those names, or one the filler lost its record
//! of, is never touched. This reads them and says where each is removed; nothing acts on
//! the answer.

use std::sync::Arc;

use async_trait::async_trait;

use super::{Category, Check, Finding, Verdict};
use crate::app_keys::MINTED_FOR;
use crate::error::codes::cred::ASTRAY;
use crate::error::{Problem, Remedy};
use crate::ports::service::{AppKeys, Astray, Failure};

/// The name this check's summary carries, and the family each one held sits in.
pub(crate) const CHECK: &str = "services.astray";

/// The heading the summary is read under.
const TITLE: &str = "What the media server holds under the stack's services' names";

/// The media server, as the check finds it.
enum Server {
    /// Nothing fills `media.serve`.
    Absent,
    /// Something fills it and cannot be asked.
    Unanswered,
    /// What fills it, asked over its contract.
    Served(Arc<dyn AppKeys>),
}

/// The media server's service, and what an operator calls it.
pub struct Named {
    /// The id of the service filling `media.serve`.
    pub id: String,
    /// What the stack calls it in front of an operator.
    pub name: String,
}

/// Reports each key or account the media server holds under a name the core mints a key
/// under that its filler did not make or no longer records.
pub struct AstrayCheck {
    /// The media server.
    server: Server,
    /// The service filling `media.serve`, where one does.
    named: Option<Named>,
}

impl AstrayCheck {
    /// A check over the media server `named`, asked through `answering` where it answers.
    #[must_use]
    pub fn new(named: Option<Named>, answering: Option<Arc<dyn AppKeys>>) -> Self {
        let server = match (&named, answering) {
            (None, _) => Server::Absent,
            (Some(_), None) => Server::Unanswered,
            (Some(_), Some(answering)) => Server::Served(answering),
        };
        Self { server, named }
    }

    /// A finding about the media server under `check`.
    fn finding(&self, check: &str, title: &str, verdict: Verdict) -> Finding {
        let finding = Finding::in_category(Category::Services, check, title, verdict);
        match &self.named {
            Some(named) => finding.about(&named.id),
            None => finding,
        }
    }

    /// What an operator calls the media server.
    fn server_name(&self) -> &str {
        self.named
            .as_ref()
            .map_or("the media server", |named| named.name.as_str())
    }
}

#[async_trait]
impl Check for AstrayCheck {
    fn category(&self) -> Category {
        Category::Services
    }

    async fn run(&self) -> Vec<Finding> {
        ran(self).await
    }
}

/// How asking for every name went, apart from what each answered.
enum Asked {
    /// Every name was answered.
    Answered,
    /// The filler answers `astray` unsupported.
    Unsupported(String),
    /// A name could not be answered.
    Unread(String),
}

/// What the media server holds under each name the core mints a key under.
async fn ran(check: &AstrayCheck) -> Vec<Finding> {
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
                unverified(&format!("{} could not be reached", check.server_name())),
            )]
        }
        Server::Served(server) => server,
    };
    let mut findings = Vec::new();
    let mut asked = Asked::Answered;
    for app in MINTED_FOR {
        match server.astray(app).await {
            Ok(held) => findings.extend(held.iter().map(|one| {
                check.finding(
                    &reported_as(app, one),
                    &format!("{} held under {app}", one.name),
                    Verdict::Warn(astray(check.server_name(), app, one)),
                )
            })),
            Err(Failure::Unsupported { .. }) => {
                asked = Asked::Unsupported(format!(
                    "{} cannot say what it holds under the stack's services' names that \
                     lemonfiber did not make, so anything somebody else made there goes \
                     unreported",
                    check.server_name()
                ));
                break;
            }
            Err(failure) => {
                asked = Asked::Unread(format!(
                    "{} could not be asked what it holds under {app}: {failure}",
                    check.server_name()
                ));
            }
        }
    }
    let summary = match asked {
        Asked::Answered if findings.is_empty() => Some(Verdict::Pass {
            note: Some(format!(
                "{} holds nothing under the stack's services' names that lemonfiber did not \
                 make",
                check.server_name()
            )),
        }),
        Asked::Answered => None,
        Asked::Unsupported(reason) => Some(Verdict::Skipped { reason }),
        Asked::Unread(reason) => Some(unverified(&reason)),
    };
    findings.extend(
        summary
            .into_iter()
            .map(|verdict| check.finding(CHECK, TITLE, verdict)),
    );
    findings
}

/// The name one held key or account's finding carries.
fn reported_as(app: &str, one: &Astray) -> String {
    format!("services.astray.{app}.{}", one.id)
}

/// A check that could not be made, and why.
fn unverified(reason: &str) -> Verdict {
    Verdict::Unverified {
        reason: reason.to_owned(),
        remedy: Remedy::new("Check the media server is up and has finished starting")
            .with_detail("lemonfiber status"),
    }
}

/// One key or account `server` holds under `app` that lemonfiber did not make or no longer
/// knows.
fn astray(server: &str, app: &str, one: &Astray) -> Problem {
    Problem::new(
        ASTRAY,
        format!(
            "{server} holds {} (id {}) under {app}, and lemonfiber did not make it or no \
             longer knows it",
            one.name, one.id
        ),
        "lemonfiber leaves it alone: it uses and removes only what it made, so this stays \
         in the media server until somebody removes it",
        Remedy::new(format!(
            "If nobody here made it, remove it in {server}: {}",
            one.removed_in
        )),
    )
}

#[cfg(test)]
mod tests;
