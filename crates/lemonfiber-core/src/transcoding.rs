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
mod tests;
