//! Where a service's network takes it, and what an egress service may not hold.

use super::{Container, Network, Posture, Value, VIA};

impl Container<'_> {
    pub(super) fn routed(&self, found: &mut Vec<(String, String)>) {
        let Some(network) = self.network else {
            return;
        };
        if let Some(sibling) = network.egress_for() {
            self.carrying(network, sibling, found);
            return;
        }
        let Some(via) = network.via() else {
            if !network.fenced {
                found.push((
                    self.at("network"),
                    "says none of via, fenced and egress_for, and a network is one of them"
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
