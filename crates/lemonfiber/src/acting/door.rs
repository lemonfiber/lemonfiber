//! The front door's code, full size, in the box over the panels.
//!
//! The front-door panel draws the code wherever it fits. Where the terminal is too
//! small for it there, the panel names this key, which opens it in the box over the
//! panels, where it moves like any reading. The footer leaves it out: it is named
//! where it is needed.

use super::reading::{lines_of, moved, Reading};
use super::{Press, Stage, Wanted};

/// The key that opens the front door's code full size.
pub(crate) const KEY: char = 'o';

/// The box with the front door's code in it, or nothing where there is no address to
/// draw one from.
pub(super) fn opened(address: Option<&str>) -> Option<Stage> {
    address.map(|address| Stage::Door(Reading::of(lines_of(&crate::render::door::code(address)))))
}

/// A press over the code: up and down move it, and any other key puts it away.
pub(super) fn door(stage: &mut Stage, mut reading: Reading, press: &Press) -> Wanted {
    if moved(&mut reading, press) {
        *stage = Stage::Door(reading);
    }
    Wanted::Nothing
}
