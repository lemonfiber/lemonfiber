//! What seeding must do before what.
//!
//! The request service refuses every call until it has an owner, so registering the
//! curators into it before that step reports two failures and then fixes them further
//! down the same run.
//!
//! Pinned against the source of the seeding run rather than against a recorded outcome:
//! it is an order that nothing in the types or the data would notice being reversed.

use std::fs;

use crate::source_tree::{production, workspace_root};

/// The request service is given an owner before anything is registered into it.
///
/// It refuses every call until it has one, and the step that gives it one is the
/// identity wiring. Registering the curators into it first is not merely early — the
/// service answers "refused the credential", which a seed run reports as two failed
/// connections. It then, further down the same run, sets up the service that had just
/// refused it, so a fresh stack reported a fault it had already fixed by the time
/// anybody read it.
///
/// Pinned by the order the calls appear in, because that is the whole of the
/// dependency: neither step takes anything from the other, so nothing else in the
/// types or the data would notice them being swapped back.
#[test]
fn the_request_service_is_set_up_before_anything_is_registered_into_it() {
    let seed = fs::read_to_string(workspace_root().join("crates/lemonfiber-core/src/seed/run.rs"))
        .unwrap_or_default();
    let shipped = production(&seed);

    // The request service is set up among the media server's steps, which run as one.
    let helper = shipped
        .find("async fn seed_media_server(")
        .unwrap_or(shipped.len());
    assert!(
        shipped
            .get(helper..)
            .is_some_and(|body| body.contains("seed_request_identity(")),
        "the request service is set up among the media server's steps"
    );
    let (Some(identity), Some(targets)) = (
        shipped.find("seed_media_server("),
        shipped.find("seed_fulfilment_targets("),
    ) else {
        unreachable!("seeding sets up the request service and registers the curators into it");
    };
    assert!(
        identity < targets,
        "the curators are registered into the request service before it is given an \
         owner, so a fresh stack reports two failures and then fixes them in the \
         same run"
    );
}
