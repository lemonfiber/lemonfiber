//! What a rehearsal of asking the keys for something comes to.

use super::{cannot, reads, reports, Asked};
use crate::device_keys::run::Asked as Devices;
use crate::keys::run::Asked as Keyed;
use crate::model::kind;
use crate::notify::configuring::Asked as Notifying;
use crate::push::run::Asked as Pushing;

/// Why minting a key cannot be rehearsed.
const A_KEY_IS_ITS_SECRET: &str = "a key is its secret — a rehearsed one would \
     be a real secret somebody could hold, made by a run that promised to make nothing";

/// Why registering a device key cannot be rehearsed.
const A_REGISTRATION_IS_PROVED: &str = "a device key is registered with the operator's \
     password and the phone's attestation in the same request, and a rehearsal carries neither";

/// A listing reads; a revoke says which key it would refuse and refuses none; a mint
/// cannot be rehearsed, because a key is its secret.
pub(super) const fn keyed(asked: &Keyed) -> Asked {
    match asked {
        Keyed::List { .. } => reads("key list"),
        Keyed::Mint { .. } => cannot("key mint", A_KEY_IS_ITS_SECRET),
        Keyed::Collect { .. } => cannot("key collect", A_KEY_IS_ITS_SECRET),
        Keyed::Revoke { .. } => reports("key revoke", &[kind::KEYS]),
    }
}

/// A listing reads; a revoke says which device key it would revoke and revokes none; a
/// registration cannot be rehearsed, because it is proved by what a rehearsal carries
/// none of.
pub(super) const fn devices(asked: &Devices) -> Asked {
    match asked {
        Devices::List => reads("device-key list"),
        Devices::Register { .. } => cannot("device-key register", A_REGISTRATION_IS_PROVED),
        Devices::Revoke { .. } => reports("device-key revoke", &[kind::DEVICE_KEYS]),
    }
}

/// A listing reads; every change and a test each say what they would come to.
pub(super) const fn notifying(asked: &Notifying) -> Asked {
    match asked {
        Notifying::List => reads("notify list"),
        Notifying::Add(_) => reports("notify add", &[kind::NOTIFY]),
        Notifying::Set(_) => reports("notify set", &[kind::NOTIFY]),
        Notifying::Test { .. } => reports("notify test", &[kind::NOTIFY]),
        Notifying::Remove { .. } => reports("notify remove", &[kind::NOTIFY]),
        Notifying::Mine { .. } => reads("notify mine"),
        Notifying::Choose { .. } => reports("notify choose", &[kind::NOTIFY_MINE]),
    }
}

/// A listing reads; a registration and a removal each say what they would come to.
pub(super) const fn pushing(asked: &Pushing) -> Asked {
    match asked {
        Pushing::List { .. } => reads("push-endpoint list"),
        Pushing::Register { .. } => reports("push-endpoint register", &[kind::PUSH_ENDPOINT]),
        Pushing::Remove { .. } => reports("push-endpoint remove", &[kind::PUSH_ENDPOINTS]),
    }
}
