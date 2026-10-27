//! What a credential the stack records is named: the capability it reaches, what tells
//! that capability's fillers apart, an ordinal where two services would share one name,
//! and what it holds.
//!
//! **A name answers for one credential or for none.** The capability is one the
//! vocabulary carries and the qualifier one of the media types, so neither is a word a
//! manifest made up; a service sharing its capability and qualifier with another is named
//! only once its ordinal is recorded; and a name two credentials of the stack would both
//! take is given to neither. So a reader and the writer, both asking here with the whole
//! stack in view, never disagree about whose credential a name holds.

use std::collections::BTreeMap;

use lemonfiber_contract::capabilities::identity::source;
use lemonfiber_contract::capabilities::library::curate;
use lemonfiber_contract::capabilities::media::serve;

use super::{ADMIN_PASSWORD_SUFFIX, CREDENTIAL_SUFFIXES};

/// The setting each recorded ordinal is kept under, as `<service>@<capability>=<n>`
/// pairs joined by `,`.
pub(crate) const ORDINALS_KEY: &str = "LEMONFIBER_CREDENTIAL_ORDINALS";

/// What naming a service's credentials reads of it.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Naming<'a> {
    /// The service's id.
    pub(crate) id: &'a str,
    /// The capabilities it provides, in the order its manifest lists them.
    pub(crate) provides: &'a [String],
    /// The media types it handles, in the order its manifest lists them.
    pub(crate) media_types: &'a [String],
}

/// The capability and qualifier a credential of `service` ending in `holds` is named
/// for, where the service provides a capability the vocabulary carries and the
/// qualifier, where there is one, is a media type.
#[must_use]
pub(crate) fn reached(service: Naming<'_>, holds: &str) -> Option<(String, Option<String>)> {
    let identity = service.provides.iter().any(|one| one == source::CAPABILITY);
    if holds == ADMIN_PASSWORD_SUFFIX && identity {
        return Some((source::CAPABILITY.to_owned(), None));
    }
    let capability = service.provides.first().filter(|one| carried(one))?;
    let qualifier = match capability.as_str() {
        curate::CAPABILITY => service.media_types.first(),
        serve::CAPABILITY if !identity => service.media_types.first(),
        _ => None,
    };
    if qualifier.is_some_and(|one| !lemonfiber_manifest::container::MEDIA.contains(&one.as_str())) {
        return None;
    }
    Some((capability.clone(), qualifier.cloned()))
}

/// Whether the vocabulary carries `capability`.
fn carried(capability: &str) -> bool {
    lemonfiber_plugin::vocabulary::carried()
        .iter()
        .any(|one| one.name == capability)
}

/// `<CAPABILITY>[_<QUALIFIER>][_<N>]<holds>`, in upper case with `_` for `.` and `-`.
#[must_use]
pub(crate) fn spelled(
    capability: &str,
    qualifier: Option<&str>,
    ordinal: Option<u32>,
    holds: &str,
) -> String {
    let upper = |text: &str| text.to_ascii_uppercase().replace(['.', '-'], "_");
    let mut name = upper(capability);
    if let Some(qualifier) = qualifier {
        name.push('_');
        name.push_str(&upper(qualifier));
    }
    if let Some(ordinal) = ordinal {
        name.push('_');
        name.push_str(&ordinal.to_string());
    }
    name.push_str(holds);
    name
}

/// The ordinal each service holds within a capability, as recorded.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Ordinals(BTreeMap<(String, String), u32>);

impl Ordinals {
    /// The ordinals a recorded value holds, every pair it cannot read left out.
    #[must_use]
    pub(crate) fn read(recorded: Option<&str>) -> Self {
        Self(
            recorded
                .unwrap_or_default()
                .split(',')
                .filter_map(|pair| {
                    let (named, ordinal) = pair.trim().split_once('=')?;
                    let (service, capability) = named.split_once('@')?;
                    let ordinal = ordinal.parse().ok().filter(|one| *one > 0)?;
                    Some(((service.to_owned(), capability.to_owned()), ordinal))
                })
                .collect(),
        )
    }

    /// The value the ordinals are recorded as.
    #[must_use]
    pub(crate) fn written(&self) -> String {
        self.0
            .iter()
            .map(|((service, capability), ordinal)| format!("{service}@{capability}={ordinal}"))
            .collect::<Vec<_>>()
            .join(",")
    }

    /// The ordinal `service` holds within `capability`, where one is recorded.
    #[must_use]
    pub(crate) fn of(&self, service: &str, capability: &str) -> Option<u32> {
        self.0
            .get(&(service.to_owned(), capability.to_owned()))
            .copied()
    }

    /// Record `ordinal` for `service` within `capability`.
    pub(crate) fn record(&mut self, service: &str, capability: &str, ordinal: u32) {
        self.0
            .insert((service.to_owned(), capability.to_owned()), ordinal);
    }

    /// The ordinal `service` is to be recorded with within the capability and qualifier
    /// its credential ending in `holds` is named for, among `services`: the one recorded,
    /// and otherwise the lowest no other service there holds or takes before it, in
    /// their order.
    #[must_use]
    pub(crate) fn taken(&self, services: &[Naming<'_>], service: &str, holds: &str) -> u32 {
        let Some(named) = services.iter().find(|one| one.id == service) else {
            return 1;
        };
        let Some(group) = reached(*named, holds) else {
            return 1;
        };
        let mut held: Vec<u32> = sharing(services, &group, holds)
            .iter()
            .filter_map(|one| self.of(one.id, &group.0))
            .collect();
        sharing(services, &group, holds)
            .iter()
            .map(|one| {
                let ordinal = self.of(one.id, &group.0).unwrap_or_else(|| {
                    let free = (1..)
                        .take(held.len() + 1)
                        .find(|n| !held.contains(n))
                        .unwrap_or(1);
                    held.push(free);
                    free
                });
                (one.id, ordinal)
            })
            .find(|(id, _)| *id == service)
            .map_or(1, |(_, ordinal)| ordinal)
    }
}

/// The services among `services` a credential ending in `holds` names for `group`.
fn sharing<'a>(
    services: &'a [Naming<'a>],
    group: &(String, Option<String>),
    holds: &str,
) -> Vec<&'a Naming<'a>> {
    services
        .iter()
        .filter(|one| reached(**one, holds).as_ref() == Some(group))
        .collect()
}

/// The name a credential of `service` ending in `holds` would take among `services`,
/// before another credential's is compared with it: none where it provides nothing the
/// vocabulary carries, or shares its capability and qualifier with another service and
/// has no ordinal recorded.
fn candidate(
    services: &[Naming<'_>],
    ordinals: &Ordinals,
    service: Naming<'_>,
    holds: &str,
) -> Option<String> {
    let group = reached(service, holds)?;
    let shared = sharing(services, &group, holds)
        .iter()
        .any(|one| one.id != service.id);
    let ordinal = match ordinals.of(service.id, &group.0) {
        Some(ordinal) => ordinal,
        None if shared => return None,
        None => 1,
    };
    Some(spelled(
        &group.0,
        group.1.as_deref(),
        Some(ordinal).filter(|n| *n > 1),
        holds,
    ))
}

/// The name a credential of `service` ending in `holds` is recorded under, among
/// `services`: none where [`candidate`] gives it none, or another credential of
/// `services` would take the same name.
#[must_use]
pub(crate) fn named(
    services: &[Naming<'_>],
    ordinals: &Ordinals,
    service: Naming<'_>,
    holds: &str,
) -> Option<String> {
    let name = candidate(services, ordinals, service, holds)?;
    (!others(services, ordinals, service.id, holds).any(|other| other == name)).then_some(name)
}

/// The names every credential of `services` but `service`'s ending in `holds` would take.
fn others<'a>(
    services: &'a [Naming<'a>],
    ordinals: &'a Ordinals,
    service: &'a str,
    holds: &'a str,
) -> impl Iterator<Item = String> + 'a {
    services
        .iter()
        .flat_map(|one| CREDENTIAL_SUFFIXES.iter().map(move |ending| (one, *ending)))
        .filter(move |(one, ending)| !(one.id == service && *ending == holds))
        .filter_map(move |(one, ending)| candidate(services, ordinals, *one, ending))
}

/// Every name two credentials of `services` would both take, which neither is kept
/// under.
#[must_use]
pub(crate) fn clashing(services: &[Naming<'_>], ordinals: &Ordinals) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    let mut clashed: Vec<String> = Vec::new();
    for name in services.iter().flat_map(|one| {
        CREDENTIAL_SUFFIXES
            .iter()
            .filter_map(move |ending| candidate(services, ordinals, *one, ending))
    }) {
        if seen.contains(&name) {
            if !clashed.contains(&name) {
                clashed.push(name);
            }
        } else {
            seen.push(name);
        }
    }
    clashed
}

#[cfg(test)]
mod tests;
