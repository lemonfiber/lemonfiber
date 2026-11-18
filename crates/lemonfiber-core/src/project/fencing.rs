//! The fence written beside each fenced service, and each fenced service held back where
//! none can be: its own profile, which no start activates, and no network at all.

use std::collections::BTreeMap;

use lemonfiber_manifest::container::{Constant, Posture, UserMode, Value};

use super::entry::{literal, Unit};
use super::inputs::Inputs;
use super::{Reading, UNFENCED};
use crate::fence::{Rules, Unfenced};
use crate::topology::{Placed, Topology};

/// What the project writes for its fenced services.
pub(super) struct Fencing<'a> {
    /// Each fenced service no fence can be written for, and why.
    unfenced: BTreeMap<String, Unfenced>,
    /// Each fence written.
    fences: Vec<Fence>,
    /// The image a fence runs, where one is pinned.
    image: Option<&'a str>,
}

/// One fence written beside its service.
struct Fence {
    /// The service it holds.
    service: String,
    /// Its id.
    id: String,
    /// The rules it holds its service to.
    rules: Rules,
    /// Its environment: the host's addresses it refuses, and the networks it passes where
    /// it holds its service inside the stack.
    handed: BTreeMap<String, Value>,
}

impl<'a> Fencing<'a> {
    /// What `inputs`' fenced services come to, placed as `topology` places them, as
    /// `reading` answers what a fence needs.
    pub(super) fn of(inputs: &Inputs, topology: &Topology, reading: &Reading<'a>) -> Self {
        let mut unfenced = BTreeMap::new();
        let mut fences = Vec::new();
        for fenced in &inputs.fenced {
            let networks = fenced
                .fence
                .as_deref()
                .and_then(|fence| answered(fence, topology, reading));
            let why = if fenced.fence.is_none() {
                Some(Unfenced::Taken)
            } else if reading.fence.is_none() {
                Some(Unfenced::Unpinned)
            } else if !reading.nftables {
                Some(Unfenced::Unheld)
            } else if reading.host.is_empty() {
                Some(Unfenced::Unaddressed)
            } else if fenced.rules == Rules::Inward && networks.is_none() {
                Some(Unfenced::Unread)
            } else {
                None
            };
            if let (None, Some(id)) = (&why, &fenced.fence) {
                fences.push(Fence {
                    service: fenced.service.clone(),
                    id: id.clone(),
                    rules: fenced.rules,
                    handed: handed(
                        fenced.rules,
                        reading,
                        networks.as_deref(),
                        &named(id, topology),
                    ),
                });
            } else {
                unfenced.insert(fenced.service.clone(), why.unwrap_or(Unfenced::Taken));
            }
        }
        Self {
            unfenced,
            fences,
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
            .filter_map(|fence| {
                let profile = units
                    .iter()
                    .find(|unit| unit.id == fence.service)?
                    .profile
                    .clone();
                Some(fence.unit(profile, self.image?))
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
}

impl Fence {
    /// The fence, running `image` in `profile` with what it is handed, confined but for the
    /// one capability it alone is given.
    fn unit(&self, profile: String, image: &str) -> Unit<'_> {
        let inward = self.rules == Rules::Inward;
        Unit {
            id: &self.id,
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
            command: if inward {
                crate::fence::STACK.map(str::to_owned).to_vec()
            } else {
                Vec::new()
            },
            dns_search: if inward { &crate::fence::SEARCHED } else { &[] },
            hostname: None,
            granted: &crate::fence::GRANTED,
            reaches_host: false,
            schedule: None,
        }
    }
}

/// Each network the fence `fence` joins as `topology` places it, as `reading` answers
/// it, or nothing where the engine answered any of them unreadably or not at all.
fn answered<'r>(
    fence: &str,
    topology: &Topology,
    reading: &Reading<'r>,
) -> Option<Vec<&'r crate::fence::Answered>> {
    topology
        .placed
        .get(fence)
        .and_then(Placed::on)?
        .keys()
        .map(|network| reading.networks.get(network))
        .collect::<Option<Vec<_>>>()
        .filter(|networks| !networks.is_empty())
}

/// The name of each service the fence `fence` shares a network with as `topology` places
/// them, and of each service running inside one of theirs.
fn named(fence: &str, topology: &Topology) -> String {
    let joined = topology
        .placed
        .get(fence)
        .and_then(Placed::on)
        .cloned()
        .unwrap_or_default();
    let beside: std::collections::BTreeSet<&str> = topology
        .placed
        .iter()
        .filter(|(id, placed)| {
            *id != fence
                && placed
                    .on()
                    .is_some_and(|on| on.keys().any(|network| joined.contains_key(network)))
        })
        .map(|(id, _)| id.as_str())
        .collect();
    let inside = topology
        .placed
        .iter()
        .filter_map(|(id, placed)| match placed {
            Placed::Inside(host) if beside.contains(host.as_str()) => Some(id.as_str()),
            _ => None,
        });
    beside
        .iter()
        .copied()
        .chain(inside)
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join(",")
}

/// The environment a fence holding to `rules` is handed: the host's addresses, and the
/// subnet and gateway of each of `networks` and `names` where it holds its service inside
/// the stack.
fn handed(
    rules: Rules,
    reading: &Reading<'_>,
    networks: Option<&[&crate::fence::Answered]>,
    names: &str,
) -> BTreeMap<String, Value> {
    let mut handed = BTreeMap::from([(
        crate::fence::HOST_ADDRESSES.to_owned(),
        Value::Constant(Constant::Text(crate::fence::addresses(reading.host))),
    )]);
    if let (Rules::Inward, Some(networks)) = (rules, networks) {
        handed.insert(
            crate::fence::NETWORKS.to_owned(),
            Value::Constant(Constant::Text(crate::fence::networks(
                networks.iter().copied(),
            ))),
        );
        handed.insert(
            crate::fence::NAMES.to_owned(),
            Value::Constant(Constant::Text(names.to_owned())),
        );
    }
    handed
}
