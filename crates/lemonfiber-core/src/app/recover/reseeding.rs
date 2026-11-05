//! Writing a plugin's minted secret back into a starting file a replacement wrote.
//!
//! The journal keeps a checksum of what was written and never what the file held, since
//! the file holds the secret. So the value goes back the way it came in: the secret store
//! holds it again once its own change is put back, and the shipped file says which line
//! it stands on.

use std::path::{Component, Path, PathBuf};

use crate::error::Problem;
use crate::ports::filesystem::{Beneath, Readable};

use super::refusing::Fault;
use super::Ctx;

/// What is said of a starting file reached through a link.
const LINKED: &str = "it is reached through a link";

/// The starting file at `path` written with the value the secret store holds of
/// `plugin`'s `secret` where the shipped file puts its placeholder, while it still holds
/// what was written, its checksum `written`; nothing where it is gone, holds anything
/// else, or the store or the files kept for the plugin no longer say what to write.
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
    let refused = |reason: String| {
        Box::new(
            Fault::NotRewound {
                path: at.to_path_buf(),
                reason,
            }
            .problem(),
        )
    };
    match rewritten(ctx, at, (plugin, secret), written) {
        Some(Ok((stack, bytes))) => ctx
            .seams
            .confined
            .written(at, &stack, &bytes, Readable::Owner)
            .map_err(|fault| refused(fault.message)),
        Some(Err(reason)) => Err(refused(reason)),
        None => Ok(()),
    }
}

/// What the starting file at `at` holds once `plugin`'s `secret` is written back into it,
/// and the stack it lies beneath: nothing where there is no stack, it is gone or it holds
/// other than what was written, and why where it cannot be worked out.
fn rewritten(
    ctx: &Ctx,
    at: &Path,
    (plugin, secret): (&str, &str),
    written: u32,
) -> Option<Result<(PathBuf, Vec<u8>), String>> {
    let stack = crate::app::project::directory(ctx)?;
    let kept = crate::app::plugins::kept::root(ctx, plugin)?;
    let (service, file) = beneath(&stack, at)?;
    let current = match ctx.seams.confined.read(at, &stack) {
        Beneath::Read(text) => text,
        Beneath::Absent => return None,
        Beneath::Escaped => return Some(Err(LINKED.to_owned())),
    };
    (crate::materialised::checksum(current.as_bytes()) == written).then_some(())?;
    let value = crate::settings::Stores::of(ctx)
        .secrets
        .value(&crate::settings::named(plugin, secret))?
        .to_owned();
    let starting = crate::plugin::program::Starting {
        root: &kept,
        service: &service,
    };
    let template = match crate::plugin::program::shipped(Some(starting)) {
        Ok(shipped) => shipped
            .into_iter()
            .find_map(|(named, bytes)| (Path::new(&named) == file).then_some(bytes))?,
        Err(why) => return Some(Err(why)),
    };
    let random = ctx.seams.random.as_ref();
    let bytes =
        crate::plugin::seeding::replaced(&template, current.as_bytes(), secret, &value, random)
            .transpose()?;
    Some(bytes.map(|bytes| (stack, bytes)))
}

/// The service whose configuration directory beneath `stack` holds `at`, and where it is
/// beneath that directory.
fn beneath<'a>(stack: &Path, at: &'a Path) -> Option<(String, &'a Path)> {
    let within = at
        .strip_prefix(stack.join(crate::project::entry::CONFIGURATION_TREE))
        .ok()?;
    let mut components = within.components();
    let service = Some(components.next()?).filter(|first| matches!(first, Component::Normal(_)))?;
    Some((
        service.as_os_str().to_str()?.to_owned(),
        components.as_path(),
    ))
}
