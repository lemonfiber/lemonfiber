//! What a plugin's adapter service says about the contracts it speaks.

use std::collections::BTreeSet;

use crate::schema::Service;
use crate::Violation;

/// Whether every contract the service speaks is one this build speaks, named once, of a
/// capability it provides, and whether the service says where it answers and is reached
/// no other way.
pub(super) fn spoken(service: &Service, found: &mut Vec<Violation>) {
    if service.speaks.is_empty() {
        return;
    }
    let at = format!("service {}", service.id);
    let mut seen = BTreeSet::new();
    for named in &service.speaks {
        if !contracted(named) {
            found.push(Violation {
                location: format!("{at}.speaks"),
                message: format!(
                    "{named} is not a contract this build speaks; each is written \
                     capability@major, as contract/capabilities/index.json lists them"
                ),
            });
        } else if let Some(unprovided) = unprovided(service, named) {
            found.push(Violation {
                location: format!("{at}.speaks"),
                message: format!(
                    "names {named}, and the service does not provide {unprovided}, which \
                     speaking its contract is what filling it takes"
                ),
            });
        }
        if !seen.insert(named.as_str()) {
            found.push(Violation {
                location: format!("{at}.speaks"),
                message: format!("names {named} more than once"),
            });
        }
    }
    if service.listens.is_none() {
        found.push(Violation {
            location: format!("{at}.listens"),
            message: "is absent, and a service that speaks a contract has to say the port it \
                      answers on, or lemonfiber could not ask it"
                .to_owned(),
        });
    }
    if service.port.is_some() && service.port == service.listens {
        found.push(Violation {
            location: format!("{at}.port"),
            message: "is the port the service speaks its contracts on, which lemonfiber publishes \
                      on this machine's loopback alone"
                .to_owned(),
        });
    }
    if service.api.is_some() {
        found.push(Violation {
            location: format!("{at}.api"),
            message: "names an adapter in lemonfiber beside contracts the service speaks, and a \
                      service is asked one way"
                .to_owned(),
        });
    }
}

/// The capability `named` is the contract of, where the service does not provide it.
fn unprovided<'a>(service: &Service, named: &'a str) -> Option<&'a str> {
    let (capability, _) = named.split_once('@')?;
    (!service
        .provides
        .iter()
        .any(|provided| provided == capability))
    .then_some(capability)
}

/// Whether `named` is `capability@major` for a contract this build speaks.
fn contracted(named: &str) -> bool {
    lemonfiber_contract::capabilities::all()
        .iter()
        .any(|capability| format!("{}@{}", capability.name, capability.major) == named)
}

#[cfg(test)]
mod tests;
