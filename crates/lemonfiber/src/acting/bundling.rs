//! What a support bundle is to hold, decided before one is described.
//!
//! `bundle::Wanted` is four answers: how much of each service's log to take, what
//! becomes of media filenames, which settings are shown as they are, and the
//! agreement showing one takes. This screen carried none of them. It sent the careful
//! defaults, which is the right thing to send when nobody has been asked — and being
//! the right default is not the same as being the only answer available, which is what
//! a surface that never asks makes it.
//!
//! Three of the four are asked now. The window is typed, because a number has no list
//! to be taken off. What becomes of filenames is taken off a list, because the command
//! carries an enum of exactly two values and both of them are already here — which is
//! the rule the narrowing next door follows and the reason nothing is fetched to build
//! this list.
//!
//! **The rows are not put to the translation first.** Every other list on this screen
//! offers each row to [`lemonfiber_api::actions::named`] and keeps what comes back,
//! because those rows are names a stack supplied and the table is the only thing that
//! knows which of them an action can carry. These two are not names; they are the two
//! values of an argument the command declares, so there is no third that might be
//! refused. What the table does still decide is the request itself: taking a row goes
//! to the run that says what a bundle would hold, and that run goes through the table
//! like every other.
//!
//! **The fourth is not asked, and that is an exception rather than a gap.** Which
//! settings are shown as they are stays on the browser. A way past the withholding
//! list here would be a capability no other surface has, on the surface least likely
//! to be sitting behind a login — so the reveal is not offered, and the guard beside
//! this list is what keeps it that way.

use lemonfiber_core::bundle::Filenames;

use super::chooser::{Chooser, Listed};
use super::errand::{self, Errand, Given};
use super::{Press, Stage, Wanted};

/// One answer to what a bundle does with media filenames.
pub(super) struct Held {
    /// What it is called on the row.
    name: &'static str,
    /// What choosing it comes to, in the line beside the name.
    about: &'static str,
    /// What the errand is given by taking it.
    given: Given,
}

impl Listed for Held {
    fn name(&self) -> &str {
        self.name
    }

    fn about(&self) -> &str {
        self.about
    }
}

/// The list of what a bundle does with media filenames, over the window it was given.
///
/// Replaced first, because it is the careful answer and the one an operator who
/// presses enter twice should land on. What replacing costs is said on the row beside
/// it: a diagnostic naming the same file twice still names it recognisably, which is
/// the whole reason the marks exist rather than a blank.
pub(super) fn over(errand: &'static Errand, lines: u32) -> Stage {
    Stage::Bundling {
        errand,
        chooser: Chooser::over(
            Held {
                name: "media filenames replaced",
                about: "each one stood in for by a mark, so two mentions of a file still match",
                given: Given::bundled(lines, Filenames::Replaced),
            },
            vec![Held {
                name: "media filenames shown as they are",
                about: "what is in the library goes into a file people attach to a post",
                given: Given::bundled(lines, Filenames::Shown),
            }],
        ),
    }
}

/// Over what a bundle does with media filenames: move, take one, or leave it.
pub(super) fn bundling(
    stage: &mut Stage,
    errand: &'static Errand,
    mut chooser: Chooser<Held>,
    press: &Press,
) -> Wanted {
    match *press {
        Press::Abandon => return Wanted::Nothing,
        Press::Accept => return errand::begun(stage, errand, chooser.taken().given),
        Press::Back => chooser.back(),
        Press::Forward => chooser.forward(),
        Press::Typed(_) | Press::Rubout => (),
    }
    *stage = Stage::Bundling { errand, chooser };
    Wanted::Nothing
}
