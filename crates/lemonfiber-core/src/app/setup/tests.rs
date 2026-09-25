use crate::validate::Credential;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;

use super::{
    progress_at, run, Applying, CredentialChoice, Outcome, Prompt, ProviderEntry, StorageWarning,
};
use crate::alert::Appetite;
use crate::config::paths::Paths;
use crate::config::{store, Protocols};
use crate::platform::Environment;
use crate::ports::filesystem::{
    Fault, FileSystem, FsKind, Identity, Ownership, Storage, StorageFacts,
};
use crate::prerequisites::PrerequisiteMap;
use crate::stack::Source;
use crate::validate::{Validation, Validator};
use crate::wizard::{Answer, Library, Phase, Plan, Progress, Vpn, Wizard};

/// A validator that answers with scripted outcomes, so a run is driven with no
/// network. Each call takes the next outcome, and the last is repeated once they
/// run out — so one failing then passing drives a retry, and a single outcome
/// answers however many times it is asked.
struct Proving {
    outcomes: Vec<Validation>,
    calls: AtomicUsize,
    asked: std::sync::Mutex<Vec<Credential>>,
}

impl Proving {
    fn giving(outcomes: Vec<Validation>) -> Self {
        Self {
            outcomes,
            calls: AtomicUsize::new(0),
            asked: std::sync::Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl Validator for Proving {
    async fn validate(&self, credential: &Credential) -> Validation {
        if let Ok(mut seen) = self.asked.lock() {
            seen.push(credential.clone());
        }
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        let index = call.min(self.outcomes.len() - 1);
        self.outcomes[index].clone()
    }
}

/// A validator whose outcome does not matter because the run never enters a
/// credential — the credential-free tests skip that step.
impl Proving {
    /// The credentials it was asked to prove, in the order it was asked.
    fn asked(&self) -> Vec<Credential> {
        self.asked
            .lock()
            .map(|seen| seen.clone())
            .unwrap_or_default()
    }
}

fn proving() -> Proving {
    Proving::giving(vec![Validation::Valid {
        observed: "unused".to_owned(),
    }])
}

/// A filesystem the tests script, so a run is driven without touching a real
/// disk. It answers the few calls the data-location probe makes; the rest it
/// is never asked, and stubs plainly.
struct ProbeFs {
    /// Whether `canonicalize` finds the path — false makes the probe walk up
    /// and, finding nothing, report the location untestable.
    reachable: bool,
    /// How many `canonicalize` calls fail before they start resolving, so a
    /// chosen leaf can be absent while a parent of it is found — the case where
    /// a link is proven on a parent rather than on the path itself.
    missing_leaves: AtomicUsize,
    /// Whether a probe file can be created there.
    writable: bool,
    /// How many link attempts fail before they start taking, so one chosen
    /// location can refuse to link and the next one accept it.
    failing_links: AtomicUsize,
    /// The file number the linked name reads back as; a value other than the
    /// probe's own models a link that could not be confirmed.
    confirmed_file: u64,
    /// The filesystem type reported, for naming why a link failed.
    kind: FsKind,
}

impl ProbeFs {
    /// A filesystem that links: reachable, writable, and confirming its links.
    fn links() -> Self {
        Self {
            reachable: true,
            missing_leaves: AtomicUsize::new(0),
            writable: true,
            failing_links: AtomicUsize::new(0),
            confirmed_file: 7,
            kind: FsKind::Linking("apfs".to_owned()),
        }
    }
}

#[async_trait]
impl FileSystem for ProbeFs {
    async fn canonicalize(&self, path: &Path) -> Result<PathBuf, Fault> {
        if !self.reachable {
            return Err(Fault::new("no such file or directory"));
        }
        let missing = self.missing_leaves.load(Ordering::SeqCst);
        if missing > 0 {
            self.missing_leaves.store(missing - 1, Ordering::SeqCst);
            return Err(Fault::new("no such file or directory"));
        }
        Ok(path.to_owned())
    }
    async fn touch(&self, _path: &Path) -> Result<(), Fault> {
        if self.writable {
            Ok(())
        } else {
            Err(Fault::new("permission denied"))
        }
    }
    async fn link(&self, _from: &Path, _to: &Path) -> Result<(), Fault> {
        let remaining = self.failing_links.load(Ordering::SeqCst);
        if remaining > 0 {
            self.failing_links.store(remaining - 1, Ordering::SeqCst);
            Err(Fault::new("operation not permitted"))
        } else {
            Ok(())
        }
    }
    async fn identify(&self, path: &Path) -> Result<Identity, Fault> {
        let file = if path.to_string_lossy().ends_with(".link") {
            self.confirmed_file
        } else {
            7
        };
        Ok(Identity { file, links: 2 })
    }
    async fn remove(&self, _path: &Path) {}
    async fn read(&self, _path: &Path) -> Option<String> {
        None
    }
    async fn write(&self, _path: &Path, _contents: &str) {}
    async fn ownership(&self, _path: &Path) -> Option<Ownership> {
        None
    }
}

#[async_trait]
impl Storage for ProbeFs {
    async fn describe(&self, _path: &Path) -> StorageFacts {
        StorageFacts {
            point: std::path::PathBuf::new(),
            kind: self.kind.clone(),
            removable: false,
            available: 0,
            total: 0,
        }
    }
}

/// What the scripted operator answers when warned a location cannot hardlink.
#[derive(Clone, Copy)]
enum Accept {
    /// Use the location in hand anyway.
    Location,
    /// Decline it and be asked for another.
    Elsewhere,
}

/// A prompt that answers from a fixed script, so a run is driven with no
/// terminal.
struct Scripted {
    protocols: Protocols,
    service_user: Option<(u32, u32)>,
    library: Library,
    household: bool,
    notifications: Appetite,
    autostart: bool,
    confirm: bool,
    /// The protocol choices each prerequisites checklist was derived from, in
    /// the order shown — so a test can prove the checklist reflects the answer.
    shown_prerequisites: std::cell::RefCell<Vec<Protocols>>,
    /// What the operator answers to "will a VPN carry it?", in turn — so a test
    /// can decline once and accept on the retry. Empty answers yes, which is the
    /// unremarkable case every other test wants.
    vpn: std::cell::RefCell<VecDeque<bool>>,
    /// What the operator answers to the unprotected-torrents warning, in turn.
    /// Empty answers yes, so a test that scripts a "no" to the VPN question and
    /// nothing here gets the accepted-the-exposure path.
    unprotected: std::cell::RefCell<VecDeque<bool>>,
    /// How many times the exposure was put to the operator — a warning that
    /// never appeared and one that appeared silently look the same otherwise.
    warned_unprotected: std::cell::Cell<usize>,
    /// The locations offered in turn; each `data_location` call takes the next.
    /// Every test scripts as many as its run will ask for.
    locations: std::cell::RefCell<VecDeque<PathBuf>>,
    /// What the operator answers to "use this location anyway?".
    accept: Accept,
    /// The storage warnings put to the operator, in order.
    warnings: std::cell::RefCell<Vec<StorageWarning>>,
    /// The locations reported as hardlinking, each with whether the result was
    /// inferred from a parent rather than proven on the location itself.
    hardlinked: std::cell::RefCell<Vec<(PathBuf, bool)>>,
    /// The indexer the operator enters, or none to leave it unset. Returned on
    /// every `credential` call, so a retry re-enters the same one.
    credential: Option<(String, String)>,
    /// The Usenet provider the operator enters, or none to leave it unset.
    provider: Option<ProviderEntry>,
    /// What the operator does with a credential the test could not prove.
    on_failure: CredentialChoice,
    /// The observed facts of credentials proven, and the outcomes of those that
    /// were not — so a test can see which path the run took.
    proven: std::cell::RefCell<Vec<String>>,
    failures: std::cell::RefCell<Vec<Validation>>,
}

impl Scripted {
    /// A script that answers every question with a workable choice and confirms.
    /// It enters no credential, the supported path for a run that is not about
    /// them — the credential tests set one.
    fn workable(data_location: PathBuf) -> Self {
        Self {
            protocols: Protocols::both(),
            service_user: Some((1000, 1000)),
            library: Library::JellyfinDocker,
            household: true,
            notifications: Appetite::default_appetite(),
            autostart: false,
            confirm: true,
            shown_prerequisites: std::cell::RefCell::new(Vec::new()),
            vpn: std::cell::RefCell::new(VecDeque::new()),
            unprotected: std::cell::RefCell::new(VecDeque::new()),
            warned_unprotected: std::cell::Cell::new(0),
            locations: std::cell::RefCell::new(VecDeque::from([data_location])),
            accept: Accept::Elsewhere,
            warnings: std::cell::RefCell::new(Vec::new()),
            hardlinked: std::cell::RefCell::new(Vec::new()),
            credential: None,
            provider: None,
            on_failure: CredentialChoice::Skip,
            proven: std::cell::RefCell::new(Vec::new()),
            failures: std::cell::RefCell::new(Vec::new()),
        }
    }
}

impl Prompt for Scripted {
    fn protocols(&self) -> Protocols {
        self.protocols
    }
    fn prerequisites(&self, map: &PrerequisiteMap) {
        self.shown_prerequisites.borrow_mut().push(map.protocols);
    }
    fn vpn(&self) -> bool {
        self.vpn.borrow_mut().pop_front().unwrap_or(true)
    }

    fn unprotected(&self) -> bool {
        self.warned_unprotected
            .set(self.warned_unprotected.get().saturating_add(1));
        self.unprotected.borrow_mut().pop_front().unwrap_or(true)
    }

    fn data_location(&self) -> PathBuf {
        self.locations.borrow_mut().pop_front().unwrap_or_default()
    }
    fn hardlinks(&self, path: &Path, inferred_from: Option<&Path>) {
        self.hardlinked
            .borrow_mut()
            .push((path.to_owned(), inferred_from.is_some()));
    }
    fn storage_warning(&self, _path: &Path, warning: &StorageWarning) -> bool {
        self.warnings.borrow_mut().push(warning.clone());
        matches!(self.accept, Accept::Location)
    }
    fn credential(&self) -> Option<(String, String)> {
        self.credential.clone()
    }
    fn credential_valid(&self, observed: &str) {
        self.proven.borrow_mut().push(observed.to_owned());
    }
    fn credential_failed(&self, outcome: &Validation) -> CredentialChoice {
        self.failures.borrow_mut().push(outcome.clone());
        self.on_failure
    }
    fn usenet_provider(&self) -> Option<ProviderEntry> {
        self.provider.clone()
    }
    fn service_user(&self) -> Option<(u32, u32)> {
        self.service_user
    }
    fn library(&self) -> Library {
        self.library
    }
    fn household(&self) -> bool {
        self.household
    }

    fn notifications(&self) -> Appetite {
        self.notifications
    }
    fn autostart(&self) -> bool {
        self.autostart
    }
    fn confirm(&self, _plan: &Plan) -> bool {
        self.confirm
    }
}

/// What an apply writes with: a real directory, the stack a test names, and a real
/// machine's randomness — which is what the key the journal's credentials are
/// sealed under is made from.
fn applying<'a>(paths: &'a Paths, stamp: &'a str) -> Applying<'a> {
    Applying {
        paths,
        source: external(),
        stamp,
        random: &A_MACHINE,
    }
}

/// The randomness a real machine supplies.
static A_MACHINE: lemonfiber_fixtures::ports::Chance =
    lemonfiber_fixtures::ports::Chance::cycling();

/// A scratch directory unique to this process and case, cleared first.
fn scratch(name: &str) -> lemonfiber_fixtures::scratch::Scratch {
    lemonfiber_fixtures::scratch::Scratch::unmade(name)
}

fn layout(dir: &Path) -> Paths {
    Paths::rooted(&dir.join("config"), &dir.join("data"))
}

/// A stack already on disk, so a run does not materialise one.
fn external() -> Source {
    Source::External(Path::new("/lemonfiber-not-a-real-stack"))
}

/// A prompt that enters the given indexer and answers a failed test the given
/// way — everything else the workable defaults.
fn entering(dir: &Path, on_failure: CredentialChoice) -> Scripted {
    Scripted {
        credential: Some(("http://indexer.test/api".to_owned(), "the-key".to_owned())),
        on_failure,
        ..Scripted::workable(dir.join("data-root"))
    }
}

mod credentials;
mod location;
mod protocols;
