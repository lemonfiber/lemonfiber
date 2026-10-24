//! `identity.source`: what an identity source is asked, the service whose accounts the
//! household signs in with.
//!
//! Its first-run setup; the household's accounts, who somebody is by name and password,
//! and whether a signed-in account still stands; invitations made, taken back and
//! withdrawn; the libraries and the certificates an account's limits are said in, and the
//! country they are named for; what one account may watch; switching an account off; the
//! devices signed in to one; and whether a new device signs in by a code.

use lemonfiber_ports::service::{
    Allowed, Certificate, Household, Invited, MediaServer, Member, NamedLibrary, Session, Signed,
    Unrated,
};

use crate::conformance::Placeholder;

/// The name the conformance cases invite somebody under.
const INVITED: &str = "conformance-member";

/// A moment before any account was made.
const LONG_AGO: &str = "2000-01-01T00:00:00Z";

/// The age limit the conformance cases set on the invited account.
const AGE_LIMIT: u32 = 12;

/// The string at `pointer` in this contract's answer to `operation`.
fn earlier(operation: &str, pointer: &str) -> String {
    Placeholder::answer(source::CAPABILITY, source::MAJOR, operation, pointer).written()
}

/// The invited account's id.
fn invited() -> String {
    earlier("invite", "/id")
}

/// The device a conformance case signs in from.
fn device(case: &str) -> String {
    format!("conformance-{case}")
}

crate::contract! {
    /// `identity.source`: an identity source.
    pub mod source = "identity.source" @ 1 {
        impl MediaServer {
            /// Whether its first-run setup is done.
            fn startup_completed() -> bool where asks = Asked {};
            /// Create its first administrator.
            fn create_admin(str name: &str, str password: &str) -> () where asks = Asked {
                name: Placeholder::Upstream("username".to_owned()).written(),
                password: Placeholder::Upstream("password".to_owned()).written(),
            };
            /// Change its administrator's password, signed in with the one in force.
            fn replace_admin_password(str replacement: &str) -> () where asks = Asked {
                replacement: Placeholder::MintedUpstream("password".to_owned()).written(),
            };
        }
        impl Household {
            /// Every account.
            fn household() -> Vec<Member> where asks = Asked {};
            /// Who somebody is, by name and password, signing in from one device.
            fn whoever(str name: &str, str password: &str, str device: &str) -> Option<Signed> where secret = &["/token"], asks = Asked {
                name: Placeholder::Upstream("username".to_owned()).written(),
                password: Placeholder::Upstream("password".to_owned()).written(),
                device: device("whoever"),
            };
            /// Whether a signed-in account still stands.
            fn standing(refer signed: &Signed as Signed) -> bool where asks = Asked {
                signed: Signed {
                    id: earlier("whoever", "/id"),
                    token: earlier("whoever", "/token"),
                },
            };
            /// Make an account for somebody invited.
            fn invite(str name: &str) -> Member where asks = Asked { name: INVITED.to_owned() };
            /// Set the first password on an unclaimed account, signed in as it.
            fn claim(str name: &str, str password: &str, str device: &str) -> bool where asks = Asked {
                name: INVITED.to_owned(),
                password: Placeholder::Minted.written(),
                device: device("claim"),
            };
            /// Take an unclaimed invitation back.
            fn unclaim(str id: &str) -> () where asks = Asked { id: invited() };
            /// When each invitation since a moment was sent.
            fn when_invited(str since: &str) -> Vec<Invited> where asks = Asked {
                since: LONG_AGO.to_owned(),
            };
            /// The libraries.
            fn libraries() -> Vec<NamedLibrary> where asks = Asked {};
            /// The certificates its rating table names.
            fn ratings() -> Vec<Certificate> where asks = Asked {};
            /// The country its rating table names certificates for.
            fn country() -> Option<String> where asks = Asked {};
            /// Set what one account may watch.
            fn allow(str id: &str, refer allowed: &Allowed as Allowed) -> () where asks = Asked {
                id: invited(),
                allowed: Allowed {
                    libraries: None,
                    age_limit: Some(AGE_LIMIT),
                    unrated: Some(Unrated::HeldBack),
                },
            };
            /// Put an account where its person can claim it.
            fn claimable(str id: &str, refer allowed: &Allowed as Allowed) -> () where asks = Asked {
                id: invited(),
                allowed: Allowed {
                    libraries: Some(Vec::new()),
                    age_limit: None,
                    unrated: None,
                },
            };
            /// Switch an account off.
            fn suspend(str id: &str) -> () where asks = Asked { id: invited() };
            /// The devices signed in to one account.
            fn sessions(str member: &str) -> Vec<Session> where asks = Asked {
                member: earlier("household", "/0/id"),
            };
            /// Whether a new device signs in by a code.
            fn signs_devices_in() -> bool where asks = Asked {};
            /// Withdraw an account.
            fn withdraw(str id: &str) -> () where asks = Asked { id: invited() };
        }
    }
}
