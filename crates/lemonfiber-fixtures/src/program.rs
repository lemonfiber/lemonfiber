//! The filesystem a copy of lemonfiber asks about itself.
//!
//! Apart from [`crate::files`] because the questions are different ones. That fake
//! stands in for a service's configuration and answers a read; this one stands in for
//! the ground a binary sits on, where four things matter and none of them is what a
//! file says: where a name resolves to once links are followed, whether the directory
//! holding it will take a new file, what small records lemonfiber left beside it, and
//! what a run wrote back.
//!
//! Nothing is held anywhere a test did not put it, and nothing resolves anywhere a
//! test did not point it. A machine that will not resolve a path at all is a state
//! worth reaching — it is what a binary that has been moved out from under a running
//! process looks like — so it is said rather than approximated by an empty answer.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use lemonfiber_ports::filesystem::{
    Fault, FileSystem, FsKind, Identity, Ownership, Storage, StorageFacts,
};

/// A filesystem around the running binary, scripted by a test.
#[derive(Default)]
pub struct Program {
    /// Where each path resolves to once links are followed.
    links: Vec<(PathBuf, PathBuf)>,
    /// Whether paths resolve at all.
    ///
    /// A machine that answers nothing is the one a fallback exists for, and a fake
    /// that always resolved would leave that fallback unreachable.
    resolves: bool,
    /// The text sitting at each exact path.
    held: Vec<(PathBuf, String)>,
    /// Whether a new file may be created.
    writable: bool,
    /// What was written, and where.
    written: Mutex<Vec<(PathBuf, String)>>,
    /// What was taken away.
    removed: Mutex<Vec<PathBuf>>,
}

impl Program {
    /// A machine that resolves every path to itself and will take a new file.
    #[must_use]
    pub fn ordinary() -> Self {
        Self {
            resolves: true,
            writable: true,
            ..Self::default()
        }
    }

    /// The same machine, with this text sitting at this exact path.
    #[must_use]
    pub fn holding(mut self, at: impl Into<PathBuf>, text: &str) -> Self {
        self.held.push((at.into(), text.to_owned()));
        self
    }

    /// The same machine, where this name resolves to that one.
    #[must_use]
    pub fn linked(mut self, from: impl Into<PathBuf>, to: impl Into<PathBuf>) -> Self {
        self.links.push((from.into(), to.into()));
        self
    }

    /// The same machine, resolving nothing at all.
    #[must_use]
    pub fn unresolvable(mut self) -> Self {
        self.resolves = false;
        self
    }

    /// The same machine, refusing to take a new file anywhere.
    #[must_use]
    pub fn unwritable(mut self) -> Self {
        self.writable = false;
        self
    }

    /// A handle the context takes.
    #[must_use]
    pub fn shared(self) -> Arc<Self> {
        Arc::new(self)
    }

    /// What was written, and where, in the order it was written.
    #[must_use]
    pub fn written(&self) -> Vec<(PathBuf, String)> {
        self.written
            .lock()
            .map(|written| written.clone())
            .unwrap_or_default()
    }

    /// What was taken away, in the order it went.
    #[must_use]
    pub fn removed(&self) -> Vec<PathBuf> {
        self.removed
            .lock()
            .map(|removed| removed.clone())
            .unwrap_or_default()
    }
}

#[async_trait]
impl FileSystem for Program {
    async fn canonicalize(&self, path: &Path) -> Result<PathBuf, Fault> {
        resolved(self, path)
    }

    async fn touch(&self, path: &Path) -> Result<(), Fault> {
        touched(self, path)
    }

    async fn link(&self, _from: &Path, _to: &Path) -> Result<(), Fault> {
        Err(Fault::new("unused"))
    }

    async fn identify(&self, _path: &Path) -> Result<Identity, Fault> {
        Err(Fault::new("unused"))
    }

    async fn remove(&self, path: &Path) {
        crate::noted(&self.removed, path.to_path_buf());
    }

    async fn read(&self, path: &Path) -> Option<String> {
        self.held
            .iter()
            .find(|(at, _)| at == path)
            .map(|(_, text)| text.clone())
    }

    async fn write(&self, path: &Path, contents: &str) {
        crate::noted(&self.written, (path.to_path_buf(), contents.to_owned()));
    }

    async fn ownership(&self, _path: &Path) -> Option<Ownership> {
        None
    }
}

#[async_trait]
impl Storage for Program {
    async fn describe(&self, _path: &Path) -> StorageFacts {
        StorageFacts {
            point: PathBuf::new(),
            kind: FsKind::Linking("test".to_owned()),
            removable: false,
            available: 0,
            total: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{FileSystem as _, Path, PathBuf, Program, Storage as _};

    #[tokio::test]
    async fn a_name_resolves_to_where_it_was_pointed_and_otherwise_to_itself() {
        let machine = Program::ordinary().linked("/usr/local/bin/lf", "/opt/Cellar/lf");

        assert_eq!(
            machine
                .canonicalize(Path::new("/usr/local/bin/lf"))
                .await
                .ok(),
            Some(PathBuf::from("/opt/Cellar/lf"))
        );
        assert_eq!(
            machine.canonicalize(Path::new("/elsewhere/lf")).await.ok(),
            Some(PathBuf::from("/elsewhere/lf"))
        );
    }

    #[tokio::test]
    async fn a_machine_that_resolves_nothing_says_so_rather_than_answering() {
        let machine = Program::ordinary().unresolvable();
        assert!(machine.canonicalize(Path::new("/anywhere")).await.is_err());
    }

    #[tokio::test]
    async fn what_a_run_wrote_and_what_it_took_away_are_both_readable_afterwards() {
        let machine = Program::ordinary().holding("/records/updates.json", "{}");

        assert_eq!(
            machine
                .read(Path::new("/records/updates.json"))
                .await
                .as_deref(),
            Some("{}")
        );
        assert_eq!(machine.read(Path::new("/records/nothing")).await, None);
        machine
            .write(Path::new("/records/updates.json"), "later")
            .await;
        machine.remove(Path::new("/bin/.probe")).await;
        assert_eq!(
            machine.written(),
            vec![(PathBuf::from("/records/updates.json"), "later".to_owned())]
        );
        assert_eq!(machine.removed(), vec![PathBuf::from("/bin/.probe")]);
    }

    #[tokio::test]
    async fn a_directory_that_will_not_take_a_file_says_which_path_it_refused() {
        let machine = Program::ordinary().unwritable();
        let refused = machine.touch(Path::new("/usr/bin/.probe")).await;
        assert_eq!(
            refused.err().map(|fault| fault.message),
            Some("permission denied: /usr/bin/.probe".to_owned())
        );
        assert!(Program::ordinary()
            .touch(Path::new("/tmp/.probe"))
            .await
            .is_ok());
    }

    /// The stub half of the contract, which nothing else reaches. A test that asks
    /// this fake for one of them has reached for a capability it was never meant to
    /// stand in for, so each refuses rather than answering plausibly.
    #[tokio::test]
    async fn every_capability_this_fake_does_not_stand_in_for_answers_as_unused() {
        let machine = Program::ordinary();
        let path = Path::new("/usr/bin/lemonfiber");

        assert!(machine.link(path, path).await.is_err());
        assert!(machine.identify(path).await.is_err());
        assert!(machine.ownership(path).await.is_none());
        assert_eq!(machine.describe(path).await.available, 0);
    }
}

/// The link a path resolves through, or the refusal a fixture was built to give.
fn resolved(program: &Program, path: &Path) -> Result<PathBuf, Fault> {
    if !program.resolves {
        return Err(Fault::new("no such file or directory"));
    }
    Ok(program
        .links
        .iter()
        .find(|(from, _)| from == path)
        .map_or_else(|| path.to_path_buf(), |(_, to)| to.clone()))
}

/// Whether this fixture lets a file be created.
fn touched(program: &Program, path: &Path) -> Result<(), Fault> {
    if program.writable {
        return Ok(());
    }
    Err(Fault::new(format!("permission denied: {}", path.display())))
}
