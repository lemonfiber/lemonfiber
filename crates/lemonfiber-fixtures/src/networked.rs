//! The networks a test says the engine holds for a project, and which of them a run took
//! away.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use lemonfiber_ports::docker::{Failure, Network, Networks};

/// An engine holding exactly these networks, recording each one taken away.
pub struct Networked {
    held: Vec<Network>,
    refusing: bool,
    taken: Mutex<Vec<String>>,
}

impl Networked {
    /// An engine holding no network.
    #[must_use]
    pub fn none() -> Arc<Self> {
        Self::holding(&[])
    }

    /// An engine holding a network for each name a project's file declares, named as
    /// Compose names it for `project`.
    #[must_use]
    pub fn holding(declared: &[(&str, &str)]) -> Arc<Self> {
        Arc::new(Self {
            held: declared
                .iter()
                .map(|(project, declared)| Network {
                    name: format!("{project}_{declared}"),
                    declared: (*declared).to_owned(),
                })
                .collect(),
            refusing: false,
            taken: Mutex::new(Vec::new()),
        })
    }

    /// The same, refusing to take any of them away.
    #[must_use]
    pub fn refusing(declared: &[(&str, &str)]) -> Arc<Self> {
        let holding = Self::holding(declared);
        Arc::new(Self {
            held: holding.held.clone(),
            refusing: true,
            taken: Mutex::new(Vec::new()),
        })
    }

    /// Record the network `name` as taken away, unless this engine refuses.
    fn took(&self, name: &str) -> Result<(), Failure> {
        if self.refusing {
            return Err(Failure::Unreachable {
                reason: format!("{name} has active endpoints"),
            });
        }
        if let Ok(mut taken) = self.taken.lock() {
            taken.push(name.to_owned());
        }
        Ok(())
    }

    /// Every network taken away, by the engine's name, in the order it was.
    #[must_use]
    pub fn taken(&self) -> Vec<String> {
        self.taken
            .lock()
            .map(|taken| taken.clone())
            .unwrap_or_default()
    }
}

#[async_trait]
impl Networks for Networked {
    async fn networks(&self, project: &str) -> Result<Vec<Network>, Failure> {
        let prefix = format!("{project}_");
        Ok(self
            .held
            .iter()
            .filter(|network| network.name.starts_with(&prefix))
            .cloned()
            .collect())
    }

    async fn removed(&self, name: &str) -> Result<(), Failure> {
        self.took(name)
    }
}
