//! A plugin whose service stands in for the stack's request gate over its contract.

/// The plugin [`a_contracted_gate`] is installed as.
pub(crate) const GATE_PLUGIN: &str = "gater";

/// [`GATE_PLUGIN`], whose service stands in for the stack's request gate over
/// `request.gate`.
pub(crate) fn a_contracted_gate() -> crate::plugin::Installed {
    let mut installed = super::contracted(
        GATE_PLUGIN,
        crate::app::gating::SERVICE,
        lemonfiber_contract::capabilities::request::gate::CAPABILITY,
    );
    for placed in &mut installed.services {
        placed.replaces = Some(crate::app::gating::SERVICE.to_owned());
    }
    installed
}

/// [`GATE_PLUGIN`] as first-party.
pub(crate) const GATE_TRUSTED: [crate::plugin::first_party::FirstParty; 1] =
    super::first_party(GATE_PLUGIN);

/// Each way [`a_contracted_gate`] cannot be asked over its contract, named: untrusted, or
/// first-party and holding no key — with what is first-party, and whether it is keyed.
pub(crate) fn unaskable_gates() -> [(
    &'static str,
    &'static [crate::plugin::first_party::FirstParty],
    bool,
); 2] {
    [("untrusted", &[], true), ("unkeyed", &GATE_TRUSTED, false)]
}
