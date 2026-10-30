//! The fence written beside each fenced service, and each fenced service held back where
//! none can be: its own profile, which no start activates, and no network at all.

use std::collections::BTreeMap;

use lemonfiber_manifest::container::{Constant, Posture, UserMode, Value};

use super::entry::{literal, Unit};
use super::inputs::Inputs;
use super::{Reading, UNFENCED};
use crate::fence::Unfenced;
use crate::topology::{Placed, Topology};

/// What the project writes for its fenced services.
pub(super) struct Fencing<'a> {
    /// Each fenced service no fence can be written for, and why.
    unfenced: BTreeMap<String, Unfenced>,
    /// Each fenced service a fence is written for, and the fence's id.
    fences: Vec<(String, String)>,
    /// The fence's environment: the host's addresses it refuses.
    handed: BTreeMap<String, Value>,
    /// The image a fence runs, where one is pinned.
    image: Option<&'a str>,
}

impl<'a> Fencing<'a> {
    /// What `inputs`' fenced services come to, as `reading` answers what a fence needs.
    pub(super) fn of(inputs: &Inputs, reading: &Reading<'a>) -> Self {
        let unfenced: BTreeMap<String, Unfenced> = inputs
            .fenced
            .iter()
            .filter_map(|(service, fence)| {
                let why = if fence.is_none() {
                    Unfenced::Taken
                } else if reading.fence.is_none() {
                    Unfenced::Unpinned
                } else if reading.host.is_empty() {
                    Unfenced::Unaddressed
                } else {
                    return None;
                };
                Some((service.clone(), why))
            })
            .collect();
        let fences = inputs
            .fenced
            .iter()
            .filter(|(service, _)| !unfenced.contains_key(service))
            .filter_map(|(service, fence)| Some((service.clone(), fence.clone()?)))
            .collect();
        Self {
            unfenced,
            fences,
            handed: BTreeMap::from([(
                crate::fence::HOST_ADDRESSES.to_owned(),
                Value::Constant(Constant::Text(crate::fence::addresses(reading.host))),
            )]),
            image: reading.fence,
        }
    }

    /// `unit`, written under [`UNFENCED`] where it is a fenced service no fence can be
    /// written for.
    pub(super) fn held_back<'u>(&self, mut unit: Unit<'u>) -> Unit<'u> {
        if self.unfenced.contains_key(unit.id) {
            UNFENCED.clone_into(&mut unit.profile);
        }
        unit
    }

    /// A fence for each fenced service of `units` one is written for, in that service's
    /// profile, so the two start together.
    pub(super) fn units(&self, units: &[Unit<'_>]) -> Vec<Unit<'_>> {
        self.fences
            .iter()
            .filter_map(|(service, fence)| {
                let profile = units
                    .iter()
                    .find(|unit| unit.id == service)?
                    .profile
                    .clone();
                Some(self.fence(fence, profile, self.image?))
            })
            .collect()
    }

    /// Where `id` is placed: nowhere where it is held back unfenced, and where `topology`
    /// places it otherwise.
    pub(super) fn placed(&self, id: &str, topology: &Topology) -> Placed {
        if self.unfenced.contains_key(id) {
            return Placed::Nowhere;
        }
        topology.placed.get(id).cloned().unwrap_or(Placed::Nowhere)
    }

    /// Each fenced service held back, and why.
    pub(super) fn unfenced(&self) -> Vec<(String, Unfenced)> {
        self.unfenced
            .iter()
            .map(|(service, why)| (service.clone(), *why))
            .collect()
    }

    /// The fence `id`, running `image` in `profile` with the host's addresses handed it,
    /// confined but for the one capability it alone is given.
    fn fence<'u>(&'u self, id: &'u str, profile: String, image: &str) -> Unit<'u> {
        Unit {
            id,
            namespace: id,
            profile,
            image: literal(image),
            published: Vec::new(),
            contract: None,
            takes_data: false,
            config_path: None,
            config_ro: true,
            mounts: &[],
            shares: None,
            tmpfs: &[],
            env: &self.handed,
            user: UserMode::Image,
            posture: Posture::Confined,
            mem_limit_mib: Some(crate::fence::MEMORY_MIB),
            shape: None,
            secrets_file: None,
            command: Vec::new(),
            granted: &crate::fence::GRANTED,
        }
    }
}
