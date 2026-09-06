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
        if let Some(refusal) = self.refusal() {
            return Err(refusal);
        }
        if let Ok(mut placed) = self.placed.lock() {
            placed.push(hosted.clone());
        }
        let definition = definition(&hosted.name);
        if let Ok(mut held) = self.held.lock() {
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

    async fn standing(&self, name: &str) -> Result<Held, Failure> {
        if !self.manager.configurable() {
            return Err(Failure::Unhostable);
        }
        Ok(self
            .held
            .lock()
            .ok()
            .and_then(|held| held.get(name).cloned())
            .unwrap_or_else(Held::absent))
    }

    async fn withdraw(&self, name: &str) -> Result<Vec<PathBuf>, Failure> {
        if let Some(refusal) = self.refusal() {
            return Err(refusal);
        }
        if let Ok(mut withdrawn) = self.withdrawn.lock() {
            withdrawn.push(name.to_owned());
        }
        let taken = self
            .held
            .lock()
            .ok()
            .and_then(|mut held| held.remove(name))
            .and_then(|held| held.definition);
        Ok(taken.into_iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::{Failure, Held, Host, Hosted, Hosting, Manager, Placed, Program, Standing};
    use std::path::PathBuf;

    fn a_command() -> Hosted {
        Hosted {
            name: "watch".to_owned(),
            program: PathBuf::from("/usr/local/bin/lemonfiber"),
            arguments: vec!["watch".to_owned(), "full".to_owned()],
            output: PathBuf::from("/records/watch.log"),
            about: "guards the data location".to_owned(),
        }
    }

    #[tokio::test]
    async fn what_was_installed_is_what_it_afterwards_holds() {
        let hosting = Hosting::with(Manager::Launchd);
        assert_eq!(hosting.manager(), Manager::Launchd);
        assert_eq!(hosting.standing("watch").await, Ok(Held::absent()));

        assert_eq!(
            hosting.place(&a_command()).await,
            Ok(Placed {
                definition: PathBuf::from("/services/lemonfiber-watch"),
                started: true,
            })
        );
        assert_eq!(hosting.placed(), vec![a_command()]);
        assert_eq!(
            hosting.standing("watch").await,
            Ok(Held {
                standing: Standing::Running,
                definition: Some(PathBuf::from("/services/lemonfiber-watch")),
                program: Some(Program {
                    at: PathBuf::from("/usr/local/bin/lemonfiber"),
                    present: true,
                }),
                runs: Some("watch full".to_owned()),
                output: Some(PathBuf::from("/records/watch.log")),
            })
        );
    }

    #[tokio::test]
    async fn what_is_taken_back_is_named_and_then_gone() {
        let hosting = Hosting::with(Manager::Systemd);
        assert!(hosting.place(&a_command()).await.is_ok());
        assert_eq!(
            hosting.withdraw("watch").await,
            Ok(vec![PathBuf::from("/services/lemonfiber-watch")])
        );
        assert_eq!(hosting.withdrawn(), vec!["watch".to_owned()]);
        assert_eq!(hosting.standing("watch").await, Ok(Held::absent()));
        assert_eq!(hosting.withdraw("watch").await, Ok(Vec::new()));
    }

    #[tokio::test]
    async fn a_platform_with_no_manager_answers_nothing_at_all() {
        let hosting = Hosting::unsupported();
        assert_eq!(hosting.manager(), Manager::Unsupported);
        assert_eq!(hosting.place(&a_command()).await, Err(Failure::Unhostable));
        assert_eq!(hosting.standing("watch").await, Err(Failure::Unhostable));
        assert_eq!(hosting.withdraw("watch").await, Err(Failure::Unhostable));
    }

    #[tokio::test]
    async fn a_manager_that_refuses_still_answers_what_it_holds() {
        let hosting = Hosting::refusing(Manager::Launchd, "Load failed: 5");
        let refusal = || Failure::Refused {
            manager: "launchd",
            reason: "Load failed: 5".to_owned(),
        };
        assert_eq!(hosting.place(&a_command()).await, Err(refusal()));
        assert_eq!(hosting.withdraw("watch").await, Err(refusal()));
        assert_eq!(hosting.standing("watch").await, Ok(Held::absent()));
    }

    #[tokio::test]
    async fn a_machine_can_be_built_already_holding_one() {
        let hosting = Hosting::holding(
            Manager::Systemd,
            "expiring",
            Hosting::installed("expiring", Standing::Stopped),
        );
        assert_eq!(
            hosting.standing("expiring").await,
            Ok(Hosting::installed("expiring", Standing::Stopped))
        );

        let gone: Held = Hosting::orphaned("expiring");
        assert!(gone.orphaned());
        assert_eq!(gone.standing, Standing::Stopped);
        assert!(gone.definition.is_some());
    }
}
