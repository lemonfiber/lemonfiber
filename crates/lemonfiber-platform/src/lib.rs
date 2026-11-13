//! Which machine this is, and every operation whose meaning differs from one platform
//! to the next.
//!
//! The one crate of the workspace that asks what it is compiled for. [`host`] decides
//! the operating system and the environment as pure functions over what it is handed,
//! [`fs`] holds the single file operations that differ, and [`beneath`] the walk beneath
//! a directory somebody else can write, through each step's own handle. Everything
//! else calls these and never asks, which an architecture test holds.
//!
//! It depends on nothing of ours, so every crate of the workspace may reach it.

pub mod beneath;
pub mod fs;
pub mod host;

#[cfg(test)]
mod scratch;
