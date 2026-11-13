//! Emby as the media server: the media server moved into its plugin as the Emby plugin's
//! server declares itself, `emby` by id, pinned at an Emby version and answering in Emby's
//! own API, with the request gate asking the identity source in Emby's API among others.

/// The Emby plugin's server's id, which names the request gate's route to it.
pub(crate) const SERVER: &str = "emby";

/// The Emby version the Emby plugin pins its server at.
pub(crate) const PINNED: &str = "4.10.2.0";

/// The media server moved into its plugin as Emby: its upstream `emby`, standing in for no
/// service of the stack's, pinned at [`PINNED`] and answering in Emby's API, and its
/// adapter fronting it.
pub(crate) fn embied() -> Option<crate::plugin::Installed> {
    super::media::moved().map(|mut installed| {
        for placed in &mut installed.services {
            if placed.native.is_some() {
                SERVER.clone_into(&mut placed.service);
                "Emby".clone_into(&mut placed.name);
                PINNED.clone_into(&mut placed.tag);
                placed.native = Some(crate::app::gating::EMBY_API.to_owned());
                placed.replaces = None;
            }
            if placed.fronts.is_some() {
                placed.fronts = Some(SERVER.to_owned());
            }
        }
        installed
    })
}

/// The request gate asking the identity source in Jellyfin's, Emby's or Plex's API, as its
/// manifest does.
pub(crate) fn emby_gate() -> crate::plugin::Installed {
    super::plexing::asking_in(
        super::a_contracted_gate(),
        &[
            lemonfiber_manifest::ApiKind::Jellyfin.name(),
            crate::app::gating::EMBY_API,
            crate::plex_link::API,
        ],
    )
}
