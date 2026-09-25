//! The images a test says this machine has pulled.
//!
//! Apart from the engine fake for the reason the port is apart from the engine: what
//! a test varies here is who is standing on an image, and nothing else about an
//! engine bears on it.

use std::sync::Arc;

use async_trait::async_trait;
use lemonfiber_ports::docker::{Failure, Image, Images};

/// An engine that has pulled exactly these images, or one that will not say.
pub struct Pulled {
    held: Vec<Image>,
    unreachable: Option<String>,
}

impl Pulled {
    /// An engine holding exactly these images.
    #[must_use]
    pub fn holding(held: Vec<Image>) -> Arc<Self> {
        Arc::new(Self {
            held,
            unreachable: None,
        })
    }

    /// An engine that will not answer, in its own words.
    #[must_use]
    pub fn unreachable(reason: &str) -> Arc<Self> {
        Arc::new(Self {
            held: Vec::new(),
            unreachable: Some(reason.to_owned()),
        })
    }

    /// One image, its size, and the projects standing on it.
    #[must_use]
    pub fn image(tag: &str, bytes: u64, projects: &[&str]) -> Image {
        Image {
            tags: vec![tag.to_owned()],
            bytes,
            projects: projects.iter().map(|name| (*name).to_owned()).collect(),
        }
    }
}

#[async_trait]
impl Images for Pulled {
    async fn images(&self) -> Result<Vec<Image>, Failure> {
        listed(self)
    }
}

/// What this fixture holds, or the unreachability it was built to report.
fn listed(pulled: &Pulled) -> Result<Vec<Image>, Failure> {
    match &pulled.unreachable {
        Some(reason) => Err(Failure::Unreachable {
            reason: reason.clone(),
        }),
        None => Ok(pulled.held.clone()),
    }
}

#[cfg(test)]
mod tests;
