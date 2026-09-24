//! The identities the bundled rows of the diagnostics register already hold.
//!
//! Its own file rather than a list at the top of the module above, because that list
//! is read by something rather than merely written down — the guard below reads the
//! modules that emit findings and holds the two to each other — and the pair belongs
//! together.

/// Every identity a bundled check reports a finding against.
///
/// Published, because a plugin contributing a row to this register has to be able to
/// be refused for colliding with one — and naming what was collided with takes knowing
/// it. Two rules keep a collision from happening at all: a contributed identity is
/// namespaced with the declaring plugin's id, and none of these carries a colon. The
/// second rule exists because the first is a property of two naming conventions
/// staying disjoint, which is a rule with an undefended edge.
///
/// **A name ending in a dot is a family, and everything under it is taken.** Four of
/// these are: an account, an indexer, a curating service and a credential each get a
/// finding of their own, and how many there are is the operator's configuration rather
/// than this build's. The rest are whole names.
///
/// Held to the checks themselves by the guard below, which reads the modules that emit
/// them: a list nobody compares against the code is a list that goes stale in the one
/// direction that matters, leaving a renamed check's old name free for a contribution
/// to take.
pub const BUNDLED_CHECKS: &[&str] = &[
    "config.credential-permissions",
    "config.download-client",
    "config.household-telling",
    "credentials.",
    "credentials.indexer",
    "environment.api",
    "environment.autostart",
    "environment.compose",
    "environment.engine",
    "network.bindings",
    "providers.indexer.",
    "providers.indexers",
    "providers.usenet",
    "providers.usenet.",
    "services.quality-guides",
    "services.releases",
    "services.releases.",
    "storage.hardlinks",
    "storage.mode",
    "storage.permissions",
    "storage.quality-headroom",
    "storage.single-mount",
    "storage.space",
    "vpn.egress-match",
    "vpn.egress-sources",
    "vpn.killswitch",
    "vpn.port-forward",
    "vpn.port-forward-client",
    "vpn.tunnel",
    "vpn.tunnel-restored",
    "vpn.unprotected",
];

#[cfg(test)]
mod tests;
