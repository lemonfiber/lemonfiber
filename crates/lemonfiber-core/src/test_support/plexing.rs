//! Plex as the media server: the media server moved into its plugin, answering in Plex's
//! API and claimed over `owner.claim`, as the Plex plugin's services are.

use lemonfiber_contract::capabilities::owner::claim;

/// The media server moved into its plugin, its upstream answering in Plex's API and its
/// adapter speaking `owner.claim`.
pub(crate) fn plexed() -> Option<crate::plugin::Installed> {
    let claim = lemonfiber_contract::spoken(claim::CAPABILITY, claim::MAJOR);
    super::media::moved().map(|mut installed| {
        for placed in &mut installed.services {
            if placed.native.is_some() {
                placed.native = Some(crate::plex_link::API.to_owned());
            }
            if placed.fronts.is_some() {
                placed.speaks.push(claim.clone());
            }
        }
        installed
    })
}

/// `installed`, each of its asks of the identity source taking Jellyfin's API or Plex's, as
/// the request gate's does.
pub(crate) fn asking_in_plex_too(installed: crate::plugin::Installed) -> crate::plugin::Installed {
    asking_in(
        installed,
        &[
            lemonfiber_manifest::ApiKind::Jellyfin.name(),
            crate::plex_link::API,
        ],
    )
}

/// `installed`, each of its asks of the identity source taking any of `apis`.
pub(crate) fn asking_in(
    mut installed: crate::plugin::Installed,
    apis: &[&str],
) -> crate::plugin::Installed {
    for placed in &mut installed.services {
        for ask in placed
            .asks
            .iter_mut()
            .filter(|ask| ask.capability == crate::app::targets::IDENTITY)
        {
            ask.native = serde_json::from_value(serde_json::json!(apis)).ok();
        }
    }
    installed
}
