//! The acts about the quality choice.
//!
//! Apart from the table that names them for the reason the household's requests and the
//! line's are: three rows reading two fields between them, where a preset that is
//! missing is the only thing any of them can refuse — and the table they came out of is
//! read by every other action on the surface.

use lemonfiber_core::app::{Command, QualityAction};

use super::{Arguments, Refused};
use crate::actions::reading::quality;

/// Every act about the quality choice.
const ABOUT: [&str; 3] = ["quality-set", "quality-reapply", "quality-upgrade"];

/// Whether this action is about how good media should look.
pub(super) fn about_the_quality(action: &str) -> bool {
    ABOUT.contains(&action)
}

/// What the action asks the core for, or why it asks nothing.
///
/// # Errors
///
/// Where a preset is asked for and none was named.
pub(super) fn asked_for(action: &str, given: &Arguments) -> Result<Command, Refused> {
    match action {
        "quality-reapply" => Ok(Command::Quality(QualityAction::Reapply)),
        "quality-upgrade" => Ok(Command::QualityUpgrade {
            confirm: given.confirm,
        }),
        _ => match &given.preset {
            Some(preset) => quality(preset, given.media_type.clone(), given.confirm),
            None => Err(Refused::Missing {
                action: action.to_owned(),
                argument: "preset".to_owned(),
            }),
        },
    }
}
