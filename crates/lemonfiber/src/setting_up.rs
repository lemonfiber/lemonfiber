//! Setup's walk, one step at a time through the dispatcher, put to a person or to the
//! command line's flags; the answers applied, and the report a parser is handed.

pub mod flags;
mod prompt;
mod walk;

use std::path::PathBuf;

use lemonfiber_core::app::Ctx;
use lemonfiber_core::config::paths::Paths;
use lemonfiber_core::config::{
    data_root_from_env, service_user_from_env, store, Protocols, Settings,
};
use lemonfiber_core::model::kind::SETUP;
use lemonfiber_core::model::{Envelope, SetupOutcome, SetupReport};
use lemonfiber_core::wizard::Step;
use lemonfiber_error::Problem;

pub use flags::{Flags, SetupFlags};
pub use prompt::{CredentialChoice, Prompt, ProviderEntry};
pub use walk::{standing, walk, Walked};

/// Why a run answered by flags ended before it had a report to give.
#[derive(Debug)]
pub enum Unwalked {
    /// The flags each question still asked needs, which were not given.
    Missing(Vec<&'static str>),
    /// What walking or applying the answers was refused with.
    Failed(Box<Problem>),
    /// Where setup stands was answered with something other than the walk.
    Unread,
}

/// The data location setup proposes when the operator names none: a directory under
/// this machine's data base.
#[must_use]
pub fn default_data_location(paths: &Paths) -> PathBuf {
    paths.data_dir().join("media")
}

/// The prompt a run without a person answers from: `flags`, proposing
/// [`default_data_location`] where they name none.
///
/// # Errors
///
/// The flag of every step in `unanswered` that `flags` gives no answer to.
pub fn answering(
    unanswered: &[Step],
    flags: SetupFlags,
    paths: &Paths,
) -> Result<Flags, Vec<&'static str>> {
    let missing = flags.missing(unanswered);
    if missing.is_empty() {
        Ok(Flags::new(flags, default_data_location(paths)))
    } else {
        Err(missing)
    }
}

/// The report a parser is handed for a run that came to `outcome` under `settings`.
#[must_use]
pub fn report(outcome: SetupOutcome, settings: &Settings) -> Envelope<SetupReport> {
    Envelope::new(
        SETUP,
        SetupReport {
            outcome,
            protocols: settings.protocols,
            data_root: settings.data_root.clone(),
            service_user: settings
                .service_user
                .map(|(user, group)| format!("{user}:{group}")),
        },
    )
}

/// `ctx`'s settings with what setup decides read back from the env file `paths` names.
#[must_use]
pub fn as_written(ctx: &Ctx, paths: &Paths) -> Settings {
    let recorded = store::read(&paths.env_file())
        .map(|file| lemonfiber_core::stored_over(ctx, file))
        .unwrap_or_default();
    Settings {
        protocols: Protocols::from_env(&recorded),
        vpn_declined: lemonfiber_core::config::vpn_declined_from_env(&recorded),
        data_root: data_root_from_env(&recorded),
        service_user: service_user_from_env(&recorded),
        ..ctx.settings.clone()
    }
}

/// Setup answered by `flags` on a machine with nothing set up yet, and the report it
/// comes to, read from what applying it wrote.
///
/// # Errors
///
/// The flags a question still needs, or what the walk or the apply was refused with.
pub async fn by_flags(
    ctx: &Ctx,
    paths: &Paths,
    flags: SetupFlags,
) -> Result<Envelope<SetupReport>, Unwalked> {
    let standing = standing(ctx)
        .await
        .map_err(Unwalked::Failed)?
        .ok_or(Unwalked::Unread)?;
    let prompt = answering(&standing.unanswered, flags, paths).map_err(Unwalked::Missing)?;
    let outcome = walk(ctx, &prompt, standing)
        .await
        .map_err(Unwalked::Failed)?;
    Ok(report(outcome.into(), &as_written(ctx, paths)))
}
