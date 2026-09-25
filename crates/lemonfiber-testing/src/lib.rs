//! The context lemonfiber's tests drive a command through, and the words they hand a
//! service where it would take a credential.
//!
//! Every test outside the core that runs a command builds its context here, from one
//! of two starting points, and names what it varies. The core's own tests read the same
//! file: a crate cannot depend on a crate that depends on it without being built twice,
//! so the core includes [`context`] as a module of its own rather than as this crate.

pub mod context;
pub mod placeholder;

pub use context::{a_context, a_live_context, nowhere, repository_stack, Context};
pub use placeholder::a_word;
