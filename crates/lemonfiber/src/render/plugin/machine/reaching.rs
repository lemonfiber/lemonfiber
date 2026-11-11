//! How each of a plugin's services is reached, in the terms its tier decides.

use lemonfiber_core::plugin::Reached;

/// How a service is reached, in the terms the tier decides, a proxied one on this machine
/// alone where `unproxied` says the stack runs no proxy.
///
/// A loopback service says so and names no address: lemonfiber renders one from the
/// tier, and a line here that spelled one out would be a second answer free to
/// disagree with the one the stack is written from.
pub(super) fn reached(reached: Option<&Reached>, unproxied: bool) -> String {
    match reached {
        None => "nothing — it publishes no port".to_owned(),
        Some(Reached::Loopback { port, group }) => {
            format!(
                "this machine only, on port {port}{}",
                panel(group.as_deref())
            )
        }
        Some(Reached::Proxied { port, group, .. }) if unproxied => format!(
            "this machine only, on port {port}, since the stack runs no proxy to reach it by \
             name{}",
            panel(group.as_deref())
        ),
        Some(Reached::Proxied {
            port,
            hostname,
            group,
        }) => format!(
            "this machine on port {port}, and the household as {hostname} through the \
             stack's proxy{}",
            panel(group.as_deref())
        ),
        Some(Reached::Household {
            port,
            hostname,
            group,
        }) => format!(
            "the household, as {hostname}, on port {port}{}",
            panel(group.as_deref())
        ),
    }
}

/// The dashboard group, where the manifest named one.
fn panel(group: Option<&str>) -> String {
    group.map_or_else(String::new, |group| format!(", under {group}"))
}
