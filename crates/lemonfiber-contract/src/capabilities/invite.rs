//! `invite.decline`: what the service a household member declines an invitation at is
//! asked.
//!
//! It is handed the one media server key it switches an invitation off with, answers
//! that key by its fingerprint alone, and is handed every invitation it may act on.

use lemonfiber_ports::service::{Declinable, Declining};

use super::{NOWHERE, TOKEN_HASH};
use crate::conformance::Placeholder;

/// When the conformance cases' invitation lapses: the start of 2100, in seconds since
/// the Unix epoch.
const LAPSES: u64 = 4_102_444_800;

crate::contract! {
    /// `invite.decline`: the decline service.
    pub mod decline = "invite.decline" @ 1 {
        impl Declining {
            /// Switch an invitation off with this key from now on.
            fn hold(str key: &str) -> () where asks = Asked { key: Placeholder::Minted.written() };
            /// The key it holds, by its fingerprint: `~` and the low sixteen bits of the 64-bit FNV-1a hash of its bytes, as four lower-case hexadecimal digits.
            fn held() -> Option<String> where asks = Asked {};
            /// Act on these invitations alone.
            fn apply_invitations(slice invitations: &[Declinable] as Declinable) -> () where asks = Asked {
                invitations: vec![Declinable {
                    token: TOKEN_HASH.to_owned(),
                    account: NOWHERE.to_owned(),
                    lapses: LAPSES,
                }],
            };
        }
    }
}
