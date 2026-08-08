//! Ending setup with something working, rather than with an empty dashboard.
//!
//! Setup finishes at the moment of maximum uncertainty: sixteen services are running,
//! everything is green, and the operator has no idea what to do. They installed this
//! because they wanted to watch something, and what has been delivered is
//! infrastructure. The walkthrough closes that gap by adding one thing, end to end,
//! narrating each step as it happens — so that afterwards the operator understands what
//! the stack does, because they watched it do it once.
//!
//! This is the pure part of that: the steps in order, what each is called in plain
//! language, where a walkthrough can stop and what to say when it does, what is safe to
//! suggest to someone with no library yet, and what to point at when it works. Nothing
//! here reaches a service — running it is [`crate::app::walkthrough`], and drawing it is
//! the binary's.
//!
//! The pipeline it walks is the same one [`crate::trace`] reports on after the fact:
//! D9 answers "where did it get to?" for something already asked for, and this asks for
//! something and watches. They share [`crate::trace::Stage`] deliberately, so the two
//! never drift into two vocabularies for one journey.

mod diagnosis;
mod handover;
mod narration;
mod shape;
mod state;
mod step;
mod suggestion;

pub use diagnosis::{Reason, Stopped};
pub use handover::{Handover, Next};
pub use narration::{size, spell_out, Line, Narrator, Speed};
pub use shape::{Shape, Why};
pub use state::State;
pub use step::{Link, Step};
pub use suggestion::{Availability, Suggestion, SUGGESTIONS};

/// How large a download has to be before its size alone is worth calling out ahead of
/// the wait, in bytes. Below this the wait is short enough that a figure is noise.
pub const LARGE: u64 = 4 * 1024 * 1024 * 1024;

#[cfg(test)]
mod tests {
    use super::{Shape, State, Step, LARGE};

    #[test]
    fn the_walk_is_offered_before_it_is_anything_else() {
        // The first state is the offer, because the operator's first involvement is
        // being asked — not being walked.
        assert_eq!(State::default(), State::Offered);
        assert_eq!(Step::default(), Step::Choosing);
        assert_eq!(Shape::default(), Shape::Pipeline);
    }

    #[test]
    fn a_size_worth_stating_before_a_wait_is_gigabytes() {
        // Below this the wait is short enough that a figure is noise rather than warning.
        assert_eq!(LARGE / (1024 * 1024 * 1024), 4);
    }
}
