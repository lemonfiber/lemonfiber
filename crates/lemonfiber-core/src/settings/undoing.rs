use std::path::Path;

use crate::config::paths::{SECRETS, SETTINGS};
use crate::config::store::{self, Failure};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Undone {
    Done,
    TheirsNow,
    Unkept,
}

pub(crate) fn resettled(
    env_file: &Path,
    name: &str,
    value: Option<&str>,
    wrote: Option<&str>,
) -> Result<Undone, Failure> {
    super::store::resettled(&env_file.with_file_name(SETTINGS), name, value, wrote)
}

pub(crate) fn reheld(
    env_file: &Path,
    name: &str,
    previous: Option<&str>,
    wrote: Option<&str>,
    filed: Option<&str>,
) -> Result<Undone, Failure> {
    super::secrets::reheld(
        &env_file.with_file_name(SECRETS),
        name,
        previous,
        wrote,
        &|value| filed.map_or(Ok(()), |key| store::set(env_file, key, value)),
    )
}
