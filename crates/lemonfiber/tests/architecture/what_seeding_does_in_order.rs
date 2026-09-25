//! What seeding must act on, and what it must do before what.
//!
//! Two rules about one run. A service's declared API is the thing that decides
//! whether its credential is ever read, so a kind the manifest allows and nothing
//! here answers is a service whose key is never published — which reads downstream
//! as a service with nothing to say. And the request service refuses every call
//! until it has an owner, so registering the \*arrs into it before that step
//! reports two failures and then fixes them further down the same run.
//!
//! Both are pinned against the source of the seeding run rather than against a
//! recorded outcome: the first is one list agreeing with another, and the second is
//! an order that nothing in the types or the data would notice being reversed.

use std::fs;

use crate::source_tree::{production, sources, workspace_root};

/// Every API a service can declare is one seeding acts on, or one deferred by name.
///
/// A service's `api.kind` is what decides whether its credential is ever read: adding
/// a kind to the manifest without a client to answer it leaves that service declaring
/// an API nothing speaks, and the only symptom is a key that never gets published —
/// which reads downstream as a service with nothing to say.
///
/// The stack keeps its own list of kinds it will accept, so this is the second half of
/// that pair: the stack refuses a kind nothing here implements, and this refuses a kind
/// nothing here acts on. Two lists agreeing is what makes either of them mean anything.
///
/// One kind is deferred rather than missing: the book indexer's wiring waits on a live
/// instance to pin its endpoints against. The exception is read from `unsupported.rs`,
/// which is where the runtime reads it, so it is stated once and a shape that stops
/// being deferred stops being exempt here by the same edit that makes it speakable.
#[test]
fn every_api_a_service_can_declare_is_acted_on() {
    let deferred_source =
        fs::read_to_string(workspace_root().join("crates/lemonfiber-core/src/unsupported.rs"))
            .unwrap_or_default();
    let Some(exception) = deferred_source
        .split_once("pub const DEFERRED:")
        .and_then(|(_, rest)| rest.split_once("];"))
        .map(|(block, _)| block)
    else {
        unreachable!("the core declares which API shapes it does not speak");
    };
    let deferred: Vec<String> = exception
        .split("ApiKind::")
        .skip(1)
        .filter_map(|rest| rest.split(&[',', ' ', ')'][..]).next())
        .map(str::to_owned)
        .collect();
    assert!(
        !deferred.is_empty(),
        "the deferred shapes were not read, so every shape would look exempt: \
         {deferred:?}"
    );

    let schema =
        fs::read_to_string(workspace_root().join("crates/lemonfiber-manifest/src/schema.rs"))
            .unwrap_or_default();
    let Some(block) = schema
        .split_once("pub enum ApiKind {")
        .and_then(|(_, rest)| rest.split_once('}'))
        .map(|(block, _)| block)
    else {
        unreachable!("the manifest declares the API kinds a service may name");
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
        "the API kinds were not read: {declared:?}"
    );

    let acted_on: String = sources()
        .into_iter()
        .filter(|(path, _)| {
            path.to_string_lossy()
                .replace('\\', "/")
                .contains("/src/app")
        })
        .map(|(_, text)| text)
        .collect();

    let ignored: Vec<&str> = declared
        .iter()
        .filter(|kind| !deferred.iter().any(|named| named == *kind))
        .filter(|kind| !acted_on.contains(&format!("ApiKind::{kind}")))
        .copied()
        .collect();
    assert!(
        ignored.is_empty(),
        "a service declaring one of these names an API nothing acts on, so its \
         credential is never read: {ignored:?}"
    );
}
/// The request service is given an owner before anything is registered into it.
///
/// It refuses every call until it has one, and the step that gives it one is the
/// identity wiring. Registering the \*arrs into it first is not merely early — the
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

    let (Some(identity), Some(targets)) = (
        shipped.find("seed_jellyfin_identity("),
        shipped.find("seed_fulfilment_targets("),
    ) else {
        unreachable!("seeding sets up the request service and registers the *arrs into it");
    };
    assert!(
        identity < targets,
        "the *arrs are registered into the request service before it is given an \
         owner, so a fresh stack reports two failures and then fixes them in the \
         same run"
    );
}
