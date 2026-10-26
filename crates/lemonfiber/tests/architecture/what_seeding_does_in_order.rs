//! What seeding must act on, and what it must do before what.
//!
//! Two rules about one run. Where a service says its credential comes from is the thing
//! that decides whether it is ever read, so a source the manifest allows and nothing
//! here answers is a service whose key is never published — which reads downstream
//! as a service with nothing to say. And the request service refuses every call
//! until it has an owner, so registering the curators into it before that step
//! reports two failures and then fixes them further down the same run.
//!
//! Both are pinned against the source of the seeding run rather than against a
//! recorded outcome: the first is one list agreeing with another, and the second is
//! an order that nothing in the types or the data would notice being reversed.

use std::fs;

use crate::source_tree::{production, sources, workspace_root};

/// Every place a service can say its credential comes from is one lemonfiber reads.
///
/// A service's `api.key_source` is what decides whether its credential is ever read:
/// adding a source to the manifest without a reader to answer it leaves that service
/// declaring a key nothing reads, and the only symptom is a key that never gets
/// published — which reads downstream as a service with nothing to say. Which service it
/// is decides nothing: a part a plugin brings is asked over its contract, and its key is
/// read by the shape of the file it is written in.
///
/// The stack keeps its own list of sources it will accept, so this is the second half of
/// that pair: the stack refuses a source nothing here implements, and this refuses a
/// source nothing here acts on. Two lists agreeing is what makes either of them mean
/// anything.
#[test]
fn every_key_source_a_service_can_declare_is_acted_on() {
    let schema =
        fs::read_to_string(workspace_root().join("crates/lemonfiber-manifest/src/schema/api.rs"))
            .unwrap_or_default();
    let Some(block) = schema
        .split_once("pub enum KeySource {")
        .and_then(|(_, rest)| rest.split_once('}'))
        .map(|(block, _)| block)
    else {
        unreachable!("the manifest declares where a service's credential may come from");
    };
    let declared: Vec<&str> = block
        .lines()
        .map(str::trim)
        .filter(|line| !line.starts_with("///") && line.ends_with(','))
        .map(|line| line.trim_end_matches(','))
        .filter(|name| !name.is_empty())
        .collect();
    assert!(
        declared.len() > 3,
        "the key sources were not read: {declared:?}"
    );

    let acted_on: String = sources()
        .into_iter()
        .filter(|(path, _)| {
            path.to_string_lossy()
                .replace('\\', "/")
                .contains("lemonfiber-core/src/credential")
        })
        .map(|(_, text)| text)
        .collect();

    let ignored: Vec<&str> = declared
        .iter()
        .filter(|source| !acted_on.contains(&format!("KeySource::{source}")))
        .copied()
        .collect();
    assert!(
        ignored.is_empty(),
        "a service declaring one of these names a credential nothing reads: {ignored:?}"
    );
}

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
