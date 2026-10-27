//! Moving the values an earlier release kept under a part's name to the keys that say
//! what they hold.

use crate::app::Ctx;
use crate::config::renamed::RENAMED;
use crate::config::store::{self, Failure};
use crate::journal::{Change, Kind};

/// Move each value the environment file holds under a key an earlier release wrote to the
/// key it is kept under now, journalled, leaving one whose key now holds a value as it is,
/// and an empty one where it stands.
///
/// # Errors
///
/// Where the environment file or the journal cannot be read or written.
pub(crate) fn renamed(ctx: &Ctx) -> Result<(), Failure> {
    if ctx.dry_run {
        return Ok(());
    }
    let Some(env) = ctx.settings.env_file.as_deref() else {
        return Ok(());
    };
    let file = store::read(env)?;
    let held = |key: &str| file.written(key).filter(|value| !value.is_empty());
    let moving: Vec<(&str, &str, &str)> = RENAMED
        .iter()
        .filter_map(|(was, now)| match (held(was), held(now)) {
            (Some(value), None) => Some((*was, *now, value)),
            _ => None,
        })
        .collect();
    if moving.is_empty() {
        return Ok(());
    }
    let paths = crate::app::targets::layout(ctx).ok_or(Failure::Nowhere)?;
    let stamp = ctx.stamp();
    let changes: Vec<Change> = moving
        .iter()
        .map(|(was, now, _)| Change {
            at: stamp.clone(),
            operation: super::OPERATION.to_owned(),
            target: (*now).to_owned(),
            kind: Kind::Renamed {
                from: (*was).to_owned(),
                to: (*now).to_owned(),
            },
        })
        .collect();
    crate::app::recover::journalled(&paths.journal(), &changes, ctx.seams.random.as_ref())?;
    for (was, now, value) in &moving {
        store::set(env, now, value)?;
        store::unset(env, was)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
