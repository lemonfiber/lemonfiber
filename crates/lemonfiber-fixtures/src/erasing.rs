//! An eraser that records what it was asked to remove.
//!
//! Removal is the one operation a test cannot let reach a real filesystem and still
//! be a test, and it is also the one where *what was asked for* matters as much as
//! what came back: a run that removed the right thing and a run that removed the
//! wrong one both answer the same way. So this records, and the assertion is on the
//! paths.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use lemonfiber_ports::filesystem::{Eraser, Fault};

/// An eraser that agrees to everything, or refuses everything, and remembers what it
/// was asked.
pub struct Erasing {
    refuses: Option<String>,
    asked: Mutex<Vec<PathBuf>>,
}

impl Erasing {
    /// One that removes whatever it is given.
    #[must_use]
    pub fn willing() -> Arc<Self> {
        Arc::new(Self {
            refuses: None,
            asked: Mutex::new(Vec::new()),
        })
    }

    /// One that refuses, in the platform's words.
    #[must_use]
    pub fn refusing(why: &str) -> Arc<Self> {
        Arc::new(Self {
            refuses: Some(why.to_owned()),
            asked: Mutex::new(Vec::new()),
        })
    }

    /// Every path it was asked to remove, in the order it was asked.
    #[must_use]
    pub fn asked(&self) -> Vec<PathBuf> {
        self.asked
            .lock()
            .map(|asked| asked.clone())
            .unwrap_or_default()
    }
}

#[async_trait]
impl Eraser for Erasing {
    async fn erase(&self, path: &Path) -> Result<(), Fault> {
        if let Ok(mut asked) = self.asked.lock() {
            asked.push(path.to_path_buf());
        }
        match &self.refuses {
            None => Ok(()),
            Some(why) => Err(Fault::new(why.clone())),
        }
    }
}
