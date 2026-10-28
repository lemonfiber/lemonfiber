//! What a plugin's adapter service says about the contracts it speaks.

use std::collections::BTreeSet;

use lemonfiber_manifest::container::exposed;

use crate::schema::Service;
use crate::Violation;

/// Whether every contract the service speaks is one this build speaks, named once, of a
/// capability it provides, whether the service says where it answers and is reached no
/// other way, and whether it names the upstream it stands in front of among `services`.
pub(super) fn spoken(service: &Service, services: &[Service], found: &mut Vec<Violation>) {
    let at = format!("service {}", service.id);
    if service.speaks.is_empty() {
        if service.fronts.is_some() {
            found.push(Violation {
                location: format!("{at}.fronts"),
                message: "names an upstream, and only a service that speaks a contract stands \
                          in front of one"
                    .to_owned(),
            });
        }
        if service.contract.is_some() {
            found.push(Violation {
                location: format!("{at}.contract"),
                message: "names the port the service speaks its contracts on, and it speaks none"
                    .to_owned(),
            });
        }
        return;
    }
    fronting(service, services, &at, found);
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
    unexposed(service, &at, found);
}

/// Whether the port the service speaks its contracts on is reached by nothing but
/// lemonfiber on this machine's loopback, as [`exposed`] decides for the compose writer
/// and the proxy too.
fn unexposed(service: &Service, at: &str, found: &mut Vec<Violation>) {
    let Some(contract) = service.contract_port() else {
        return;
    };
    if let Some(how) = exposed(contract, service.port, service.listens, &service.publish) {
        let field = if service.contract.is_some() {
            "contract"
        } else {
            how.field()
        };
        found.push(Violation {
            location: format!("{at}.{field}"),
            message: format!(
                "is {contract}, the port the service speaks its contracts on, and {} reaches it, \
                 where lemonfiber publishes that port on this machine's loopback alone",
                how.said()
            ),
        });
    }
}

/// Whether the service names, where it names an upstream, one other service of its
/// plugin that speaks nothing; one naming none is its own upstream.
fn fronting(service: &Service, services: &[Service], at: &str, found: &mut Vec<Violation>) {
    let refused = match service.fronts.as_deref() {
        None => None,
        Some(fronted) if fronted == service.id => {
            Some("names the adapter itself, and an adapter stands in front of another".to_owned())
        }
        Some(fronted) => match services.iter().find(|one| one.id == fronted) {
            None => Some(format!(
                "names {fronted}, which this plugin does not declare"
            )),
            Some(upstream) if !upstream.speaks.is_empty() => Some(format!(
                "names {fronted}, which speaks a contract itself rather than being the upstream"
            )),
            Some(_) => None,
        },
    };
    if let Some(message) = refused {
        found.push(Violation {
            location: format!("{at}.fronts"),
            message,
        });
    }
}

/// The capability `named` is the contract of, where the service does not provide it and
/// it is not a facet, which is spoken and never provided.
fn unprovided<'a>(service: &Service, named: &'a str) -> Option<&'a str> {
    let capability = lemonfiber_contract::capability_of(named)?;
    (!lemonfiber_contract::capabilities::is_facet(capability)
        && !service
            .provides
            .iter()
            .any(|provided| provided == capability))
    .then_some(capability)
}

/// Whether `named` is `capability@major` for a contract this build speaks.
fn contracted(named: &str) -> bool {
    lemonfiber_contract::capabilities::all()
        .iter()
        .any(|capability| lemonfiber_contract::spoken(capability.name, capability.major) == named)
}

#[cfg(test)]
mod tests;
