//! Setup's questions, put one step at a time through the dispatcher.
//!
//! The same steps a browser takes: where the walk stands, one answer, back one,
//! apply. The core proves each answer as it is recorded and says what it found; the
//! operator is asked what to do about a finding, and whatever they choose is another
//! step.

use lemonfiber_core::app::setup::asked;
use lemonfiber_core::app::{dispatch, Command, Ctx, Outcome, SetupAction};
use lemonfiber_core::error::Problem;
use lemonfiber_core::model::{SetupOutcome, StorageFinding, WizardReport};
use lemonfiber_core::prerequisites::prerequisites;
use lemonfiber_core::validate::Validation;
use lemonfiber_core::wizard::{Answer, Indexer, Provider, Step, Vpn};

use super::{CredentialChoice, Prompt};

/// How a walk through setup ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Walked {
    /// The reviewed answers were applied.
    Applied,
    /// The operator saw the plan and chose not to apply it.
    Abandoned,
}

impl From<Walked> for SetupOutcome {
    fn from(walked: Walked) -> Self {
        match walked {
            Walked::Applied => Self::Applied,
            Walked::Abandoned => Self::Abandoned,
        }
    }
}

/// Where setup stands on the machine `ctx` sets up, as the walk's first step reads it:
/// nothing where the dispatcher answered with something other than the walk.
///
/// # Errors
///
/// What the dispatcher refused the step with.
pub async fn standing(ctx: &Ctx) -> Result<Option<WizardReport>, Box<Problem>> {
    let answered = Box::pin(dispatch(Command::Setup(SetupAction::Where), ctx)).await?;
    Ok(walked_to(answered).ok())
}

/// Why a walk stopped short of its end.
#[derive(Debug)]
enum Ended {
    /// A step was refused.
    Refused(Box<Problem>),
    /// The dispatcher answered with something other than the walk, which ends it with
    /// nothing applied.
    Elsewhere,
}

impl From<Box<Problem>> for Ended {
    fn from(problem: Box<Problem>) -> Self {
        Self::Refused(problem)
    }
}

/// Where an answer leaves setup, where it is the walk's.
fn walked_to(answered: Outcome) -> Result<WizardReport, Ended> {
    match answered {
        Outcome::Wizard(report) => Ok(report),
        _ => Err(Ended::Elsewhere),
    }
}

/// How a walk that stopped short ended, as its caller is told.
fn ended(walked: Result<Walked, Ended>) -> Result<Walked, Box<Problem>> {
    match walked {
        Ok(walked) => Ok(walked),
        Err(Ended::Refused(problem)) => Err(problem),
        Err(Ended::Elsewhere) => Ok(Walked::Abandoned),
    }
}

/// One step of the walk, and where it leaves setup.
async fn step(ctx: &Ctx, action: SetupAction) -> Result<WizardReport, Ended> {
    walked_to(Box::pin(dispatch(Command::Setup(action), ctx)).await?)
}

/// Put every question still unanswered to `prompt`, from where `report` stands, and
/// apply the answers once the operator confirms what they come to.
///
/// # Errors
///
/// The refusal a step meets: an answer that does not apply here, an egress guard left
/// short, or an apply that failed and left its marker for recovery.
pub async fn walk(
    ctx: &Ctx,
    prompt: &dyn Prompt,
    report: WizardReport,
) -> Result<Walked, Box<Problem>> {
    ended(walking(ctx, prompt, report).await)
}

/// Each step from where `report` stands to the review, and the review.
async fn walking(
    ctx: &Ctx,
    prompt: &dyn Prompt,
    mut report: WizardReport,
) -> Result<Walked, Ended> {
    loop {
        let asked = report.asks && report.unanswered.contains(&report.at);
        report = match report.at {
            Step::Review => {
                if !prompt.confirm(&report.plan) {
                    return Ok(Walked::Abandoned);
                }
                step(ctx, SetupAction::Apply).await?;
                return Ok(Walked::Applied);
            }
            Step::Preflight if report.asks => {
                answer(ctx, Answer::House(prompt.house(&report.house))).await?
            }
            Step::Protocols if asked => {
                let protocols = prompt.protocols();
                prompt.prerequisites(&prerequisites(protocols));
                answer(ctx, Answer::Protocols(protocols)).await?
            }
            Step::Vpn if asked => answer(ctx, Answer::Vpn(vpn(prompt, &report))).await?,
            Step::DataLocation if asked => location(ctx, prompt).await?,
            Step::Credentials if asked => credentials(ctx, prompt).await?,
            Step::Provider if asked => provider(ctx, prompt).await?,
            Step::ServiceUser if asked => {
                answer(ctx, Answer::ServiceUser(prompt.service_user())).await?
            }
            Step::Library if asked => answer(ctx, Answer::Library(prompt.library())).await?,
            Step::Household if asked => answer(ctx, Answer::Household(prompt.household())).await?,
            Step::Notifications if asked => {
                answer(ctx, Answer::Notifications(prompt.notifications())).await?
            }
            Step::Autostart if asked => answer(ctx, Answer::Autostart(prompt.autostart())).await?,
            _ => step(ctx, SetupAction::Next).await?,
        };
    }
}

/// Record `given` and move on.
async fn answer(ctx: &Ctx, given: Answer) -> Result<WizardReport, Ended> {
    step(ctx, SetupAction::Answer(given)).await
}

/// Whether a VPN carries the torrents, with what the guard asks of it where one does;
/// a no taken only once the operator has heard what it exposes.
fn vpn(prompt: &dyn Prompt, report: &WizardReport) -> Vpn {
    loop {
        if prompt.vpn() {
            return Vpn::Carrying(asked(&report.guard, &mut |asking| prompt.guard(asking)));
        }
        if prompt.unprotected() {
            return Vpn::Absent;
        }
    }
}

/// A data location, kept once it links or once the operator accepts what its test found.
async fn location(ctx: &Ctx, prompt: &dyn Prompt) -> Result<WizardReport, Ended> {
    loop {
        let chosen = prompt.data_location();
        let report = answer(ctx, Answer::DataLocation(chosen.clone())).await?;
        match &report.storage {
            Some(StorageFinding::Links { inferred_from }) => {
                prompt.hardlinks(&chosen, inferred_from.as_deref());
                return Ok(report);
            }
            Some(found) if !prompt.storage_warning(&chosen, found) => {
                step(ctx, SetupAction::Back).await?;
            }
            _ => return Ok(report),
        }
    }
}

/// An indexer, kept once it is proven or once the operator chooses what to do with one
/// that is not.
async fn credentials(ctx: &Ctx, prompt: &dyn Prompt) -> Result<WizardReport, Ended> {
    loop {
        let Some((url, key)) = prompt.credential() else {
            return answer(ctx, Answer::Credentials(None)).await;
        };
        let given = Answer::Credentials(Some(Indexer {
            url,
            key,
            validated: false,
        }));
        let report = answer(ctx, given).await?;
        match settled(ctx, prompt, report).await? {
            Settled::Kept(report) => return Ok(*report),
            Settled::Again => {}
            Settled::Unset => return answer(ctx, Answer::Credentials(None)).await,
        }
    }
}

/// A Usenet provider, kept once its login is proven or once the operator chooses what
/// to do with one that is not.
async fn provider(ctx: &Ctx, prompt: &dyn Prompt) -> Result<WizardReport, Ended> {
    loop {
        let Some(entry) = prompt.usenet_provider() else {
            return answer(ctx, Answer::Provider(None)).await;
        };
        let given = Answer::Provider(Some(Provider {
            host: entry.host,
            port: entry.port,
            user: entry.user,
            pass: entry.pass,
            tls: entry.tls,
            validated: false,
        }));
        let report = answer(ctx, given).await?;
        match settled(ctx, prompt, report).await? {
            Settled::Kept(report) => return Ok(*report),
            Settled::Again => {}
            Settled::Unset => return answer(ctx, Answer::Provider(None)).await,
        }
    }
}

/// What the operator made of a credential's live test.
enum Settled {
    /// Kept, proven or not, with where the walk now stands.
    Kept(Box<WizardReport>),
    /// Taken back, to be entered again.
    Again,
    /// Taken back, to be left unset.
    Unset,
}

/// Show what proving the credential just recorded came to, and where it did not prove
/// it, take it back unless the operator keeps it.
async fn settled(ctx: &Ctx, prompt: &dyn Prompt, report: WizardReport) -> Result<Settled, Ended> {
    let failed = match &report.proof {
        Some(Validation::Valid { observed }) => {
            prompt.credential_valid(observed);
            None
        }
        other => other.as_ref(),
    };
    let Some(outcome) = failed else {
        return Ok(Settled::Kept(Box::new(report)));
    };
    match prompt.credential_failed(outcome) {
        CredentialChoice::Proceed => Ok(Settled::Kept(Box::new(report))),
        CredentialChoice::Retry => {
            step(ctx, SetupAction::Back).await?;
            Ok(Settled::Again)
        }
        CredentialChoice::Skip => {
            step(ctx, SetupAction::Back).await?;
            Ok(Settled::Unset)
        }
    }
}

#[cfg(test)]
mod tests;
