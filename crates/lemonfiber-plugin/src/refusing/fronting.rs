//! Which of a plugin's services may name the API it answers the stack's other services in,
//! and the conformance cases an adapter is asked that reach a host outside the stack.

use crate::schema::Service;
use crate::Violation;

/// Whether a service naming its native API names one word, and is the upstream another
/// service of the same plugin fronts.
pub(super) fn native(service: &Service, services: &[Service], found: &mut Vec<Violation>) {
    let Some(native) = service.native.as_deref() else {
        return;
    };
    let location = format!("service {}.native", service.id);
    if !lemonfiber_manifest::container::is_api_name(native) {
        found.push(Violation {
            location: location.clone(),
            message: format!(
                "names {native}, and the API a service answers in is named in one lowercase \
                 word"
            ),
        });
    }
    if !fronted(service, services) {
        found.push(Violation {
            location,
            message: "names the API this service answers in, which only the upstream another \
                      service of this plugin fronts may name"
                .to_owned(),
        });
    }
}

/// The longest DNS name, and the longest label in one.
const DNS_MOST: (usize, usize) = (253, 63);

/// Whether a service naming the cases it is asked that reach outside the stack fronts
/// another, names each as a case of a contract it speaks, and names a DNS name for each.
pub(super) fn outside(service: &Service, found: &mut Vec<Violation>) {
    if service.outside.is_empty() {
        return;
    }
    let location = format!("service {}.outside", service.id);
    if service.fronts.is_none() {
        found.push(Violation {
            location: location.clone(),
            message: "names cases reaching outside the stack, which only an adapter that fronts                       another service is asked"
                .to_owned(),
        });
    }
    for (case, host) in &service.outside {
        if !spoken_case(service, case) {
            found.push(Violation {
                location: location.clone(),
                message: format!(
                    "names {case}, which is no case of a contract this service speaks, written                      <capability>@<major>/<case>"
                ),
            });
        }
        if !dns_name(host) {
            found.push(Violation {
                location: location.clone(),
                message: format!("says {case} reaches {host}, which is not a DNS name"),
            });
        }
    }
}

/// Whether `named` is `<capability>@<major>/<case>` naming a case of a contract `service`
/// speaks.
fn spoken_case(service: &Service, named: &str) -> bool {
    let Some((spoken, case)) = named.split_once('/') else {
        return false;
    };
    service.speaks.iter().any(|one| one == spoken)
        && lemonfiber_contract::capabilities::all()
            .iter()
            .filter(|capability| {
                lemonfiber_contract::spoken(capability.name, capability.major) == spoken
            })
            .flat_map(lemonfiber_contract::conformance::cases)
            .any(|one| one.case == case)
}

/// Whether `host` is a DNS name of at least two labels, each of letters, digits and `-`
/// that neither begins nor ends with `-`.
fn dns_name(host: &str) -> bool {
    let (most, label_most) = DNS_MOST;
    let labels: Vec<&str> = host.split('.').collect();
    host.len() <= most
        && labels.len() >= 2
        && labels.iter().all(|label| {
            (1..=label_most).contains(&label.len())
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
}

/// Whether another of `services` fronts `service`.
fn fronted(service: &Service, services: &[Service]) -> bool {
    services
        .iter()
        .any(|adapter| adapter.id != service.id && adapter.fronts.as_deref() == Some(&service.id))
}

#[cfg(test)]
mod tests;
