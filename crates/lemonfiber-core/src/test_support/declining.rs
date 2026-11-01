//! A plugin whose service stands in for the stack's decline service over its contract.

/// The plugin [`a_contracted_decline`] is installed as.
pub(crate) const DECLINE_PLUGIN: &str = "decliner";

/// [`DECLINE_PLUGIN`], whose service stands in for the stack's decline service over
/// `invite.decline`.
pub(crate) fn a_contracted_decline() -> crate::plugin::Installed {
    let mut installed = super::contracted(
        DECLINE_PLUGIN,
        crate::app::invite::declining::SERVICE,
        lemonfiber_contract::capabilities::invite::decline::CAPABILITY,
    );
    for placed in &mut installed.services {
        placed.replaces = Some(crate::app::invite::declining::SERVICE.to_owned());
    }
    installed
}

/// [`DECLINE_PLUGIN`] as first-party.
pub(crate) const DECLINE_TRUSTED: [crate::plugin::first_party::FirstParty; 1] =
    super::first_party(DECLINE_PLUGIN);
