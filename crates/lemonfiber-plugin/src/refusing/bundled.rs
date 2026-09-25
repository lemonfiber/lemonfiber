//! What the stack this build ships already holds, which a plugin may not take.
//!
//! Read out of the shipped stack description and the proxy configuration beside it,
//! rather than written down here as a list. A list would be a second answer to a
//! question those two files already answer, and it would be free to disagree with
//! them: a service dropped from the stack would go on reserving its id and its port,
//! one added would reserve neither, and nothing would say either had happened. These
//! are the files the binary is built from, so the pin moves and this moves with it.
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
const STACK: &str = include_str!("../../../../assets/media-stack/stack.toml");

/// The proxy configuration beside it, which says which names the stack answers on.
///
/// The hostnames are here and not in the stack description because they are the
/// proxy's own vocabulary — a label in front of the operator's domain rather than a
/// property of a service — and the shipped file is where they are written.
const PROXY: &str = include_str!("../../../../assets/media-stack/config/caddy/Caddyfile");

/// What a proxied name is written in front of, in the shipped configuration.
const DOMAIN: &str = ".{$DOMAIN";

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

/// Every name the shipped proxy is written to answer on.
///
/// The stanzas that are commented out count. They are not dead text: the shipped
/// file writes each one out with what enabling it would mean, so they are names the
/// operator has already been offered — and a plugin holding one would turn that
/// offer into a collision on the day it was taken up, in a file the operator is
/// editing by hand and would have no reason to suspect.
pub(super) fn hostnames() -> &'static [String] {
    static ANSWERED: LazyLock<Vec<String>> = LazyLock::new(|| proxied(PROXY));
    &ANSWERED
}

/// The shipped service with this id, where the stack ships one.
pub(super) fn named(id: &str) -> Option<&'static Service> {
    services().iter().find(|service| service.id == id)
}

/// The shipped service published on this port, where the stack publishes one there.
pub(super) fn publishing(port: u16) -> Option<&'static Service> {
    services().iter().find(|service| service.port == Some(port))
}

/// Whether the shipped proxy is written to answer on this name.
pub(super) fn answering(label: &str) -> bool {
    hostnames().iter().any(|held| held == label)
}

/// Every label the proxy configuration puts in front of the operator's domain.
///
/// Read off the one shape the file is written in — a label, the domain placeholder,
/// and a block — with a leading comment marker allowed, because a disabled stanza is
/// still a name the file has spoken for. Anything else in the file says nothing
/// about a name, including the prose that mentions the placeholder without putting a
/// label in front of it.
fn proxied(text: &str) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    for line in text.lines() {
        let said = line.trim_start().trim_start_matches('#').trim_start();
        let Some((label, _)) = said.split_once(DOMAIN) else {
            continue;
        };
        if label.is_empty() || !label.chars().all(is_label) {
            continue;
        }
        if !found.iter().any(|held| held == label) {
            found.push(label.to_owned());
        }
    }
    found
}

/// Whether one character may appear in a DNS label.
fn is_label(letter: char) -> bool {
    letter.is_ascii_lowercase() || letter.is_ascii_digit() || letter == '-'
}

#[cfg(test)]
mod tests;
