//! What the quality and music surfaces answer with.
//!
//! One of the report families the machine-readable contract is made of; they live in
//! separate files and are re-exported as one, so `crate::model::X` reads the same as it
//! always did.

use serde::Serialize;

/// One preset in force, and what it means for the media it applies to — the
/// operator's question answered in their own terms, with no scoring vocabulary.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct PresetChoice {
    /// What this applies to: `everything`, or a specific media type.
    pub scope: String,
    /// The preset's plain-language name.
    pub preset: String,
    /// What it means, in the operator's terms rather than the tool's.
    pub means: String,
    /// The resolution and encode it targets.
    pub resolution: String,
    /// Roughly how much disk an hour of it takes.
    pub size_per_hour: String,
    /// What playback costs, in plain terms.
    pub transcoding: String,
    /// Whether this host would have to transcode it in software — the caution
    /// stated before a choice a household cannot smoothly play.
    pub needs_transcoding_here: bool,
}

/// One audio-format choice in force, for media that has no resolution — the same
/// question as a [`PresetChoice`], answered in format terms rather than resolution.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct MusicChoice {
    /// What this applies to — `music`.
    pub scope: String,
    /// The format's plain-language name.
    pub format: String,
    /// What it means, in the operator's terms.
    pub means: String,
    /// The audio format it targets, in plain terms.
    pub targets: String,
    /// Roughly how much disk an hour of it takes.
    pub size_per_hour: String,
    /// The practical caveat worth knowing — playing it, or finding it.
    pub note: String,
}

/// The operator's quality choice, what each preset means, and what the command
/// did with it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct QualityReport {
    /// The global choice first, then each media type set apart from it.
    pub choices: Vec<PresetChoice>,
    /// The audio-format choice for music, where one is set — media that has no
    /// resolution, so it is reported apart from the resolution presets rather than
    /// forced into their shape.
    pub music: Option<MusicChoice>,
    /// Whether the Recyclarr config has been hand-edited since lemonfiber wrote it —
    /// the `customised` state, in which the preset is no longer authoritative until
    /// it is deliberately re-asserted. For a reapply, whether an edit was overwritten.
    pub customised: bool,
    /// What became of the choice.
    pub disposition: Disposition,
}

/// What choosing an audio format for music did: the choice, whether it was recorded
/// or only rehearsed, and — once recorded — what became of applying it to the music
/// service.
///
/// Music has no resolution and no community profile to lean on, so unlike a resolution
/// preset the choice is carried straight to the service through its API. The choice is
/// still recorded first, so it is remembered even when the service cannot be reached.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct MusicReport {
    /// The format chosen, what it means, and what it costs.
    pub choice: MusicChoice,
    /// Whether the choice was recorded, or only rehearsed.
    pub disposition: Disposition,
    /// What became of applying it to the music service, or `None` for a rehearsal
    /// that applied nothing.
    pub outcome: Option<Triggered>,
}
