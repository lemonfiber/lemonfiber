//! Where a service's network takes it, and what an egress service may not hold.

use super::{Container, Network, Posture, Value, VIA};

/// The capabilities that let a process mark a socket, which a service held inside the
/// stack never holds, so only its fence's own questions reach the engine's resolver.
const MARKING: [&str; 2] = ["NET_ADMIN", "NET_RAW"];

impl Container<'_> {
    pub(super) fn routed(&self, found: &mut Vec<(String, String)>) {
        let Some(network) = self.network else {
            return;
        };
        if network.internal {
            self.held_inside(network, found);
            return;
        }
        if let Some(sibling) = network.egress_for() {
            self.carrying(network, sibling, found);
            return;
        }
        let Some(via) = network.via() else {
            if !network.fenced {
                found.push((
                    self.at("network"),
                    "says none of via, fenced, internal and egress_for, and a network is one of them"
                        .to_owned(),
                ));
            }
            return;
        };
        if network.fenced {
            found.push((
                self.at("network"),
                "says both via and fenced, and each is a network namespace of its own".to_owned(),
            ));
        }
        if !VIA.contains(&via) {
            found.push((
                self.at("network.via"),
                format!("names {via}, which no service may run inside the network of"),
            ));
        }
        if self.provides.iter().any(|provided| provided == via) {
            found.push((
                self.at("network.via"),
                format!("names {via}, which the service provides itself"),
            ));
        }
    }

    /// What a service held inside the stack, under `network`, takes beside it that is a
    /// network namespace of its own.
    fn held_inside(&self, network: &Network, found: &mut Vec<(String, String)>) {
        let marking = self.shape.is_some_and(|shape| {
            shape
                .grants()
                .iter()
                .any(|granted| MARKING.contains(granted))
        });
        let beside = [
            (network.via().is_some(), "via"),
            (network.fenced, "fenced"),
            (network.egress_for().is_some(), "egress_for"),
            (marking, "a shape granting what marks a socket"),
            (
                self.provides
                    .iter()
                    .any(|provided| VIA.contains(&provided.as_str())),
                "provides a capability another service runs inside the network of",
            ),
        ];
        found.extend(beside.into_iter().filter(|(taken, _)| *taken).map(|(_, what)| {
            (
                self.at("network"),
                format!("says both internal and {what}, and each is a network namespace of its own"),
            )
        }));
    }

    /// What an egress service carrying `sibling` out of the machine, under `network`, holds
    /// that it may not: anything worth taking, any way in, or another network.
    fn carrying(&self, network: &Network, sibling: &str, found: &mut Vec<(String, String)>) {
        let held = [
            (sibling == self.id, "names the service itself"),
            (network.via().is_some(), "takes via beside it"),
            (network.fenced, "is fenced beside it"),
            (self.posture != Posture::Confined, "is not confined"),
            (!self.secrets.is_empty(), "declares a secret"),
            (
                self.config_path.is_some(),
                "declares a configuration directory",
            ),
            (
                !self.mounts.is_empty() || self.shares_config.is_some(),
                "declares a mount",
            ),
            (self.takes_data, "takes the data mount"),
            (
                self.port.is_some() || !self.publish.is_empty(),
                "publishes a port",
            ),
            (self.speaks, "speaks a contract"),
            (!self.provides.is_empty(), "provides a capability"),
            (
                self.env
                    .values()
                    .any(|value| matches!(value, Value::Url(_))),
                "asks the address of a capability",
            ),
        ];
        found.extend(
            held.into_iter()
                .filter(|(breaks, _)| *breaks)
                .map(|(_, what)| {
                    (
                self.at("network.egress_for"),
                format!("{what}, and a service carrying another out of the machine holds nothing"),
            )
                }),
        );
    }
}
