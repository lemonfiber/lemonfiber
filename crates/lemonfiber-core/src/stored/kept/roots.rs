//! The two directories everything lemonfiber keeps sits under.

use crate::config::paths::Paths;
use crate::stored::Root;

/// The two directories everything lemonfiber keeps sits under.
pub(in crate::stored) fn roots(paths: &Paths) -> Vec<Root> {
    vec![
        Root {
            at: paths.config_dir().display().to_string(),
            what: "Your answers, and lemonfiber's memory of what it wrote. Small, worth keeping a \
                   copy of, and the half that cannot be made again."
                .to_owned(),
        },
        Root {
            at: paths.data_dir().display().to_string(),
            what: "What can be made again: the materialised stack, each service's own \
                   configuration, the archives, the bundles, and what a command this machine \
                   keeps running has said."
                .to_owned(),
        },
    ]
}
