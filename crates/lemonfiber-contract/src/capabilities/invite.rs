//! `invite.decline`: what the service a household member declines an invitation at is
//! asked.
//!
//! It is handed the one media server key it switches an invitation off with and the
//! certificates the media server's internal TLS port may present, answers each by its
//! fingerprint alone, and is handed every invitation it may act on.

use lemonfiber_ports::service::{Claimed, Declinable, Declining, Invitations, KeyHeld};

use super::{CERTIFICATE_PIN, NAMED, NOWHERE, TOKEN_HASH};
use crate::conformance::Placeholder;

/// When the conformance cases' invitation was issued: the start of 2026, in seconds since
/// the Unix epoch.
const ISSUED: u64 = 1_767_225_600;

/// When the conformance cases' invitation lapses: the start of 2100, in seconds since
/// the Unix epoch.
const LAPSES: u64 = 4_102_444_800;

crate::contract! {
    /// `invite.decline`: the decline service.
    pub mod decline = "invite.decline" @ 1 {
        impl Declining {
            /// Switch an invitation off with this key from now on, reaching the media server only over TLS presenting one of these certificates.
            fn hold(str key: &str, slice pins: &[String] as String) -> () where asks = Asked {
                key: Placeholder::Minted.written(),
                pins: vec![CERTIFICATE_PIN.to_owned()],
            };
            /// The key it holds and the certificates it accepts, each by its SHA-256 in lower-case hexadecimal: never the key itself.
            fn held() -> Option<KeyHeld> where asks = Asked {};
            /// Act on these invitations alone, telling a claimed account as the media server does.
            fn apply_invitations(refer invitations: &Invitations as Invitations) -> () where asks = Asked {
                invitations: Invitations {
                    claimed: Claimed::Unknown,
                    invitations: vec![Declinable {
                        token: TOKEN_HASH.to_owned(),
                        account: NOWHERE.to_owned(),
                        name: NAMED.to_owned(),
                        issued: ISSUED,
                        lapses: LAPSES,
                    }],
                },
            };
        }
    }
}
