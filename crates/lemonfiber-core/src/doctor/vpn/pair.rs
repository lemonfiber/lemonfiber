//! Which two containers this check is about, and whether there are two.
//!
//! Resolved from the manifest by capability and dependency rather than by name: a
//! stack whose tunnel is called something other than gluetun is read the same way,
//! where a check that matched on names would quietly not apply to it while
//! reporting that everything is fine.
//!
//! The interesting answer is the one that resolves nothing, because it is two
//! situations wearing the same shape — a client with nothing containing it, and no
//! client at all. Telling them apart is the difference between a warning worth
//! reading and a sentence about a risk this stack cannot run.

use lemonfiber_manifest::{Manifest, Protocol};

/// The capability a VPN gateway needs to route the traffic of the containers
/// sharing its network, and so the mark by which one is recognised — the unit is
/// capability, not the container's name.
const GATEWAY_CAPABILITY: &str = "NET_ADMIN";

/// The two containers the check compares: the tunnel and the client contained
/// by it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Pair {
    /// The service that provides the tunnel.
    pub gateway: String,
    /// The download client whose traffic must traverse it.
    pub client: String,
}

/// The tunnel gateway and the download client contained by it, recognised by
/// capability rather than name so support is not tied to one provider or one
/// image.
///
/// The gateway is the torrent-profile service holding the routing capability;
/// the client is the torrent-profile service that depends on it.
/// Whether this stack declares something that downloads torrents.
///
/// Asked only where no pair resolved, to tell "nothing contains the client" from
/// "there is no client". The gateway itself is excluded by its capability rather
/// than by name, the same way the pair is resolved, so a stack that names its
/// tunnel something else is read the same way.
pub(super) fn torrent_client(manifest: &Manifest) -> bool {
    let torrent: Vec<&str> = manifest
        .profiles
        .iter()
        .filter(|profile| profile.protocol == Some(Protocol::Torrent))
        .map(|profile| profile.id.as_str())
        .collect();
    manifest.services.iter().any(|service| {
        torrent.contains(&service.profile.as_str())
            && !service
                .capabilities
                .iter()
                .any(|capability| capability == GATEWAY_CAPABILITY)
    })
}

pub(crate) fn resolve_pair(manifest: &Manifest) -> Option<Pair> {
    let torrent: Vec<&str> = manifest
        .profiles
        .iter()
        .filter(|profile| profile.protocol == Some(Protocol::Torrent))
        .map(|profile| profile.id.as_str())
        .collect();

    let gateway = manifest.services.iter().find(|service| {
        torrent.contains(&service.profile.as_str())
            && service
                .capabilities
                .iter()
                .any(|capability| capability == GATEWAY_CAPABILITY)
    })?;

    let client = manifest.services.iter().find(|service| {
        torrent.contains(&service.profile.as_str())
            && service.depends_on.iter().any(|on| on == &gateway.id)
    })?;

    Some(Pair {
        gateway: gateway.id.clone(),
        client: client.id.clone(),
    })
}
