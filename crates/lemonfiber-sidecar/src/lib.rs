//! The files lemonfiber and the images it builds for the stack hand each other.
//!
//! A lemonfiber-built image runs in the stack and holds no state of its own beyond
//! what it records. What it acts on, the core writes into its configuration
//! directory, and what it records, the core reads back. This crate is the one
//! definition of those files, for both sides: the core writes them through its
//! filesystem port, and the image reads them with whatever it has.
//!
//! It holds formats and nothing else. No file is opened here, nothing reaches the
//! network, and nothing depends on the core, so an image that depends on this crate
//! carries no more of lemonfiber than the shapes it shares.

pub mod decline;

mod unreadable;

pub use unreadable::Unreadable;
