//! There is one way into the request service, and it is the account somebody
//! already has.
//!
//! A household member signs in to Seerr with their Jellyfin account — the same
//! credentials that let them watch — because this program points Seerr's identity at
//! the media server rather than letting it keep accounts of its own. That is what
//! makes a second registration unnecessary: not a setting that forbids one, but
//! there being nothing else to sign in through.
//!
//! Held by reading the endpoints this program asks for. That is a sweep of source
//! text, which is the wrong instrument when the claim is about behaviour and the
//! text merely describes it — here the claim *is* which endpoints are asked for, so
//! the strings are the subject rather than a description of it.
//!
//! What this does not claim: that Seerr's own local sign-in is switched off. Nothing
//! here touches that setting, and nothing asks for it — what is required is that no
//! separate registration be *required*, and an identity source somebody already has
//! an account with satisfies that whatever else the service permits.

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

/// The request service's client, as it ships.
///
/// Read up to where its own tests begin, so a path written only to be refused by a
/// fake does not read as one this program asks for.
fn shipped() -> String {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/seerr.rs");
    let text = fs::read_to_string(source).unwrap_or_default();
    text.find("\nmod tests {")
        .and_then(|at| text.get(..at).map(str::to_owned))
        .unwrap_or(text)
}

/// Every path this program asks the request service for that is a way in.
fn ways_in(source: &str) -> BTreeSet<String> {
    source
        .split('"')
        .filter(|piece| piece.starts_with("/auth"))
        .map(str::to_owned)
        .collect()
}

/// The one way in is the media server.
///
/// A second entry here would be a second set of credentials somebody in the house
/// could be asked for, which is the registration this exists to make unnecessary.
#[test]
fn the_only_way_in_is_through_the_media_server() {
    let source = shipped();

    assert!(
        source.contains("/settings/public"),
        "the request service's client was not read, so nothing below checked anything"
    );

    let found = ways_in(&source);
    assert_eq!(
        found.iter().map(String::as_str).collect::<Vec<_>>(),
        ["/auth/jellyfin"],
        "the request service is reached through a way in that is not the media \
         server, so somebody in the house can be asked for credentials they were \
         never given"
    );
}

/// Signing in says which media server it means.
///
/// The endpoint alone is not the claim: pointing at the media server is what makes
/// the household's existing account the one that works, and a sign-in that named no
/// server type would leave the service to guess.
#[test]
fn signing_in_names_the_media_server_it_authenticates_against() {
    let source = shipped();

    assert!(
        source.contains("\"serverType\""),
        "the sign-in no longer says what it is authenticating against, so the \
         household's existing account is not what this configures"
    );
    assert!(
        source.contains("\"hostname\""),
        "the sign-in names no media server to authenticate against"
    );
}
