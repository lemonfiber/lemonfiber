//! Nobody in the house needs an account here, and this holds it to the types.
//!
//! Somebody who lives here signs into Jellyfin to watch and into Seerr to ask for
//! something, and both authenticate against the one Jellyfin account they were
//! given. This program is not in that path. It has one credential of its own — the
//! operator's, for the web surface — and no way to hold a second.
//!
//! That is true today by there being nothing to make it false, which is the kind of
//! truth that lasts until somebody adds a login. What makes it structural rather
//! than a promise is that the whole credential surface is *identity-free*: a
//! password is set, read back and verified with nowhere to say whose it is. There is
//! no argument to pass a name to, so there is no second person to admit.
//!
//! Held against the signatures rather than against the prose describing them. A
//! sweep for the words the source uses would pass a household login added under
//! different wording, and go red on a doc comment that was only rephrased — wrong in
//! both directions at once.

use std::path::Path;

use lemonfiber_core::admission::credential::{self, Credential, Weak};
use lemonfiber_core::ports::random::Random;
use lemonfiber_fixtures::ports::Chance;
use lemonfiber_fixtures::support::a_password;

/// Nothing in the credential surface has anywhere to name a person.
///
/// The compiler is the assertion. Each coercion below stops building if the function
/// it names grows a parameter for whose credential is meant, which is the smallest
/// change that could give somebody in the house an account here: a password cannot
/// be set for somebody, looked up for somebody, or checked against somebody.
///
/// Written as coercions because it is the *shape* that carries the claim — what
/// these functions do when called is held beside them, and none of it would notice
/// an added argument.
#[test]
fn nothing_in_the_credential_surface_takes_an_identity() {
    let set: fn(&str, &dyn Random) -> Result<Credential, Weak> = Credential::set;
    let verifies: fn(&Credential, &str) -> bool = Credential::verifies;
    let read: fn(&Path) -> Option<Credential> = credential::at;

    // Named above so a changed signature is a build failure, and called here so the
    // claim is about the functions that admit somebody rather than three bindings
    // that happen to typecheck.
    let held = set(&a_password(), &Chance::cycling()).ok();
    assert!(
        held.is_some_and(|credential| verifies(&credential, &a_password())),
        "the password that was set is not the one this proves, so these are not the \
         functions the claim above is about"
    );

    assert!(
        read(Path::new("/nowhere/lemonfiber/credential")).is_none(),
        "an install where nobody set a password holds one anyway"
    );
}

/// What is written down says what proves a password, not whose it is.
///
/// The other direction the claim could be lost: an identity added to the record
/// rather than to the signatures. One field is what a store with nobody to tell
/// apart needs, and a second is the first thing a per-person account would want.
#[test]
fn what_is_kept_carries_no_owner() {
    let fields: Vec<String> = Credential::set(&a_password(), &Chance::cycling())
        .ok()
        .and_then(|held| serde_json::to_value(held).ok())
        .and_then(|kept| {
            kept.as_object()
                .map(|record| record.keys().cloned().collect())
        })
        .unwrap_or_default();

    assert_eq!(
        fields,
        ["verifier"],
        "an empty list means no record was written down and nothing here was read; \
         anything beside the proof of a password means the store gained something to \
         tell two people apart with"
    );
}
