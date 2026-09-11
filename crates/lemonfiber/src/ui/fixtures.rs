//! The fakes both halves of this surface's tests are driven through.
//!
//! Shared between the module that serves and the module that says what it is
//! serving, because a fake copied per module is a fake that drifts per module.

use std::net::SocketAddr;

use async_trait::async_trait;
use lemonfiber_core::ports::process::{Failure, Output, Runner};

/// A run of a program that went the way the test chose.
pub(crate) struct Ran(Result<Output, Failure>);

#[async_trait]
impl Runner for Ran {
    async fn run(&self, _: &[String]) -> Result<Output, Failure> {
        ran(&self.0)
    }
}

/// What the fake was built to answer with, as a fresh answer each time.
fn ran(answer: &Result<Output, Failure>) -> Result<Output, Failure> {
    match answer {
        Ok(output) => Ok(output.clone()),
        Err(_) => Err(Failure::NotFound {
            program: "xdg-open".to_owned(),
        }),
    }
}

/// A program that ran and exited with this status.
pub(crate) fn exited(status: i32) -> Ran {
    Ran(Ok(Output {
        status: Some(status),
        stdout: String::new(),
        stderr: String::new(),
    }))
}

/// A program that is not installed.
pub(crate) fn missing() -> Ran {
    Ran(Err(Failure::NotFound {
        program: "xdg-open".to_owned(),
    }))
}

/// An address of numbers, which cannot fail to be one.
pub(crate) fn bound() -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], 8471))
}
