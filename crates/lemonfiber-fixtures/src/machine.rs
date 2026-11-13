//! The machine itself: when it started, where its power comes from, and which processes
//! are still running on it.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use lemonfiber_ports::machine::{Power, Running, Started, Supply};

/// A machine that says when it started and where its power comes from, or will not say.
pub struct Booted {
    /// When it started, in seconds since the epoch, where it says.
    pub at: Option<u64>,
    /// Where its power comes from, where it says.
    pub power: Option<Power>,
}

#[async_trait]
impl Started for Booted {
    async fn at(&self) -> Option<u64> {
        self.at
    }
}

#[async_trait]
impl Supply for Booted {
    async fn source(&self) -> Option<Power> {
        self.power
    }
}

/// Something done the first time a machine is asked whether a process is alive.
type Meanwhile = Box<dyn Fn() + Send + Sync>;

/// A machine answering whether a process is alive with each scripted answer in turn,
/// the last repeating once the rest are spent.
pub struct Living {
    answers: Vec<Option<bool>>,
    asked: Mutex<usize>,
    /// What happens the first time it is asked, before it answers.
    meanwhile: Option<Meanwhile>,
}

impl Living {
    /// A machine answering these, in order.
    #[must_use]
    pub fn saying(answers: &[Option<bool>]) -> Arc<Self> {
        Arc::new(Self {
            answers: answers.to_vec(),
            asked: Mutex::new(0),
            meanwhile: None,
        })
    }

    /// A machine answering these, in order, doing `meanwhile` the first time it is asked.
    #[must_use]
    pub fn meanwhile(
        answers: &[Option<bool>],
        meanwhile: impl Fn() + Send + Sync + 'static,
    ) -> Arc<Self> {
        Arc::new(Self {
            answers: answers.to_vec(),
            asked: Mutex::new(0),
            meanwhile: Some(Box::new(meanwhile)),
        })
    }

    /// How many times it was asked.
    #[must_use]
    pub fn times(&self) -> usize {
        self.asked.lock().map(|asked| *asked).unwrap_or_default()
    }
}

#[async_trait]
impl Running for Living {
    async fn alive(&self, _pid: u32) -> Option<bool> {
        let asked = self
            .asked
            .lock()
            .map(|mut asked| {
                *asked += 1;
                *asked - 1
            })
            .unwrap_or_default();
        if asked == 0 {
            if let Some(meanwhile) = &self.meanwhile {
                meanwhile();
            }
        }
        let last = self.answers.len().saturating_sub(1);
        self.answers.get(asked.min(last)).copied().flatten()
    }
}
