//! A start refused where a directory the project on disk mounts into a service is reached
//! through a link beneath the project directory, which the engine would follow.

use std::path::{Path, PathBuf};

use super::super::Ctx;
use crate::error::codes::life::MOUNT_LINKED;
use crate::error::{Problem, Remedy, State};
use crate::ports::filesystem::{Beneath, Lying};
use crate::project::document::sources;
use crate::project::FILE;

/// Refuse where the project on disk, or a directory it mounts into any of its services, is
/// reached through a link beneath the project directory.
///
/// # Errors
///
/// [`MOUNT_LINKED`], naming each service and path reached through a link.
pub(crate) fn unlinked(ctx: &Ctx) -> Result<(), Box<Problem>> {
    let Some(directory) = super::directory(ctx) else {
        return Ok(());
    };
    let confined = ctx.seams.confined.as_ref();
    let project = directory.join(FILE);
    let linked: Vec<(String, PathBuf)> = match confined.read(&project, &directory) {
        Beneath::Read(text) => sources(&text)
            .into_iter()
            .map(|(service, source)| {
                let beneath = source.strip_prefix("./").unwrap_or(&source);
                (service, directory.join(beneath))
            })
            .filter(|(_, path)| confined.lies(path, &directory) == Lying::Escaped)
            .collect(),
        Beneath::Absent => Vec::new(),
        Beneath::Escaped => vec![(STACK.to_owned(), project)],
    };
    match linked.as_slice() {
        [] => Ok(()),
        [(whose, path), ..] => Err(Box::new(refused(whose, path, &linked))),
    }
}

/// Who a start over a project reached through a link is refused for.
const STACK: &str = "the stack";

/// The refusal of a start of `whose` over `path`, with each of `linked` beside it.
fn refused(whose: &str, path: &Path, linked: &[(String, PathBuf)]) -> Problem {
    let path = path.display();
    Problem::new(
        MOUNT_LINKED,
        format!("{whose} was not started: {path} is reached through a link"),
        format!(
            "The engine follows a link wherever it mounts, so {whose} would be handed whatever \
             {path} points at on this machine. A stack lemonfiber wrote has no link there, so \
             nothing was started."
        ),
        Remedy::new(format!("Take the link away at {path}, then start again")),
    )
    .in_state(State::Guided)
    .with_detail(
        linked
            .iter()
            .map(|(whose, path)| format!("{whose}: {}", path.display()))
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

#[cfg(test)]
mod tests;
