use super::{Consent, Preview, MOVED_ON};
use crate::backup::{Manifest, Relocation, Scope};

/// A listing of one archive, re-pointing onto `now`.
fn listed(now: &str) -> Preview {
    let manifest = Manifest {
        schema: crate::backup::SCHEMA,
        product_version: "0.8.0".to_owned(),
        created_at: "2026-08-21T00:00:00Z".to_owned(),
        data_root: "/srv/media".to_owned(),
        scope: Scope::WholeStack,
        sensitive: true,
        members: Vec::new(),
    };
    let relocation = Relocation {
        was: "/srv/media".to_owned(),
        now: now.to_owned(),
    };
    super::super::listing(manifest, false, Some(relocation))
}

/// A consent given for the listing that was read.
fn given(listing: &str) -> Consent {
    Consent::Given {
        listing: listing.to_owned(),
    }
}

/// Only a run that was told to overwrite overwrites anything.
#[test]
fn only_a_yes_overwrites() {
    assert!(!Consent::List.overwrites());
    assert!(given("00000000").overwrites());
    assert!(Consent::Standing.overwrites());
}

/// The listing that was read is compared with the listing that stands, so a yes
/// cannot be spent on a re-point the operator never saw.
///
/// Both halves: the same listing is accepted, and it is the *same* consent that
/// the moved-on listing refuses — so this is a comparison rather than a refusal
/// of everything.
#[test]
fn consent_given_for_one_listing_is_not_spent_on_another() {
    let read = listed("/mnt/library");
    let consent = given(&read.agreement);

    assert!(consent.held(&read).is_ok(), "the listing that was read");

    // The same archive, and a data root that has moved under it: the same
    // restore, re-pointing somewhere the operator never agreed to.
    let moved = listed("/mnt/somewhere-else");
    let refused = consent
        .held(&moved)
        .err()
        .map(|problem| (problem.code, problem.meaning.clone()));
    let (code, meaning) = refused.unwrap_or((MOVED_ON, String::new()));
    assert_eq!(code, MOVED_ON);
    assert!(meaning.contains(&read.agreement), "{meaning}");
    assert!(meaning.contains(&moved.agreement), "{meaning}");
}

/// The two that name no listing are never stale, because neither read one.
#[test]
fn a_consent_that_named_no_listing_is_never_stale() {
    let moved = listed("/mnt/somewhere-else");

    assert!(Consent::List.held(&moved).is_ok());
    assert!(Consent::Standing.held(&moved).is_ok());
}
