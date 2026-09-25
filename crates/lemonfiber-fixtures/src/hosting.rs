//! A service manager that keeps what it was given, and says what it was asked.
//!
//! The real ones write a file and then ask the platform whether it took, which is
//! two things a test cannot do and does not want to. What a test about hosting is
//! actually asking is what lemonfiber does with the answer — so this holds a map
//! of names to what stands under them, and every case a caller has to handle is
//! reachable by constructing one: a machine with no manager at all, a manager
//! that refuses, a name nothing is installed under, and a name installed against
//! a program that has since gone.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use lemonfiber_ports::hosting::{Failure, Held, Host, Hosted, Manager, Placed, Program, Standing};

/// Where this fake pretends definitions are kept.
const SOMEWHERE: &str = "/services";

/// A service manager holding whatever it was given.
pub struct Hosting {
    manager: Manager,
    held: Mutex<BTreeMap<String, Held>>,
    refuses: Option<String>,
    placed: Mutex<Vec<Hosted>>,
    withdrawn: Mutex<Vec<String>>,
}

impl Hosting {
    /// A manager of the given kind, holding nothing and agreeing to everything.
    #[must_use]
    pub fn with(manager: Manager) -> Arc<Self> {
        Arc::new(Self {
            manager,
            held: Mutex::new(BTreeMap::new()),
            refuses: None,
            placed: Mutex::new(Vec::new()),
            withdrawn: Mutex::new(Vec::new()),
        })
    }

    /// A machine whose platform lemonfiber does not configure.
    #[must_use]
    pub fn unsupported() -> Arc<Self> {
        Self::with(Manager::Unsupported)
    }

    /// A manager that refuses everything asked of it, in its own words.
    #[must_use]
    pub fn refusing(manager: Manager, reason: &str) -> Arc<Self> {
        Arc::new(Self {
            manager,
            held: Mutex::new(BTreeMap::new()),
            refuses: Some(reason.to_owned()),
            placed: Mutex::new(Vec::new()),
            withdrawn: Mutex::new(Vec::new()),
        })
    }

    /// A manager already holding one name, in the state given.
    #[must_use]
    pub fn holding(manager: Manager, name: &str, held: Held) -> Arc<Self> {
        let one = Self::with(manager);
        if let Ok(mut map) = one.held.lock() {
            map.insert(name.to_owned(), held);
        }
        one
    }

    /// What is installed under a name, as this fake would answer for it.
    #[must_use]
    pub fn installed(name: &str, standing: Standing) -> Held {
        Held {
            standing,
            definition: Some(definition(name)),
            program: Some(Program {
                at: PathBuf::from("/usr/local/bin/lemonfiber"),
                present: true,
            }),
            runs: Some(format!("/usr/local/bin/lemonfiber {name}")),
            output: Some(PathBuf::from(format!("/records/{name}.log"))),
        }
    }

    /// What is installed under a name whose program has since gone.
    #[must_use]
    pub fn orphaned(name: &str) -> Held {
        Held {
            program: Some(Program {
                at: PathBuf::from("/gone/lemonfiber"),
                present: false,
            }),
            ..Self::installed(name, Standing::Stopped)
        }
    }

    /// Everything it was asked to install, in the order it was asked.
    #[must_use]
    pub fn placed(&self) -> Vec<Hosted> {
        self.placed
            .lock()
            .map(|placed| placed.clone())
            .unwrap_or_default()
    }

    /// Every name it was asked to take back, in the order it was asked.
    #[must_use]
    pub fn withdrawn(&self) -> Vec<String> {
        self.withdrawn
            .lock()
            .map(|withdrawn| withdrawn.clone())
            .unwrap_or_default()
    }

    /// The failure this fake was built to answer with, where it was built to fail.
    fn refusal(&self) -> Option<Failure> {
        if !self.manager.configurable() {
            return Some(Failure::Unhostable);
        }
        self.refuses.as_ref().map(|reason| Failure::Refused {
            manager: self.manager.named(),
            reason: reason.clone(),
        })
    }
}

/// Where this fake says a name's definition lives.
fn definition(name: &str) -> PathBuf {
    PathBuf::from(format!("{SOMEWHERE}/lemonfiber-{name}"))
}

#[async_trait]
impl Host for Hosting {
    fn manager(&self) -> Manager {
        self.manager
    }

    async fn place(&self, hosted: &Hosted) -> Result<Placed, Failure> {
        placed(self, hosted)
    }

    async fn standing(&self, name: &str) -> Result<Held, Failure> {
        standing_of(self, name)
    }

    async fn withdraw(&self, name: &str) -> Result<Vec<PathBuf>, Failure> {
        withdrawn(self, name)
    }
}

/// Remember the placement and answer as though it took, unless told to refuse.
fn placed(hosting: &Hosting, hosted: &Hosted) -> Result<Placed, Failure> {
    if let Some(refusal) = hosting.refusal() {
        return Err(refusal);
    }
    crate::noted(&hosting.placed, hosted.clone());
    let definition = definition(&hosted.name);
    if let Ok(mut held) = hosting.held.lock() {
        held.insert(
            hosted.name.clone(),
            Held {
                standing: Standing::Running,
                definition: Some(definition.clone()),
                program: Some(Program {
                    at: hosted.program.clone(),
                    present: true,
                }),
                runs: Some(hosted.arguments.join(" ")),
                output: Some(hosted.output.clone()),
            },
        );
    }
    Ok(Placed {
        definition,
        started: true,
    })
}

/// What this fixture is holding under a name, where the manager can hold anything.
fn standing_of(hosting: &Hosting, name: &str) -> Result<Held, Failure> {
    if !hosting.manager.configurable() {
        return Err(Failure::Unhostable);
    }
    Ok(hosting
        .held
        .lock()
        .ok()
        .and_then(|held| held.get(name).cloned())
        .unwrap_or_else(Held::absent))
}

/// Forget the name, and hand back the definition it had.
fn withdrawn(hosting: &Hosting, name: &str) -> Result<Vec<PathBuf>, Failure> {
    if let Some(refusal) = hosting.refusal() {
        return Err(refusal);
    }
    crate::noted(&hosting.withdrawn, name.to_owned());
    let taken = hosting
        .held
        .lock()
        .ok()
        .and_then(|mut held| held.remove(name))
        .and_then(|held| held.definition);
    Ok(taken.into_iter().collect())
}

#[cfg(test)]
mod tests;
