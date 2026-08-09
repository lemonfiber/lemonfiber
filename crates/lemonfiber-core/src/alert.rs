//! Telling the operator, once, when it matters.
//!
//! A check finds something wrong; a condition remembers it; an alert is the moment
//! the operator is interrupted about it. Those are three different decisions and
//! this is the third, kept apart from the other two because the reasons to stay
//! quiet are not the reasons something is wrong.
//!
//! Almost everything here exists to *not* send something. A fault that is still
//! the same fault has already been reported. A service flapping between broken and
//! fixed produces one alert about the flapping rather than forty about the states.
//! Six things going wrong at once is one bad moment, not six interruptions. An
//! operator who stops reading alerts is worse off than one who was never sent any,
//! and every rule here is in service of not becoming that.
//!
//! Nothing here delivers anything. What a channel is and how it fails is the next
//! layer's; deciding what is worth saying is this one's.

mod digest;
mod moment;

pub use digest::{Digest, FLAPPING};
pub use moment::{Alert, Moment};
