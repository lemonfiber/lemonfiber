//! What setup settled, read from the configuration as it stands, and whether this stack is
//! set up.

use crate::app::targets::layout;
use crate::app::Ctx;
use crate::config::env::EnvFile;
use crate::config::store;
use crate::config::Protocols;
use crate::error::{Diagnose as _, Problem};
use crate::model::{ServiceUser, SetUp, SettledReport, UserFrom};
use crate::project::entry::{operated_ids, OperatedIds};
use crate::wizard::Status;

/// What setup settled, and whether this stack is set up.
///
/// # Errors
///
/// Where there is nowhere to keep configuration.
pub(crate) fn settled(ctx: &Ctx) -> Result<SettledReport, Box<Problem>> {
    let Some(paths) = layout(ctx) else {
        return Err(Box::new(store::Failure::Nowhere.problem()));
    };
    let env_file = paths.env_file();
    let saved = super::setup::progress_at(&paths.setup_progress());
    let setup = if Status::of(saved.as_ref()).unfinished() {
        SetUp::PartWay
    } else if env_file.exists() {
        SetUp::Done
    } else {
        SetUp::None
    };
    let file = store::read(&env_file)
        .map(|file| crate::settings::legacy::overlaid(ctx, file))
        .unwrap_or_default();
    let owner = super::project::directory(ctx)
        .as_deref()
        .and_then(crate::plugin::adapting::running_as);
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
