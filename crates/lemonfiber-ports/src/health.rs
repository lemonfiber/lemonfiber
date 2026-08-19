//! What the whole stack amounts to, in one word.
//!
//! The summary is computed from what is *wrong* rather than from what is running.
//! Those come apart exactly where it matters: sixteen containers can be up and
//! answering while torrent traffic leaves outside the tunnel, and a summary built
//! from a container count calls that healthy. It is not healthy. It is the worst
//! thing the stack has ever done, and the operator is being told everything is
//! fine while it happens.
//!
//! So the input is the conditions — the operator-affecting findings the checks
//! raised — and how far the stack got is consulted only to tell "nothing is wrong"
//! apart from "nothing is running" and "nobody has looked yet".
//!
//! One computation, so every surface says the same thing. A dashboard and a
//! command that disagree about whether the stack is healthy teach the operator to
//! trust neither.
//!
//! - [`Reach`] — how far the stack got towards being askable.
//! - [`Standing`] — the one word, and how the words rank.
//! - [`Summary`] — the line itself.
//! - [`observed`] — turning what a surface saw into the conditions to summarise.

pub mod observed;
mod reach;
mod standing;
mod summary;

pub use observed::{observed, Egress};
pub use reach::Reach;
pub use standing::Standing;
pub use summary::Summary;
