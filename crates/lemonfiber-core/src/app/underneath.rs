//! How the stack runs without lemonfiber: which stack it is, where its project is, the
//! plain Compose words that bring it up, and what those words alone do not bring up.
//!
//! The words are the ones a start would run, built by the same invocation a rehearsed
//! start builds, so this read and lemonfiber's own start cannot come to disagree.

use crate::app::Ctx;
use crate::config::display::without_credentials;
use crate::config::store::Failure;
use crate::error::{Diagnose as _, Problem};
use crate::model::{NonInteractive, StackException, StackReport, StackSource};
use crate::stack::compose::Action;
use crate::stack::Source;

/// How any action is run without a question: by naming its command.
const NAMED: &str = "lemonfiber <command>";

/// Why a part this build installs is not brought up by the plain words alone.
const INSTALLED_FIRST: &str = "lemonfiber installs it from this build before the stack first \
                               starts, and plain Compose installs nothing";

/// How the stack runs without lemonfiber.
///
/// # Errors
///
/// Where the stack cannot be read, where nothing is set up to say where its project is,
/// and wherever building the start's invocation refuses.
pub(crate) async fn stack(ctx: &Ctx) -> Result<StackReport, Box<Problem>> {
    ctx.stack
        .checked_manifest(ctx.today())
        .map_err(|why| Box::new(why.problem()))?;
    let source = match ctx.stack {
        Source::Embedded(_) => StackSource::Bundled,
        Source::External(path) => StackSource::Own(path.display().to_string()),
    };
    let (Some(project), Some(env)) = (
        super::project::directory(ctx),
        ctx.settings.env_file.as_deref(),
    ) else {
        return Err(Box::new(Failure::Nowhere.problem()));
    };
    let forms = crate::autostart::run::load(ctx).forms().to_vec();
    let mut rehearsing = ctx.clone();
    rehearsing.dry_run = true;
    let start = super::engine::invocation(&rehearsing, &forms, &Action::Up)?;
    let exceptions = super::engine::offline(ctx, &forms)
        .await?
        .map(|offline| {
            offline
                .installs
                .into_iter()
                .map(|install| StackException {
                    part: install.plugin,
                    because: INSTALLED_FIRST.to_owned(),
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(StackReport {
        source,
        project: project.display().to_string(),
        env: env.display().to_string(),
        plain_up: start
            .command
            .iter()
            .map(|word| without_credentials(word))
            .collect(),
        non_interactive: NonInteractive {
            form: NAMED.to_owned(),
        },
        exceptions,
    })
}

#[cfg(test)]
mod tests;
