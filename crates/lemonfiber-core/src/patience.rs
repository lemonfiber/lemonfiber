//! How long a service is given to come round to an answer it is working towards.

use std::future::Future;
use std::time::Duration;

/// How long between asks, and how many asks, while a service works towards an answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Patience {
    /// How long before each ask.
    pub between: Duration,
    /// How many asks there are.
    pub asks: u32,
}

/// A service starting again, which takes a few seconds and up to two minutes on a slow
/// machine.
pub(crate) const RESTART: Patience = Patience {
    between: Duration::from_secs(3),
    asks: 40,
};

/// A media server reading one item afresh in the background.
pub(crate) const REFRESH: Patience = Patience {
    between: Duration::from_secs(3),
    asks: 20,
};

impl Patience {
    /// The longest it waits.
    #[must_use]
    pub(crate) fn longest(self) -> Duration {
        self.between.saturating_mul(self.asks)
    }

    /// Ask after each wait until an answer is `done` or the asks run out, and answer the
    /// last answer either way.
    pub(crate) async fn until<T, Asking, Asked>(
        self,
        mut ask: Asking,
        done: impl Fn(&T) -> bool,
    ) -> T
    where
        Asking: FnMut() -> Asked,
        Asked: Future<Output = T>,
    {
        let mut asked = 0;
        loop {
            tokio::time::sleep(self.between).await;
            asked += 1;
            let answer = ask().await;
            if done(&answer) || asked >= self.asks {
                return answer;
            }
        }
    }
}

#[cfg(test)]
mod tests;
