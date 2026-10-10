//! `identity.source`: what an identity source is asked, the service whose accounts the
//! household signs in with.
//!
//! Its first-run setup; the household's accounts, who somebody is by name and password,
//! and whether a signed-in account still stands; invitations made, taken back and
//! withdrawn; the libraries and the certificates an account's limits are said in; what one
//! account may watch; switching an account off; the devices signed in to one; and whether
//! a new device signs in by a code.

use lemonfiber_ports::service::{
    Allowed, Certificate, Household, Invited, MediaServer, Member, NamedLibrary, Session, Signed,
};

crate::contract! {
    /// `identity.source`: an identity source.
    pub mod source = "identity.source" @ 1 {
        impl MediaServer {
            /// Whether its first-run setup is done.
            fn startup_completed() -> bool;
            /// Create its first administrator.
            fn create_admin(str name: &str, str password: &str) -> ();
        }
        impl Household {
            /// Every account.
            fn household() -> Vec<Member>;
            /// Who somebody is, by name and password, signing in from one device.
            fn whoever(str name: &str, str password: &str, str device: &str) -> Option<Signed>;
            /// Whether a signed-in account still stands.
            fn standing(refer signed: &Signed as Signed) -> bool;
            /// Make an account for somebody invited.
            fn invite(str name: &str) -> Member;
            /// Set the first password on an unclaimed account, signed in as it.
            fn claim(str name: &str, str password: &str, str device: &str) -> bool;
            /// Take an unclaimed invitation back.
            fn unclaim(str id: &str) -> ();
            /// Withdraw an account.
            fn withdraw(str id: &str) -> ();
            /// When each invitation since a moment was sent.
            fn when_invited(str since: &str) -> Vec<Invited>;
            /// The libraries.
            fn libraries() -> Vec<NamedLibrary>;
            /// The certificates its rating table names.
            fn ratings() -> Vec<Certificate>;
            /// Set what one account may watch.
            fn allow(str id: &str, refer allowed: &Allowed as Allowed) -> ();
            /// Put an account where its person can claim it.
            fn claimable(str id: &str, refer allowed: &Allowed as Allowed) -> ();
            /// Switch an account off.
            fn suspend(str id: &str) -> ();
            /// The devices signed in to one account.
            fn sessions(str member: &str) -> Vec<Session>;
            /// Whether a new device signs in by a code.
            fn signs_devices_in() -> bool;
        }
    }
}
