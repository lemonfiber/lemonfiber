//! `owner.claim`: what a part that belongs to whoever claims it first is asked.
//!
//! A facet: the core asks it of every service that speaks it, never choosing among
//! fillers. Who holds the part, and claiming it with the credential the plugin minted and
//! handed its adapter, which never crosses a call.

use lemonfiber_ports::service::{Claim, Owner, Ownership};

crate::contract! {
    /// `owner.claim`: a part that belongs to whoever claims it first.
    pub mod claim = "owner.claim" @ 1 {
        impl Ownership {
            /// Who holds it now.
            fn owner() -> Owner where asks = Asked {};
            /// Claim it with the credential the adapter holds where nobody holds it, changing nothing where somebody does.
            fn claim() -> Claim where asks = Asked {};
        }
    }
}
