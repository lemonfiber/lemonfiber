//! The command line this binary parses, and the reference generated from it.
//!
//! Only the declarations live here. The dispatcher that routes them, the surfaces
//! that render the answers, and everything they reach are the binary's own, next
//! door in `main.rs` — a library because the reference is emitted by a program that
//! is not this binary, and it must read the same declarations rather than a second
//! description of them.
//!
//! [`reaching`] is here for the same reason read the other way round: what the
//! dashboard offers is decided in the binary, and a guard outside it has to be able
//! to read the answer.

pub mod cli;
pub mod codes;
pub mod reaching;
pub mod reference;
