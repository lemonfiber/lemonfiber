//! What a machine a test is pointed at says it has, and what it says it has not.
//!
//! Apart from the engine fake for the reason the port is apart from the engine: what
//! a test varies here is what is on the machine under the daemon, and nothing else
//! about an engine bears on it.
//!
//! The third shape is the one no real daemon will produce on request, and the reason
//! this is scripted rather than derived from a directory: a machine that says yes to
//! everything, including somewhere that cannot be there. That is what a check which
//! has stopped being able to say no looks like from the inside, and it has to be
//! reachable from a test or the guard against it is a guard nobody has ever seen work.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use lemonfiber_ports::docker::{Failure, Locations, Mount, Presence};

/// What a machine answers when it is asked about a path.
pub enum Answers {
    /// It has exactly these, and nothing else.
    Only(Vec<PathBuf>),
    /// The same thing about everything, whatever it is asked.
    Always(Presence),
    /// Nothing at all: it could not be reached.
    Nothing(String),
}

/// A machine that answers about its own paths this way.
pub struct Located {
    answers: Answers,
    asked: Mutex<Vec<PathBuf>>,
    container: Option<(String, Vec<Mount>)>,
}

impl Located {
    /// A machine holding exactly these locations.
    #[must_use]
    pub fn holding(held: &[&str]) -> Arc<Self> {
        Self::with(Answers::Only(held.iter().map(PathBuf::from).collect()))
    }

    /// A machine that says the same thing about everywhere.
    #[must_use]
    pub fn saying(answer: Presence) -> Arc<Self> {
        Self::with(Answers::Always(answer))
    }

    /// A machine that cannot be reached to be asked, in its own words.
    #[must_use]
    pub fn unreachable(reason: &str) -> Arc<Self> {
        Self::with(Answers::Nothing(reason.to_owned()))
    }

    /// A machine answering however this says.
    #[must_use]
    pub fn with(answers: Answers) -> Arc<Self> {
        Arc::new(Self {
            answers,
            asked: Mutex::new(Vec::new()),
            container: None,
        })
    }

    /// A machine running one container, with these host paths mounted at these
    /// container paths, given as `(host, container)`.
    ///
    /// Asked about any other container, it has none by that name.
    #[must_use]
    pub fn running(container: &str, mounts: &[(&str, &str)]) -> Arc<Self> {
        Arc::new(Self {
            answers: Answers::Only(Vec::new()),
            asked: Mutex::new(Vec::new()),
            container: Some((
                container.to_owned(),
                mounts
                    .iter()
                    .map(|(source, destination)| Mount {
                        source: PathBuf::from(source),
                        destination: PathBuf::from(destination),
                    })
                    .collect(),
            )),
        })
    }

    /// Everywhere it was asked about, in order.
    #[must_use]
    pub fn asked(&self) -> Vec<PathBuf> {
        self.asked
            .lock()
            .map(|held| held.clone())
            .unwrap_or_default()
    }
}

#[async_trait]
impl Locations for Located {
    async fn located(&self, path: &Path) -> Result<Presence, Failure> {
        answered(self, path)
    }

    async fn mounted(&self, container: &str) -> Result<Option<Vec<Mount>>, Failure> {
        described(self, container)
    }
}

/// What this fixture says a container has mounted.
///
/// A machine that cannot be reached cannot describe a container either, so it fails
/// here in the same words it fails about a path.
fn described(machine: &Located, container: &str) -> Result<Option<Vec<Mount>>, Failure> {
    if let Answers::Nothing(reason) = &machine.answers {
        return Err(Failure::Unreachable {
            reason: reason.clone(),
        });
    }
    Ok(machine
        .container
        .as_ref()
        .filter(|(id, _)| id == container)
        .map(|(_, mounts)| mounts.clone()))
}

/// What this fixture says about a path, and the record that it was asked.
fn answered(machine: &Located, path: &Path) -> Result<Presence, Failure> {
    crate::noted(&machine.asked, path.to_path_buf());

    match &machine.answers {
        Answers::Only(held) => Ok(if held.iter().any(|there| there == path) {
            Presence::There
        } else {
            Presence::Absent
        }),
        Answers::Always(answer) => Ok(*answer),
        Answers::Nothing(reason) => Err(Failure::Unreachable {
            reason: reason.clone(),
        }),
    }
}
