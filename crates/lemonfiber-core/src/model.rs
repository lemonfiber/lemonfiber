//! The values a surface renders.
//!
//! One set of types, serialised directly. `--json` and the web API are the same
//! values rather than two hand-maintained projections of them, which is what
//! makes the web API and the TUI's interface the same thing by construction —
//! and gives the machine-readable contract exactly one thing to version.

/// The machine-readable output contract's version.
///
/// Additive change leaves it alone, so a script asserting `== 1` keeps working
/// as features are added. Removing or retyping a field increments it.
pub const API_VERSION: u32 = 1;

mod alerts;
mod asking;
mod catalogue;
mod checking;
mod door;
mod envelope;
mod held;
mod history;
mod hosting;
mod household;
mod invitation;
pub mod kind;
mod migration;
mod provenance;
mod quality;
mod queue;
mod running;
mod self_update;
mod settings;
mod trace;
mod upgrade;
mod walkthrough;
mod wiring;

pub use alerts::*;
pub use asking::*;
pub use catalogue::*;
pub use checking::*;
pub use door::*;
pub use envelope::*;
pub use held::*;
pub use history::*;
pub use hosting::*;
pub use household::*;
pub use invitation::*;
pub use migration::*;
pub use provenance::*;
pub use quality::*;
pub use queue::*;
pub use running::*;
pub use self_update::*;
pub use settings::*;
pub use trace::*;
pub use upgrade::*;
pub use walkthrough::*;
pub use wiring::*;

#[cfg(test)]
mod tests;
