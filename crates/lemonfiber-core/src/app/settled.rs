//! What setup settled, read from the configuration as it stands, and whether this stack is
//! set up.

use futures_util::future::OptionFuture;

use crate::app::targets::layout;
use crate::app::Ctx;
use crate::config::env::EnvFile;
use crate::config::store;
use crate::config::Protocols;
use crate::error::{Diagnose as _, Problem};
use crate::model::{ServiceUser, SettledReport, UserFrom};
use crate::plugin::adapting::as_adapter;
use crate::project::entry::{operated_ids, OperatedIds};

/// What setup settled, and whether this stack is set up.
///
/// # Errors
///
/// Where there is nowhere to keep configuration.
pub(crate) async fn settled(ctx: &Ctx) -> Result<SettledReport, Box<Problem>> {
    let Some(paths) = layout(ctx) else {
        return Err(Box::new(store::Failure::Nowhere.problem()));
    };
    let env_file = paths.env_file();
    let saved = super::setup::progress_at(&paths.setup_progress());
    let setup = super::setup::set_up(saved.as_ref(), &paths);
    let file = store::read(&env_file)
        .map(|file| crate::settings::legacy::overlaid(ctx, file))
        .unwrap_or_default();
    let stack = super::project::directory(ctx);
    let asked = stack
        .as_deref()
        .map(|stack| ctx.seams.filesystem.ownership(stack));
    let owner = OptionFuture::from(asked)
        .await
        .flatten()
        .map(|held| as_adapter((held.uid, held.gid)));
    Ok(SettledReport {
        setup,
        data_root: data_root(&file),
        protocols: Protocols::from_env(&file),
        service_user: service_user(operated_ids(
            &|key: &str| file.get(key).map(str::to_owned),
            owner,
        )),
    })
}

/// Where `DATA_ROOT` keeps the library, as it names it.
fn data_root(file: &EnvFile) -> Option<String> {
    crate::config::data_root_from_env(file)
        .map(|path| path.display().to_string())
        .or_else(|| crate::config::nfs_from_env(file).map(|export| export.written()))
}

/// The user and group the services run as, and where the two come from.
const fn service_user(ids: OperatedIds) -> ServiceUser {
    ServiceUser {
        uid: ids.uid,
        gid: ids.gid,
        from: match (ids.uid_set, ids.gid_set) {
            (true, true) => UserFrom::Set,
            (false, false) => UserFrom::StackOwner,
            _ => UserFrom::PartSet,
        },
    }
}

#[cfg(test)]
mod tests;
