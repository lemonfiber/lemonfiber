//! The ports a test needs standing in for but never asks a question of.
//!
//! A runner that spawns nothing, a clock that does not move, randomness a test chose. Each
//! was written out two or three times across these crates, differing in nothing but a
//! constant — two `Idle`s that were byte-identical, two clocks apart only in which second
//! they stopped at, and four ways of scripting the same randomness.
//!
//! They are here for the reason the transport and the filesystem are: a fake that exists
//! twice is two places for the semantics to drift, and the drift is invisible until a test
//! passes against one copy and would have failed against the other.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use async_trait::async_trait;
use lemonfiber_ports::network::Site;
use lemonfiber_ports::process::{Failure as RunFailure, Output, Runner};
use lemonfiber_ports::random::Random;
use lemonfiber_ports::time::Clock;

/// A runner that spawns nothing.
///
/// For a path that must not reach a program: it answers as though nothing is installed, so
/// a test that unexpectedly shells out fails saying so rather than running something.
pub struct Idle;

#[async_trait]
impl Runner for Idle {
    async fn run(&self, _argv: &[String]) -> Result<Output, RunFailure> {
        Err(RunFailure::NotFound {
            program: "unused".to_owned(),
        })
    }
}

/// A clock stopped at a fixed moment, so what a run stamps is the same every time.
pub struct Stopped(SystemTime);

impl Stopped {
    /// Stopped this many seconds after the epoch.
    #[must_use]
    pub fn at(seconds: u64) -> Arc<Self> {
        Arc::new(Self(SystemTime::UNIX_EPOCH + Duration::from_secs(seconds)))
    }
}

impl Clock for Stopped {
    fn now(&self) -> SystemTime {
        self.0
    }
}

/// How a test scripts the randomness it is given.
enum Given {
    /// Exactly these bytes, however many were asked for — or nothing at all.
    Exactly(Option<Vec<u8>>),
    /// Letters cycled to the length asked for: a credential-shaped value built rather
    /// than written, which is how every credential fixture in this repository is made.
    Cycling,
}

/// Randomness a test chose, rather than any that varies between runs.
///
/// Named for what it is rather than for the port, because the port is already called
/// `Random` and a fake wearing the same name reads as the thing itself.
pub struct Chance(Given);

impl Chance {
    /// Exactly these bytes, or nothing at all where a test is about a source that cannot
    /// draw — which is a thing every caller has to survive.
    #[must_use]
    pub const fn exactly(bytes: Option<Vec<u8>>) -> Self {
        Self(Given::Exactly(bytes))
    }

    /// Letters cycled to whatever length is asked for.
    #[must_use]
    pub const fn cycling() -> Self {
        Self(Given::Cycling)
    }
}

impl Random for Chance {
    fn bytes(&self, n: usize) -> Option<Vec<u8>> {
        match &self.0 {
            Given::Exactly(bytes) => bytes.clone(),
            Given::Cycling => Some(
                ('a'..='p')
                    .map(|letter| letter as u8)
                    .cycle()
                    .take(n)
                    .collect(),
            ),
        }
    }
}

/// A machine that says what it is called, and can be made to say something else
/// next time.
///
/// The name varies with the machine, so a test written against the real one would
/// pass where it was written and nowhere else. The second answer is what makes a
/// renamed machine testable: a reader that remembered the first would go on giving
/// it after this one has changed.
pub struct Renamed {
    /// The answers, in the order they are given. The last is repeated once the rest
    /// have been handed out, so a caller asking a third time is not answered with an
    /// absence nobody scripted.
    answers: Vec<Option<String>>,
    /// How many have been asked for.
    asked: AtomicUsize,
}

impl Renamed {
    /// A machine that answers the same way however often it is asked.
    #[must_use]
    pub fn called(name: Option<&str>) -> Arc<Self> {
        Arc::new(Self {
            answers: vec![name.map(str::to_owned)],
            asked: AtomicUsize::new(0),
        })
    }

    /// A machine that answers this way once and that way afterwards.
    #[must_use]
    pub fn then(self: &Arc<Self>, name: Option<&str>) -> Arc<Self> {
        let mut answers = self.answers.clone();
        answers.push(name.map(str::to_owned));
        Arc::new(Self {
            answers,
            asked: AtomicUsize::new(0),
        })
    }

    /// How many times it has been asked.
    #[must_use]
    pub fn times(&self) -> usize {
        self.asked.load(Ordering::Relaxed)
    }
}

#[async_trait]
impl Site for Renamed {
    async fn name(&self) -> Option<String> {
        let asked = self.asked.fetch_add(1, Ordering::Relaxed);
        // The last answer stands once the rest have been handed out, so a caller
        // asking once more than a test scripted is not answered with an absence
        // nobody chose.
        let last = self.answers.len().saturating_sub(1);
        self.answers.get(asked.min(last)).cloned().flatten()
    }
}
