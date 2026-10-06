//! What a rehearsal of asking the keys for something comes to.

use super::{cannot, reads, reports, Asked};
use crate::keys::run::Asked as Keyed;
use crate::model::kind;

/// Why minting a key cannot be rehearsed.
const A_KEY_IS_ITS_SECRET: &str = "a key is its secret — a rehearsed one would \
     be a real secret somebody could hold, made by a run that promised to make nothing";

/// A listing reads; a revoke says which key it would refuse and refuses none; a mint
/// cannot be rehearsed, because a key is its secret.
pub(super) const fn keyed(asked: &Keyed) -> Asked {
    match asked {
        Keyed::List => reads("key list"),
        Keyed::Mint { .. } => cannot("key mint", A_KEY_IS_ITS_SECRET),
        Keyed::Revoke { .. } => reports("key revoke", &[kind::KEYS]),
    }
}
