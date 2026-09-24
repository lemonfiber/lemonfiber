//! Audio quality in plain language, for media that has no resolution.
//!
//! A film or a series has a resolution — 1080p, 4K — and [`quality::Preset`](crate::quality)
//! is the operator's answer to *how good should this look*. Music has no resolution;
//! its quality axis is the audio format itself: a small lossy file, a lossless copy
//! of the CD, or a hi-res studio master. So the same friendly question — *how good,
//! and how much disk* — needs a second, format-shaped answer here rather than a
//! resolution one bent to fit.
//!
//! This is that answer as three formats, each stating in plain terms what it means
//! and what it costs. Carrying it out is a separate concern: unlike the resolution
//! presets there is no community profile to lean on (Recyclarr configures only
//! Sonarr and Radarr), so the format maps to Lidarr's own quality profile, applied
//! through its API. Nothing here reaches a service or a disk; it is the pure model
//! that surface and the Lidarr writer are built on.

pub use lemonfiber_ports::media::Format;

#[cfg(test)]
mod tests;
