//! What a read of what setup settled answers with.
//!
//! One of the report families the machine-readable contract is made of; they live in
//! separate files and are re-exported as one, so `crate::model::X` reads the same as it
//! always did.

use serde::Serialize;

/// Whether this stack is set up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum SetUp {
    /// Nothing is configured and no setup has begun.
    None,
    /// A setup has begun and not finished: answers are being gathered, are waiting for
    /// review, or an apply stopped part-way.
    PartWay,
    /// The stack is configured and no setup is part-way through.
    Done,
}

/// Where the user the services run as comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum UserFrom {
    /// The operator set both `PUID` and `PGID`.
    Set,
    /// The operator set one of `PUID` and `PGID`, and the other is the stack
    /// directory's owner's.
    PartSet,
    /// The operator set neither, so both are the stack directory's owner's, or 1000
    /// where that owner is root or cannot be read.
    StackOwner,
}

/// The user and group the stack's services run as.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct ServiceUser {
    /// The user id.
    pub uid: u32,
    /// The group id.
    pub gid: u32,
    /// Where the two come from.
    pub from: UserFrom,
}

/// What setup settled, read from the configuration as it stands, and whether this stack
/// is set up.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct SettledReport {
    /// Whether this stack is set up.
    pub setup: SetUp,
    /// Where the library is kept, a path on this machine or an NFS export as `DATA_ROOT`
    /// names it, or nothing where no data root is configured.
    pub data_root: Option<String>,
    /// Which ways of downloading are configured.
    pub protocols: crate::config::Protocols,
    /// The user and group the stack's services run as.
    pub service_user: ServiceUser,
}
