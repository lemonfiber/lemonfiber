//! The user and group a service speaking no contract runs as, and which of them the
//! operator set.

use super::OPERATOR_FALLBACK;
use crate::config::{PGID_KEY, PUID_KEY};

/// The ids a service speaking no contract runs as, and which of them the operator set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OperatedIds {
    /// The user id.
    pub(crate) uid: u32,
    /// The group id.
    pub(crate) gid: u32,
    /// Whether the user id is the operator's `PUID`.
    pub(crate) uid_set: bool,
    /// Whether the group id is the operator's `PGID`.
    pub(crate) gid_set: bool,
}

/// The ids a service speaking no contract runs as: the operator's `PUID` and `PGID` where
/// each is set to an id, and otherwise `owner`'s, or the fallback user's where `owner` is
/// nothing.
pub(crate) fn operated_ids(
    operator: &dyn Fn(&str) -> Option<String>,
    owner: Option<(u32, u32)>,
) -> OperatedIds {
    let (uid, gid) = owner.unwrap_or((OPERATOR_FALLBACK, OPERATOR_FALLBACK));
    let set = |key: &str| operator(key).and_then(|value| value.trim().parse::<u32>().ok());
    let (puid, pgid) = (set(PUID_KEY), set(PGID_KEY));
    OperatedIds {
        uid: puid.unwrap_or(uid),
        gid: pgid.unwrap_or(gid),
        uid_set: puid.is_some(),
        gid_set: pgid.is_some(),
    }
}
