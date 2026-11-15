//! Setup answered without a person: the wizard walked with the command line's flags
//! as its answers, the answers applied, and the report a parser is handed.

pub mod flags;

use std::path::PathBuf;

use lemonfiber_core::app::apply::Applying;
use lemonfiber_core::app::setup::{self as core_setup, Prompt};
use lemonfiber_core::app::Ctx;
use lemonfiber_core::config::paths::Paths;
use lemonfiber_core::config::{
    data_root_from_env, service_user_from_env, store, Protocols, Settings,
};
use lemonfiber_core::error::Problem;
use lemonfiber_core::instant::Instant;
use lemonfiber_core::model::{kind, Envelope, SetupOutcome, SetupReport};
use lemonfiber_core::wizard::Wizard;

pub use flags::{Flags, SetupFlags};

/// Why a run answered by flags ended before it had a report to give.
#[derive(Debug)]
pub enum Unwalked {
    /// The flags each question still asked needs, which were not given.
    Missing(Vec<&'static str>),
    /// What walking or applying the answers was refused with.
    Failed(Box<Problem>),
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
/// The flag of every question `wizard` still asks that `flags` gives no answer to.
pub fn answering(
    wizard: &Wizard,
    flags: SetupFlags,
    paths: &Paths,
) -> Result<Flags, Vec<&'static str>> {
    let missing = flags.missing(wizard);
    if missing.is_empty() {
        Ok(Flags::new(flags, default_data_location(paths)))
    } else {
        Err(missing)
    }
}

/// What walking `wizard` with `prompt`'s answers and applying them came to, the change
/// journal stamped by `ctx`'s clock.
///
/// # Errors
///
/// What the walk or the apply was refused with.
pub async fn walked(
    ctx: &Ctx,
    paths: &Paths,
    wizard: &mut Wizard,
    prompt: &dyn Prompt,
) -> Result<core_setup::Outcome, Box<Problem>> {
    let guard = core_setup::guard(ctx);
    let applying = Applying {
        paths,
        source: ctx.stack,
        stamp: Instant::of(ctx.seams.clock.now()),
        random: ctx.seams.random.as_ref(),
        confined: ctx.seams.confined.as_ref(),
        guard: guard.as_ref(),
    };
    core_setup::run(
        wizard,
        (prompt, guard.as_ref()),
        ctx.seams.filesystem.as_ref(),
        ctx.validator.as_ref(),
        &applying,
    )
    .await
}

/// The report a parser is handed for a run that came to `outcome` under `settings`.
#[must_use]
pub fn report(outcome: SetupOutcome, settings: &Settings) -> Envelope<SetupReport> {
    Envelope::new(
        kind::SETUP,
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
    let mut wizard = Wizard::new(ctx.environment);
    let prompt = answering(&wizard, flags, paths).map_err(Unwalked::Missing)?;
    let outcome = walked(ctx, paths, &mut wizard, &prompt)
        .await
        .map_err(Unwalked::Failed)?;
    Ok(report(outcome.into(), &as_written(ctx, paths)))
}
