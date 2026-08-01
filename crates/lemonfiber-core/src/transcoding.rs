//! Saying, before the choice, what a household's playback can actually do.
//!
//! The deepest quality preset asks for 4K HDR, which a media server must
//! transcode for any device that cannot direct-play it. Whether that transcode
//! is smooth or pins every CPU core is a property of the platform and how
//! Jellyfin runs, not of the preset — and the operator cannot see it from the
//! choice itself. lemonfiber can: it knows the environment and whether Jellyfin
//! runs in its container or on the host, so it states the consequence for
//! household playback before the choice is confirmed rather than after the
//! complaints.
//!
//! This module is pure: it maps facts to a caution. Whether and how that caution
//! is shown, and the confirmation it precedes, belong to the surface.

use crate::platform::Environment;
use crate::quality::Preset;
use crate::wizard::Library;

/// What the media server this stack runs can do with content a client cannot
/// direct-play.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Playback {
    /// No media server runs here, so lemonfiber transcodes nothing and the
    /// question does not arise — an external player is the operator's own concern.
    NoServer,
    /// The server reaches a hardware encoder, so even 4K HDR plays smoothly on a
    /// device that has to transcode.
    HardwareTranscoding,
    /// The server can only transcode on the CPU — which on a Docker virtual
    /// machine means 4K HDR will not play well.
    SoftwareOnly,
}

impl Playback {
    /// What this environment and Jellyfin mode amount to: Linux reaches the
    /// encoder from inside the container; macOS and Windows reach it only when
    /// Jellyfin runs natively; without a media server there is nothing to reach.
    ///
    /// This is the one place transcoding bridges the platform and the operator's
    /// media-server choice; everything downstream takes the [`Playback`] it
    /// returns and stays clear of both. The judgment is at the granularity of the
    /// platform, not the silicon: a Linux host is taken to reach its encoder,
    /// which a headless server with no GPU does not — narrowing that to detected
    /// hardware is left to a later check.
    ///
    /// Native mode maps to hardware transcoding on its own because it is only ever
    /// a valid choice where the platform offers it — macOS and Windows — so the
    /// pair that would misread it (an unsupported platform running native) does not
    /// arise through the wizard.
    #[must_use]
    pub const fn of(environment: Environment, library: Library) -> Self {
        match library {
            Library::None => Self::NoServer,
            Library::JellyfinNative => Self::HardwareTranscoding,
            Library::JellyfinDocker if environment.can_transcode_in_docker() => {
                Self::HardwareTranscoding
            }
            Library::JellyfinDocker => Self::SoftwareOnly,
        }
    }
}

/// The caution to state before an operator confirms a quality preset, when this
/// host would have to transcode it on the CPU.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Warning {
    /// The preset whose 4K HDR this host cannot hardware-transcode, so a surface
    /// can name it and draw on its [`Preset::consequence`] for the detail.
    pub preset: Preset,
}

/// The caution to state before confirming `preset` for a media server whose
/// [`Playback`] is known, or `None` where playback can take it in stride.
///
/// A warning is warranted only for a preset that commonly forces transcoding on a
/// server that can transcode in software alone. One that reaches a hardware
/// encoder plays it smoothly, a lighter preset does not provoke it, and no server
/// transcodes nothing — none of which is worth a caution. Taking the playback
/// rather than the platform keeps this a pure question about a preset, and lets a
/// surface resolve the capability once and weigh every preset in a selection
/// against it.
#[must_use]
pub fn warn_before_confirming(preset: Preset, playback: Playback) -> Option<Warning> {
    let software_only = playback == Playback::SoftwareOnly;
    (preset.likely_needs_transcoding() && software_only).then_some(Warning { preset })
}

#[cfg(test)]
mod tests {
    use super::{warn_before_confirming, Playback, Warning};
    use crate::platform::Environment;
    use crate::quality::Preset;
    use crate::wizard::Library;

    #[test]
    fn a_docker_media_server_transcodes_in_hardware_only_where_the_platform_can() {
        // Linux reaches the encoder from the container; macOS and Windows do not.
        assert_eq!(
            Playback::of(Environment::LinuxNative, Library::JellyfinDocker),
            Playback::HardwareTranscoding
        );
        assert_eq!(
            Playback::of(Environment::LinuxDesktop, Library::JellyfinDocker),
            Playback::HardwareTranscoding
        );
        assert_eq!(
            Playback::of(Environment::MacOs, Library::JellyfinDocker),
            Playback::SoftwareOnly
        );
        assert_eq!(
            Playback::of(Environment::Windows, Library::JellyfinDocker),
            Playback::SoftwareOnly
        );
    }

    #[test]
    fn native_jellyfin_reaches_the_encoder_wherever_it_runs() {
        // Native mode exists to reach the host encoder the Docker VM cannot.
        for environment in [Environment::MacOs, Environment::Windows] {
            assert_eq!(
                Playback::of(environment, Library::JellyfinNative),
                Playback::HardwareTranscoding
            );
        }
    }

    #[test]
    fn no_media_server_means_there_is_nothing_to_transcode() {
        for environment in [
            Environment::MacOs,
            Environment::Windows,
            Environment::LinuxNative,
            Environment::LinuxDesktop,
            Environment::Unsupported,
        ] {
            assert_eq!(Playback::of(environment, Library::None), Playback::NoServer);
        }
    }

    #[test]
    fn maximum_on_a_software_only_host_is_warned_before_confirmation() {
        let playback = Playback::of(Environment::MacOs, Library::JellyfinDocker);
        assert_eq!(
            warn_before_confirming(Preset::Maximum, playback),
            Some(Warning {
                preset: Preset::Maximum
            })
        );
    }

    #[test]
    fn a_host_that_hardware_transcodes_needs_no_warning() {
        // Linux Docker, and native mode on macOS, both reach the encoder.
        for playback in [
            Playback::of(Environment::LinuxNative, Library::JellyfinDocker),
            Playback::of(Environment::MacOs, Library::JellyfinNative),
        ] {
            assert!(warn_before_confirming(Preset::Maximum, playback).is_none());
        }
    }

    #[test]
    fn a_preset_that_does_not_force_transcoding_is_never_warned() {
        let software_only = Playback::of(Environment::MacOs, Library::JellyfinDocker);
        for preset in [Preset::SpaceSaving, Preset::Balanced, Preset::HighQuality] {
            assert!(
                warn_before_confirming(preset, software_only).is_none(),
                "{preset:?} does not force transcoding and needs no warning",
            );
        }
    }

    #[test]
    fn without_a_media_server_even_maximum_is_not_warned() {
        let playback = Playback::of(Environment::MacOs, Library::None);
        assert!(warn_before_confirming(Preset::Maximum, playback).is_none());
    }
}
