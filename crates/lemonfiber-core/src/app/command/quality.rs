//! What one run was asked about how good the media should look.
//!
//! Its own file beside the other command actions. Showing states what each preset
//! means and what it costs; the rest act, and each carries only what its own act
//! needs rather than a shared bag of fields most of them would leave empty.

use crate::quality::Preset;

/// What a quality command asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QualityAction {
    /// Show the choice in force and what each preset means and costs.
    Show,
    /// Choose a preset — for everything, or for one media type — and record it.
    Set {
        /// The preset to choose.
        preset: Preset,
        /// The media type it applies to, or the whole library where absent.
        media_type: Option<String>,
        /// Whether the operator confirmed a choice this host would have to
        /// transcode in software, which is otherwise held rather than recorded.
        confirm: bool,
    },
    /// Re-assert the recorded preset over a hand-edited Recyclarr config — the
    /// explicit consent to let the preset win where a run would preserve the edit.
    Reapply,
}
