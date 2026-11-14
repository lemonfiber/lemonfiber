//! The actions about watching that carry a device, a position, an id, a live stream or a
//! pass.

/// The actions whose command carries the id a player keeps for the device it plays on.
///
/// A grant and a browser's play: the device is what the session each opens belongs to,
/// so asking again for the same one replaces that session rather than adding another.
pub const TAKES_DEVICE: &[&str] = &["grant", "play"];

/// The action whose command carries how far a member got through one title: the
/// position and whether they finished it, which are one report.
pub const TAKES_PROGRESS: &[&str] = &["watched"];

/// The actions whose command carries one thing by its id: a title, a channel, or a wake a
/// phone confirms.
pub const TAKES_TITLE: &[&str] = &["watched", "ask", "push-woken", "tune", "play"];

/// The action whose command carries a live stream the core opened, to renew or to close.
pub const TAKES_STREAM: &[&str] = &["tune"];

/// The action whose command carries a pass a browser hands back for a new one.
pub const TAKES_PASS: &[&str] = &["play"];
