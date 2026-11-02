//! What the stack this build ships already holds, which a plugin may not take.
//!
//! Read out of the shipped stack description and the parts the bundle pins, rather
//! than written down here as a list. A list would be a second answer to a question those
//! already answer, and it would be free to disagree with them: a service dropped from the
//! stack would go on reserving its id and its port, one added would reserve neither, and
//! nothing would say either had happened. These are what the binary is built from, so
//! the pin moves and this moves with it.
//!
//! **A register that reads empty refuses nothing, and cannot notice that about
//! itself.** Every rule over one walks what it found, so a register that found
//! nothing passes exactly as quietly as a stack a manifest happens not to collide
//! with. What holds this one up is therefore not in it: the tests below ask it what
//! it found and hold that to what the shipped files declare, so a read that silently
//! stopped working is a failure rather than a silence.

use std::sync::LazyLock;

use lemonfiber_manifest::{Manifest, Service};

/// The stack description this build ships, which says which ids and ports are taken.
const STACK: &str = lemonfiber_bundled::STACK;

/// Every service the shipped stack declares.
///
/// Read once, from a file this binary is compiled with. Deliberately not a latch:
/// a latch is settled by one caller and read by the rest, and a read that settled
/// it would make a later settle silently do nothing. There is no caller to hand
/// this a value — the answer is a function of the shipped description alone — so
/// the construct that says so is the one with no settle to be ignored.
pub(super) fn services() -> &'static [Service] {
    static SHIPPED: LazyLock<Vec<Service>> = LazyLock::new(|| declared(STACK));
    &SHIPPED
}

/// The services one stack description declares, or none where it cannot be read.
///
/// Apart from the register above so that both of its answers can be asked for. An
/// unreadable description yields no services, and every rule over the register then
/// walks nothing — which is invisible from inside those rules and has to be a test's
/// job, so the reading is a function a test can hand a description to.
fn declared(text: &str) -> Vec<Service> {
    Manifest::from_toml(text).map_or_else(|_| Vec::new(), |stack| stack.services)
}

/// Every name the household reaches a part the bundle pins at, with the stack's service
/// that part's service stands in for, or its own id where it stands in for none.
///
/// The label is each household service's entry, as an installed plugin's is: the one its
/// manifest names, or its id. Every part counts, installed or still assembled into the
/// stack, because the household reaches both.
fn routed() -> &'static [(String, String)] {
    static ROUTED: LazyLock<Vec<(String, String)>> = LazyLock::new(|| {
        lemonfiber_bundled::PINNED
            .iter()
            .filter_map(|pinned| crate::Manifest::from_toml(pinned.copy).ok())
            .flat_map(|part| labelled(&part))
            .collect()
    });
    &ROUTED
}

/// Each label `part`'s household services are reached at, with the stack's service each
/// stands in for, or its own id.
fn labelled(part: &crate::Manifest) -> Vec<(String, String)> {
    part.services
        .iter()
        .filter(|service| service.port.is_some() && service.bind == Some(crate::Bind::Lan))
        .map(|service| {
            (
                part.entry(service).hostname.to_owned(),
                service
                    .replaces
                    .clone()
                    .unwrap_or_else(|| service.id.clone()),
            )
        })
        .collect()
}

/// The shipped service with this id, where the stack ships one.
pub(super) fn named(id: &str) -> Option<&'static Service> {
    services().iter().find(|service| service.id == id)
}

/// Whether lemonfiber holds a credential for the shipped service `id`: a part the bundle
/// installs moves its own service in place of it and hands the adapter in front of that
/// service a key lemonfiber minted for it.
pub(super) fn credential_held(id: &str) -> bool {
    minted_for(id)
}

/// Whether a part the bundle installs moves its own service in place of the shipped
/// service `id` and mints the key the adapter in front of that service is handed.
fn minted_for(id: &str) -> bool {
    lemonfiber_bundled::PINNED
        .iter()
        .filter(|pinned| pinned.stage == Some(lemonfiber_bundled::Stage::Installed))
        .filter_map(|pinned| crate::Manifest::from_toml(pinned.copy).ok())
        .any(|part| {
            let moves = part
                .services
                .iter()
                .any(|service| service.id == id && service.replaces.as_deref() == Some(id));
            let handed = part
                .services
                .iter()
                .filter(|service| service.fronts.as_deref() == Some(id))
                .flat_map(|service| service.upstream.values())
                .any(|named| {
                    part.secrets.iter().any(|secret| {
                        secret.minted.is() && secret.of == id && secret.id == named.secret
                    })
                });
            moves && handed
        })
}

/// The service the household reaches at this name, where it reaches a part the bundle
/// pins there.
pub(super) fn routed_to(label: &str) -> Option<&'static str> {
    routed()
        .iter()
        .find(|(held, _)| held == label)
        .map(|(_, service)| service.as_str())
}

/// The name the household reaches the stack's service `service` at, where a part the
/// bundle pins declares one for it.
pub(super) fn label_of(service: &str) -> Option<&'static str> {
    routed()
        .iter()
        .find(|(_, held)| held == service)
        .map(|(label, _)| label.as_str())
}

#[cfg(test)]
mod tests;
