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
        match &self.unreachable {
            Some(reason) => Err(Failure::Unreachable {
                reason: reason.clone(),
            }),
            None => Ok(self.held.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use lemonfiber_ports::docker::Images as _;

    use super::Pulled;

    #[tokio::test]
    async fn an_engine_answers_with_what_the_test_put_in_it() {
        let listed = Pulled::holding(vec![Pulled::image("sonarr:1", 400, &["lemonfiber"])])
            .images()
            .await;

        assert_eq!(
            listed.map(|images| images.len()),
            Ok(1),
            "the scripted image is the one that comes back"
        );
    }

    #[tokio::test]
    async fn an_engine_that_will_not_say_keeps_its_own_words() {
        let refused = Pulled::unreachable("no daemon here").images().await;

        assert!(refused.is_err_and(|failure| failure.to_string().contains("no daemon here")));
    }
}
