//! The fakes both halves of this surface's tests are driven through.
//!
//! Shared between the module that serves and the module that says what it is
//! serving, because a fake copied per module is a fake that drifts per module.

use std::net::SocketAddr;

use lemonfiber_core::ports::process::{Failure, Output};
use lemonfiber_fixtures::support::Scripted;

/// A program that ran and exited with this status.
pub(crate) fn exited(status: i32) -> Scripted {
    Scripted(Ok(Output {
        status: Some(status),
        stdout: String::new(),
        stderr: String::new(),
    }))
}

/// A program that is not installed.
pub(crate) fn missing() -> Scripted {
    Scripted(Err(Failure::NotFound {
        program: "xdg-open".to_owned(),
    }))
}

/// An address of numbers, which cannot fail to be one.
pub(crate) fn bound() -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], 8471))
}
