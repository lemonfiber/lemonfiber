//! What the credentials of the stack's own services, and of the parts moved in place of
//! them, are named.

use super::{Filler, Fillers, Origin};
use crate::config::credentials::{Naming, Ordinals};

impl Fillers {
    /// The name a credential `filler` holds would be kept under, ending in `holds`,
    /// before anything is refused.
    pub(super) fn spelled(&self, filler: &Filler, holds: &str) -> Option<String> {
        match &filler.origin {
            Origin::Plugin { named } if !keeps_the_stacks_names(filler) => {
                Some(crate::config::for_plugin(named, &filler.id, holds))
            }
            _ => {
                let namings: Vec<Naming<'_>> = self
                    .services
                    .iter()
                    .filter(|one| keeps_the_stacks_names(one))
                    .map(|one| self.naming(one))
                    .collect();
                crate::config::credentials::named(
                    &namings,
                    &self.ordinals,
                    self.naming(filler),
                    holds,
                )
            }
        }
    }

    /// The name an earlier release recorded a credential `filler` holds under, ending in
    /// `holds`, where it named it after the service's id.
    #[must_use]
    pub(crate) fn earlier(filler: &Filler, holds: &str) -> Option<String> {
        keeps_the_stacks_names(filler).then(|| crate::config::for_service(&filler.id, holds))
    }

    /// Every service whose credential ending in `holds` shares its capability and
    /// qualifier with another's, with that capability and the ordinal it takes there.
    #[must_use]
    pub(crate) fn numbering(&self, holds: &str) -> Vec<(String, String, u32)> {
        let namings: Vec<Naming<'_>> = self
            .services
            .iter()
            .filter(|one| keeps_the_stacks_names(one))
            .map(|one| self.naming(one))
            .collect();
        let groups: Vec<Option<(String, Option<String>)>> = namings
            .iter()
            .map(|one| crate::config::credentials::reached(*one, holds))
            .collect();
        namings
            .iter()
            .zip(&groups)
            .filter_map(|(one, group)| {
                let group = group.as_ref()?;
                let shared = groups
                    .iter()
                    .filter(|other| other.as_ref() == Some(group))
                    .count()
                    > 1;
                shared.then(|| {
                    (
                        one.id.to_owned(),
                        group.0.clone(),
                        self.ordinals.taken(&namings, one.id, holds),
                    )
                })
            })
            .collect()
    }

    /// What naming `filler`'s credentials reads of it: what the stack's own service of
    /// its id declares, so a part moved in place of one keeps its names; otherwise what
    /// it declares, and where it declares nothing, what the adapter of its own plugin in
    /// front of it declares for it.
    pub(super) fn naming<'a>(&'a self, filler: &'a Filler) -> Naming<'a> {
        if let Some((provides, media_types)) = self.declared.get(&filler.id) {
            return Naming {
                id: &filler.id,
                provides,
                media_types,
            };
        }
        let adapter = self.fronting(filler);
        let declared = |own: &'a [String], fronted: fn(&'a Filler) -> &'a [String]| match adapter {
            Some(adapter) if own.is_empty() => fronted(adapter),
            _ => own,
        };
        Naming {
            id: &filler.id,
            provides: declared(&filler.provides, |one| &one.provides),
            media_types: declared(&filler.media_types, |one| &one.media_types),
        }
    }

    /// Every name two credentials here would both take, which neither is kept under.
    #[must_use]
    pub(crate) fn clashing(&self) -> Vec<String> {
        let namings: Vec<Naming<'_>> = self
            .services
            .iter()
            .filter(|one| keeps_the_stacks_names(one))
            .map(|one| self.naming(one))
            .collect();
        crate::config::credentials::clashing(&namings, &self.ordinals)
    }

    /// The ordinals recorded for these fillers.
    #[must_use]
    pub(crate) const fn ordinals(&self) -> &Ordinals {
        &self.ordinals
    }
}

/// Whether `filler`'s credentials are named as the stack names its own: a stack service,
/// or a first-party part moved in place of one.
pub(super) fn keeps_the_stacks_names(filler: &Filler) -> bool {
    !matches!(filler.origin, Origin::Plugin { .. }) || filler.stands_in()
}
