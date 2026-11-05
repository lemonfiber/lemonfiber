//! Writing a plugin's minted secret back into a starting file a replacement wrote.
//!
//! The journal keeps a checksum of what was written and never what the file held, since
//! the file holds the secret. So the value goes back the way it came in: the secret store
//! holds it again once its own change is put back, and the shipped file says which line
//! it stands on.

use std::path::{Component, Path};

use crate::error::Problem;
use crate::ports::filesystem::{Beneath, Readable};

use super::refusing::Fault;
use super::{Ctx, Step};

/// What the starting file at `path` comes to: put back where it still holds what was
/// written, its checksum `written`, or nothing to do where it is gone, and somebody's
/// own where it holds anything else.
pub(super) fn unseeded(path: &Path, written: u32) -> Result<Step, Fault> {
    match std::fs::read(path) {
        Ok(holds) if crate::materialised::checksum(&holds) == written => Ok(Step::Done),
        Ok(_) => Ok(Step::TheirsNow(path.display().to_string())),
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => Ok(Step::Done),
        Err(why) => Err(not_rewound(path, why.to_string())),
    }
}

/// The starting file at `path` written with the value the secret store holds of
/// `plugin`'s `secret` where the shipped file puts its placeholder, while it still holds
/// what was written, its checksum `written`.
///
/// # Errors
///
/// Where the file is reached through a link, or the value cannot be written into it.
pub(super) fn reseeded(
    ctx: &Ctx,
    path: &str,
    plugin: &str,
    secret: &str,
    written: u32,
) -> Result<(), Box<Problem>> {
    let at = Path::new(path);
    let refused = |reason: String| Box::new(not_rewound(at, reason).problem());
    let Some((stack, kept)) =
        crate::app::project::directory(ctx).zip(crate::app::plugins::kept::root(ctx, plugin))
    else {
        return Ok(());
    };
    let Some((service, file)) = beneath(&stack, at) else {
        return Ok(());
    };
    let current = match ctx.seams.confined.read(at, &stack) {
        Beneath::Read(text) => text,
        Beneath::Absent => return Ok(()),
        Beneath::Escaped => return Err(refused("it is reached through a link".to_owned())),
    };
    let value = crate::settings::Stores::of(ctx)
        .secrets
        .value(&crate::settings::named(plugin, secret))
        .map(str::to_owned);
    let shipped = crate::plugin::program::shipped(Some(crate::plugin::program::Starting {
        root: &kept,
        service: &service,
    }))
    .map_err(&refused)?;
    let template = shipped
        .into_iter()
        .find_map(|(named, bytes)| (Path::new(&named) == file).then_some(bytes));
    let (Some(value), Some(template)) = (value, template) else {
        return Ok(());
    };
    if crate::materialised::checksum(current.as_bytes()) != written {
        return Ok(());
    }
    let replaced = crate::plugin::seeding::replaced(
        &template,
        current.as_bytes(),
        secret,
        &value,
        ctx.seams.random.as_ref(),
    )
    .map_err(&refused)?;
    match replaced {
        Some(bytes) => ctx
            .seams
            .confined
            .written(at, &stack, &bytes, Readable::Owner)
            .map_err(|fault| refused(fault.message)),
        None => Ok(()),
    }
}

/// The service whose configuration directory beneath `stack` holds `at`, and where it is
/// beneath that directory.
fn beneath<'a>(stack: &Path, at: &'a Path) -> Option<(String, &'a Path)> {
    let within = at
        .strip_prefix(stack.join(crate::project::entry::CONFIGURATION_TREE))
        .ok()?;
    let mut components = within.components();
    let Some(Component::Normal(service)) = components.next() else {
        return None;
    };
    Some((service.to_str()?.to_owned(), components.as_path()))
}

/// What is said of the starting file at `path` that could not be written back, and why.
fn not_rewound(path: &Path, reason: String) -> Fault {
    Fault::NotRewound {
        path: path.to_path_buf(),
        reason,
    }
}
