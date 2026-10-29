//! A plugin's own `posture/prepare`, run with a throwaway copy of the starting files of
//! the service whose id is the plugin's before a recording starts that service, for what
//! install writes there.

use std::path::{Path, PathBuf};
use std::time::Duration;

use super::throwaway::{linkless, Reached, Refusals, Throwaway};
use crate::ports::process::Runner;

/// Where a plugin ships the script, beneath its root.
pub(crate) const PREPARE: &str = "posture/prepare";

/// How long the script may run before it is stopped.
pub(crate) const PREPARED_WITHIN: Duration = Duration::from_secs(30);

/// What runs a plugin's `posture/prepare`, and the root of the plugin that ships it.
#[derive(Clone, Copy)]
pub(crate) struct Preparing<'a> {
    /// Runs the script on this machine.
    pub(crate) runner: &'a dyn Runner,
    /// The plugin's own source, which the script is run from.
    pub(crate) root: &'a Path,
}

impl Preparing<'_> {
    /// `copy` handed to the plugin's `posture/prepare`, where it ships one; why not where
    /// the script could not be run or did not exit 0 within [`PREPARED_WITHIN`].
    pub(crate) async fn prepared(&self, copy: &Throwaway) -> Result<(), String> {
        let Some(script) = runnable(self.root, Path::new(PREPARE))? else {
            return Ok(());
        };
        let argv = [spoken(&script)?, spoken(copy.path())?];
        match tokio::time::timeout(PREPARED_WITHIN, self.runner.run_from(&argv, self.root)).await {
            Ok(Ok(output)) => match output.status {
                Some(0) => Ok(()),
                Some(exit) => Err(format!("{PREPARE} exited {exit}")),
                None => Err(format!("{PREPARE} was ended by a signal")),
            },
            Ok(Err(failure)) => Err(format!("{PREPARE} could not be run: {failure}")),
            Err(_overran) => Err(format!(
                "{PREPARE} was still running at its limit of {} seconds, and was stopped",
                PREPARED_WITHIN.as_secs()
            )),
        }
    }
}

/// `path` as a program's argument, where it is UTF-8.
fn spoken(path: &Path) -> Result<String, String> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| format!("{} is not UTF-8, so {PREPARE} was not run", path.display()))
}

/// The file `named` beneath `root`, where it is a file of its own and every step to it a
/// directory of its own; none where it or a step to it is not there.
fn runnable(root: &Path, named: &Path) -> Result<Option<PathBuf>, String> {
    let at = root.join(named);
    let beneath = at.parent().unwrap_or(root);
    if linkless(root, beneath, &SHIPPED)? == Reached::Absent {
        return Ok(None);
    }
    match std::fs::symlink_metadata(&at) {
        Ok(kind) if kind.is_file() => Ok(Some(at)),
        Ok(_) => Err(unrun(&at.display().to_string())),
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(why) => Err(unseen(&at, &why)),
    }
}

/// A walk to a file a plugin ships to be run.
const SHIPPED: Refusals = Refusals {
    unkind: unrun,
    unread: unseen,
};

/// The refusal of a file a plugin ships to be run, reached through something that is not a
/// directory or is no file itself.
fn unrun(named: &str) -> String {
    format!("{named} is a link or a device, so it was not run")
}

/// The refusal of a file a plugin ships to be run, where it could not be looked at.
fn unseen(at: &Path, why: &std::io::Error) -> String {
    format!(
        "{} could not be looked at, so it was not run: {why}",
        at.display()
    )
}

#[cfg(test)]
mod tests;
