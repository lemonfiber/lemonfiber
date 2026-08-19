//! What an operator is told, in the words the boundary carries it in.
//!
//! A digest is what a channel is handed: the run's findings gathered, classified, and
//! reduced to what this operator asked to hear about. It is data rather than logic —
//! deciding *when* to send one is the layer above's, and it stays there.
//!
//! Here rather than above the boundary because the notify port delivers one, and a port
//! that could not name what it delivers would push the naming into every channel.

mod appetite;
mod class;
mod digest;
mod kind;
mod moment;

pub use appetite::{Appetite, Wants};
pub use class::Class;
pub use digest::{Digest, FLAPPING};
pub use kind::is_ours;
pub use moment::{Alert, Moment};
