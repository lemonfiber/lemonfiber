//! Standing lemonfiber beside a setup that is already running.
//!
//! The mode a cautious operator wants: a second copy on ports nothing else is using, so
//! it can be tried without moving the first out of the way. Nothing of theirs is
//! touched — not a container, not a file, not a port they already answer on.
//!
//! Where each service listens instead is recorded as one of lemonfiber's own settings,
//! journalled so it can be put back, and the generator writes each service's published
//! port from it. Unconfirmed it says what it would record and records nothing, so what
//! an operator agrees to is what they were shown.

use crate::config::env::EnvFile;
use crate::config::{store, BESIDE_KEY};
use crate::error::{Diagnose, Problem};
use crate::journal::{Change, Kind};
use crate::model::{BesideReport, MigrationReport, MovedReport};
use crate::reconfigure::Stance;

use super::Ctx;

/// The operation recording the moves is journalled under.
const OPERATION: &str = "beside";

/// Stand beside what is already here, or say why that cannot happen.
///
/// # Errors
///
/// Where there is nowhere configured to write to, or the file cannot be written.
pub fn stand(
    ctx: &Ctx,
    survey: &MigrationReport,
    confirmed: bool,
) -> Result<BesideReport, Box<Problem>> {
    if let Some(refused) = blocked(survey) {
        return Ok(refused);
    }

    if !confirmed || ctx.dry_run {
        return Ok(BesideReport {
            ports: survey.beside.clone(),
            stance: Stance::Pending,
            ..BesideReport::default()
        });
    }

    let (Some(recorded), Some(paths)) = (
        ctx.settings.env_file.clone(),
        crate::app::targets::layout(ctx),
    ) else {
        return Err(Box::new(store::Failure::Nowhere.problem()));
    };
    let held = store::read(&recorded).map_err(|failure| Box::new(failure.problem()))?;
    let moves = written(&survey.beside);
    let change = Change {
        at: ctx.stamp(),
        operation: OPERATION.to_owned(),
        target: BESIDE_KEY.to_owned(),
        kind: Kind::Set {
            key: BESIDE_KEY.to_owned(),
            previous: held.get(BESIDE_KEY).map(str::to_owned),
            current: moves.clone(),
        },
    };
    crate::app::recover::journalled(&paths.journal(), &[change])
        .map_err(|failure| Box::new(failure.problem()))?;
    store::set(&recorded, BESIDE_KEY, &moves).map_err(|failure| Box::new(failure.problem()))?;

    Ok(BesideReport {
        ports: survey.beside.clone(),
        written: Some(recorded.display().to_string()),
        stance: Stance::Applied,
        ..BesideReport::default()
    })
}

/// What stops standing beside, where anything does.
///
/// Apart from the act so it can be asked without a machine: both answers are about what
/// the survey found and nothing else.
fn blocked(survey: &MigrationReport) -> Option<BesideReport> {
    let why = if survey.read {
        if !survey.beside.is_empty() {
            return None;
        }
        if !survey
            .standing
            .iter()
            .any(crate::migration::standing::runs_ours)
        {
            return Some(refused(
                "nothing here runs a service lemonfiber runs, so there is no setup to stand \
                 beside",
            ));
        }
        "no service lemonfiber runs has anywhere else to listen, so there is no way to \
         stand beside what is here"
    } else {
        "what is on this machine could not be read, and standing beside a setup that \
         could not be looked at would be guessing which ports are free"
    };
    Some(refused(why))
}

/// Standing beside, refused for `why`.
fn refused(why: &str) -> BesideReport {
    BesideReport {
        stance: Stance::Blocked,
        refusal: Some(why.to_owned()),
        ..BesideReport::default()
    }
}

/// Where each service listens instead, as the setting holds it: `service:from=to`, one
/// for each service moved, separated by commas.
#[must_use]
pub fn written(moved: &[MovedReport]) -> String {
    moved
        .iter()
        .map(|one| format!("{}:{}={}", one.service, one.from, one.to))
        .collect::<Vec<_>>()
        .join(",")
}

/// The moves `held` records, or nothing where it records none or one will not read.
#[must_use]
pub fn moved(held: &EnvFile) -> Vec<MovedReport> {
    held.get(BESIDE_KEY).and_then(read).unwrap_or_default()
}

/// The moves `value` writes, where every one of them reads.
#[must_use]
pub fn read(value: &str) -> Option<Vec<MovedReport>> {
    value
        .split(',')
        .map(str::trim)
        .filter(|one| !one.is_empty())
        .map(|one| {
            let (service, ports) = one.split_once(':')?;
            let (from, to) = ports.split_once('=')?;
            Some(MovedReport {
                service: service.to_owned(),
                from: from.parse().ok()?,
                to: to.parse().ok()?,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests;
