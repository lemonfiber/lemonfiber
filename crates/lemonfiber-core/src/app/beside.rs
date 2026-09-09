//! Standing lemonfiber beside a setup that is already running.
//!
//! The mode a cautious operator wants: a second copy on ports nothing else is using, so
//! it can be tried without moving the first out of the way. Nothing of theirs is
//! touched — not a container, not a file, not a port they already answer on.
//!
//! What it writes is a Compose file of lemonfiber's own, layered over the stack's, that
//! says where each service listens instead. Unconfirmed it says what it would write and
//! writes nothing, so what an operator agrees to is what they were shown.

use std::fmt::Write as _;

use crate::config::{store, OVERLAY_KEY};
use crate::error::{Diagnose, Problem};
use crate::model::{BesideReport, MigrationReport, MovedReport};

use super::Ctx;

/// The name the layered file is written under, beside the configuration it belongs to.
const FILE: &str = "beside.yml";

/// Stand beside what is already here, or say why that cannot happen.
///
/// # Errors
///
/// Where there is nowhere configured to write to, or the file cannot be written.
pub async fn stand(
    ctx: &Ctx,
    survey: &MigrationReport,
    confirmed: bool,
) -> Result<BesideReport, Box<Problem>> {
    if !survey.read {
        return Ok(refused(
            "what is on this machine could not be read, and standing beside a setup that \
             could not be looked at would be guessing which ports are free",
        ));
    }
    if survey.beside.is_empty() {
        return Ok(refused(
            "no service lemonfiber runs has anywhere else to listen, so there is no way \
             to stand beside what is here",
        ));
    }

    if !confirmed {
        return Ok(BesideReport {
            ports: survey.beside.clone(),
            rehearsed: true,
            ..BesideReport::default()
        });
    }

    let Some(path) = ctx.settings.env_file.as_ref().map(|env| {
        env.parent()
            .map_or_else(|| std::path::PathBuf::from(FILE), |dir| dir.join(FILE))
    }) else {
        return Err(Box::new(store::Failure::Nowhere.problem()));
    };

    ctx.filesystem.write(&path, &layered(&survey.beside)).await;

    let recorded = ctx
        .settings
        .env_file
        .as_ref()
        .ok_or_else(|| Box::new(store::Failure::Nowhere.problem()))?;
    store::set(recorded, OVERLAY_KEY, &path.display().to_string())
        .map_err(|failure| Box::new(failure.problem()))?;

    Ok(BesideReport {
        ports: survey.beside.clone(),
        written: Some(path.display().to_string()),
        applied: true,
        ..BesideReport::default()
    })
}

/// What is answered where standing beside cannot happen.
fn refused(why: &str) -> BesideReport {
    BesideReport {
        refused: Some(why.to_owned()),
        ..BesideReport::default()
    }
}

/// The Compose file that says where each service listens instead.
///
/// Only the ports. Everything else about a service is the stack's own file's business,
/// and an overlay that restated any of it would be a second answer to a question already
/// answered — one that goes stale the moment the stack's own file changes.
fn layered(moved: &[MovedReport]) -> String {
    let mut written = String::from("services:\n");
    for one in moved {
        let _ = write!(
            written,
            "  {}:\n    ports:\n      - \"{}:{}\"\n",
            one.service, one.to, one.from
        );
    }
    written
}

#[cfg(test)]
mod tests {
    use super::layered;
    use crate::model::MovedReport;

    fn moved(service: &str, from: u16, to: u16) -> MovedReport {
        MovedReport {
            service: service.to_owned(),
            from,
            to,
        }
    }

    /// The host port moves and the container port does not: what a service listens on
    /// inside itself is the image's business, and rewriting it would break the service
    /// rather than move it.
    #[test]
    fn only_the_host_side_of_a_port_is_moved() {
        let written = layered(&[moved("sonarr", 8989, 8990)]);
        assert!(written.contains("\"8990:8989\""), "{written}");
    }

    /// Every service that moved, under the one key Compose merges by.
    #[test]
    fn each_service_that_moved_is_named_under_services() {
        let written = layered(&[moved("sonarr", 8989, 8990), moved("radarr", 7878, 7879)]);
        assert!(written.starts_with("services:\n"), "{written}");
        assert!(written.contains("  sonarr:\n"), "{written}");
        assert!(written.contains("  radarr:\n"), "{written}");
    }

    /// Nothing but ports, so the overlay cannot go stale against the stack's own file.
    #[test]
    fn nothing_but_ports_is_restated() {
        let written = layered(&[moved("sonarr", 8989, 8990)]);
        assert!(!written.contains("image"), "{written}");
        assert!(!written.contains("volumes"), "{written}");
    }
}
