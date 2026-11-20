use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use lemonfiber_core::alert::Appetite;
use lemonfiber_core::app::setup::Asking;
use lemonfiber_core::app::Ctx;
use lemonfiber_core::config::paths::Paths;
use lemonfiber_core::config::{store, Protocols, Settings};
use lemonfiber_core::model::{SettingReport, SetupOutcome, StorageFinding};
use lemonfiber_core::platform::Environment;
use lemonfiber_core::ports::filesystem::FileSystem;
use lemonfiber_core::prerequisites::PrerequisiteMap;
use lemonfiber_core::stack::Source;
use lemonfiber_core::validate::{Credential, Validation, Validator};
use lemonfiber_core::wizard::Library;
use lemonfiber_fixtures::probing::Probing;

use super::{ended, standing, walk, walked_to, Ended, Walked};
use crate::setting_up::{CredentialChoice, Prompt, ProviderEntry};

/// A validator answering each credential with the next of its outcomes, the last
/// repeated once they run out, and keeping what it was asked to prove.
struct Answering {
    outcomes: Mutex<VecDeque<Validation>>,
    asked: Mutex<Vec<Credential>>,
}

impl Answering {
    fn with(outcomes: Vec<Validation>) -> Arc<Self> {
        Arc::new(Self {
            outcomes: Mutex::new(outcomes.into()),
            asked: Mutex::new(Vec::new()),
        })
    }

    fn asked(&self) -> usize {
        self.asked
            .lock()
            .map(|asked| asked.len())
            .unwrap_or_default()
    }
}

#[async_trait]
impl Validator for Answering {
    async fn validate(&self, credential: &Credential) -> Validation {
        if let Ok(mut asked) = self.asked.lock() {
            asked.push(credential.clone());
        }
        let Ok(mut outcomes) = self.outcomes.lock() else {
            return refused();
        };
        if outcomes.len() > 1 {
            outcomes.pop_front().unwrap_or_else(refused)
        } else {
            outcomes.front().cloned().unwrap_or_else(refused)
        }
    }
}

fn refused() -> Validation {
    Validation::Rejected {
        detail: "the key was refused".to_owned(),
    }
}

fn valid() -> Validation {
    Validation::Valid {
        observed: "answered a search".to_owned(),
    }
}

/// A prompt answering from a script and keeping what it was shown.
struct Scripted {
    protocols: Protocols,
    vpn: RefCell<VecDeque<bool>>,
    guarded: RefCell<usize>,
    unprotected: RefCell<VecDeque<bool>>,
    locations: RefCell<VecDeque<PathBuf>>,
    keeps_copying: bool,
    indexers: RefCell<VecDeque<Option<(String, String)>>>,
    providers: RefCell<VecDeque<ProviderEntry>>,
    on_failure: CredentialChoice,
    confirms: bool,
    prerequisites_shown: RefCell<usize>,
    warned: RefCell<Vec<StorageFinding>>,
    linked: RefCell<Vec<PathBuf>>,
    failed: RefCell<usize>,
    reviewed: RefCell<Vec<SettingReport>>,
}

impl Scripted {
    /// Usenet only, one location, no indexer, and the plan confirmed.
    fn workable(location: PathBuf) -> Self {
        Self {
            protocols: Protocols {
                usenet: true,
                torrent: false,
            },
            vpn: RefCell::new(VecDeque::new()),
            guarded: RefCell::new(0),
            unprotected: RefCell::new(VecDeque::new()),
            locations: RefCell::new(VecDeque::from([location])),
            keeps_copying: true,
            indexers: RefCell::new(VecDeque::new()),
            providers: RefCell::new(VecDeque::new()),
            on_failure: CredentialChoice::Proceed,
            confirms: true,
            prerequisites_shown: RefCell::new(0),
            warned: RefCell::new(Vec::new()),
            linked: RefCell::new(Vec::new()),
            failed: RefCell::new(0),
            reviewed: RefCell::new(Vec::new()),
        }
    }

    fn entering(location: PathBuf, on_failure: CredentialChoice) -> Self {
        Self {
            indexers: RefCell::new(VecDeque::from([
                Some(("http://indexer.test".to_owned(), "first-key".to_owned())),
                Some(("http://indexer.test".to_owned(), "second-key".to_owned())),
            ])),
            on_failure,
            ..Self::workable(location)
        }
    }

    /// Two Usenet logins to enter, one after the other.
    fn providing(location: PathBuf, on_failure: CredentialChoice) -> Self {
        let login = |user: &str| ProviderEntry {
            host: "news.lemonfiber.invalid".to_owned(),
            port: 563,
            user: user.to_owned(),
            pass: ["not", "a", "real", "one"].join("-"),
            tls: true,
        };
        Self {
            providers: RefCell::new(VecDeque::from([login("first"), login("second")])),
            on_failure,
            ..Self::workable(location)
        }
    }
}

impl Prompt for Scripted {
    fn protocols(&self) -> Protocols {
        self.protocols
    }
    fn prerequisites(&self, _map: &PrerequisiteMap) {
        *self.prerequisites_shown.borrow_mut() += 1;
    }
    fn vpn(&self) -> bool {
        self.vpn.borrow_mut().pop_front().unwrap_or(false)
    }
    fn guard(&self, _asking: &Asking<'_>) -> Option<String> {
        *self.guarded.borrow_mut() += 1;
        None
    }
    fn unprotected(&self) -> bool {
        self.unprotected.borrow_mut().pop_front().unwrap_or(true)
    }
    fn data_location(&self) -> PathBuf {
        let mut locations = self.locations.borrow_mut();
        if locations.len() > 1 {
            locations.pop_front().unwrap_or_default()
        } else {
            locations.front().cloned().unwrap_or_default()
        }
    }
    fn hardlinks(&self, path: &Path, _inferred_from: Option<&Path>) {
        self.linked.borrow_mut().push(path.to_owned());
    }
    fn storage_warning(&self, _path: &Path, found: &StorageFinding) -> bool {
        self.warned.borrow_mut().push(found.clone());
        self.keeps_copying
    }
    fn credential(&self) -> Option<(String, String)> {
        self.indexers.borrow_mut().pop_front().flatten()
    }
    fn credential_valid(&self, _observed: &str) {}
    fn credential_failed(&self, _outcome: &Validation) -> CredentialChoice {
        *self.failed.borrow_mut() += 1;
        self.on_failure
    }
    fn usenet_provider(&self) -> Option<ProviderEntry> {
        self.providers.borrow_mut().pop_front()
    }
    fn service_user(&self) -> Option<(u32, u32)> {
        None
    }
    fn library(&self) -> Library {
        Library::None
    }
    fn household(&self) -> bool {
        false
    }
    fn notifications(&self) -> Appetite {
        Appetite::default_appetite()
    }
    fn autostart(&self) -> bool {
        false
    }
    fn house(&self, proposed: &lemonfiber_core::wizard::House) -> lemonfiber_core::wizard::House {
        proposed.clone()
    }
    fn confirm(&self, plan: &[SettingReport]) -> bool {
        *self.reviewed.borrow_mut() = plan.to_vec();
        self.confirms
    }
}

/// A scratch install unique to this test.
fn scratch(name: &str) -> (lemonfiber_fixtures::scratch::Scratch, Paths) {
    let dir = lemonfiber_fixtures::scratch::Scratch::unmade(name);
    let paths = Paths::rooted(&dir.join("config"), &dir.join("data"));
    (dir, paths)
}

/// A context keeping its files in `paths`, proving credentials through `validator`
/// and testing locations on `filesystem`.
fn ctx(paths: &Paths, validator: Arc<dyn Validator>, filesystem: Arc<dyn FileSystem>) -> Ctx {
    lemonfiber_testing::a_context()
        .over(Source::External(Path::new("/lemonfiber-not-a-real-stack")))
        .environment(Environment::MacOs)
        .filesystem(filesystem)
        .settings(Settings {
            env_file: Some(paths.env_file()),
            stack_dir: Some(paths.stack()),
            ..Settings::default()
        })
        .build()
        .with_validator(validator)
}

/// A context on the real disk, proving every credential.
fn proving(paths: &Paths) -> Ctx {
    ctx(
        paths,
        Answering::with(vec![valid()]),
        Arc::new(lemonfiber_adapters::Disk),
    )
}

/// Walk setup from where it stands, through `prompt`.
async fn walked(ctx: &Ctx, prompt: &Scripted) -> Option<Walked> {
    let report = standing(ctx).await.ok().flatten()?;
    walk(ctx, prompt, report).await.ok()
}

/// What the walk's applied settings hold for `key`.
fn written(paths: &Paths, key: &str) -> Option<String> {
    store::read(&paths.env_file())
        .ok()
        .and_then(|file| file.get(key).map(str::to_owned))
}

/// What the plan reviewed holds for `key`.
fn planned(prompt: &Scripted, key: &str) -> Option<String> {
    prompt
        .reviewed
        .borrow()
        .iter()
        .find(|setting| setting.key == key)
        .map(|setting| setting.value.clone())
}

#[tokio::test]
async fn a_walk_put_to_its_end_applies_what_was_answered() {
    let (dir, paths) = scratch("walk-applied");
    let ctx = proving(&paths);
    let prompt = Scripted::workable(dir.join("media"));

    let walked = walked(&ctx, &prompt).await;

    assert_eq!(walked, Some(Walked::Applied));
    assert!(paths.env_file().exists());
    assert_eq!(*prompt.prerequisites_shown.borrow(), 1);
    assert_eq!(*prompt.linked.borrow(), vec![dir.join("media")]);
}

#[tokio::test]
async fn a_plan_declined_at_the_review_writes_nothing() {
    let (dir, paths) = scratch("walk-declined");
    let ctx = proving(&paths);
    let prompt = Scripted {
        confirms: false,
        ..Scripted::workable(dir.join("media"))
    };

    let walked = walked(&ctx, &prompt).await;

    assert_eq!(walked, Some(Walked::Abandoned));
    assert!(!paths.env_file().exists());
    assert!(!prompt.reviewed.borrow().is_empty());
}

#[tokio::test]
async fn a_location_that_copies_and_is_declined_is_asked_for_again() {
    let (dir, paths) = scratch("walk-copies");
    let copies = dir.join("copies");
    let links = dir.join("links");
    let _ = std::fs::create_dir_all(&copies);
    let _ = std::fs::create_dir_all(&links);
    let ctx = ctx(
        &paths,
        Answering::with(vec![valid()]),
        Arc::new(Probing {
            failing_links: AtomicUsize::new(1),
            ..Probing::linking()
        }),
    );
    let prompt = Scripted {
        locations: RefCell::new(VecDeque::from([copies, links.clone()])),
        keeps_copying: false,
        ..Scripted::workable(PathBuf::new())
    };

    let walked = walked(&ctx, &prompt).await;

    assert_eq!(walked, Some(Walked::Applied));
    assert!(matches!(
        prompt.warned.borrow().as_slice(),
        [StorageFinding::CopyOnly { .. }]
    ));
    assert_eq!(*prompt.linked.borrow(), vec![links.clone()]);
    assert_eq!(
        planned(&prompt, "DATA_ROOT"),
        Some(links.display().to_string())
    );
}

#[tokio::test]
async fn a_location_that_copies_and_is_kept_is_used_as_it_is() {
    let (dir, paths) = scratch("walk-copies-kept");
    let copies = dir.join("copies");
    let _ = std::fs::create_dir_all(&copies);
    let ctx = ctx(
        &paths,
        Answering::with(vec![valid()]),
        Arc::new(Probing {
            failing_links: AtomicUsize::new(usize::MAX),
            ..Probing::linking()
        }),
    );
    let prompt = Scripted::workable(copies.clone());

    let walked = walked(&ctx, &prompt).await;

    assert_eq!(walked, Some(Walked::Applied));
    assert_eq!(prompt.warned.borrow().len(), 1);
    assert_eq!(
        planned(&prompt, "DATA_ROOT"),
        Some(copies.display().to_string())
    );
}

#[tokio::test]
async fn a_credential_that_fails_is_entered_again_where_the_operator_retries() {
    let (dir, paths) = scratch("walk-retry");
    let validator = Answering::with(vec![refused(), valid()]);
    let ctx = ctx(
        &paths,
        validator.clone(),
        Arc::new(lemonfiber_adapters::Disk),
    );
    let prompt = Scripted::entering(dir.join("media"), CredentialChoice::Retry);

    let walked = walked(&ctx, &prompt).await;

    assert_eq!(walked, Some(Walked::Applied));
    assert_eq!(validator.asked(), 2);
    assert_eq!(*prompt.failed.borrow(), 1);
    assert_eq!(
        written(&paths, "INDEXER_URL").as_deref(),
        Some("http://indexer.test")
    );
}

#[tokio::test]
async fn a_credential_that_fails_is_left_unset_where_the_operator_skips_it() {
    let (dir, paths) = scratch("walk-skip");
    let validator = Answering::with(vec![refused()]);
    let ctx = ctx(
        &paths,
        validator.clone(),
        Arc::new(lemonfiber_adapters::Disk),
    );
    let prompt = Scripted::entering(dir.join("media"), CredentialChoice::Skip);

    let walked = walked(&ctx, &prompt).await;

    assert_eq!(walked, Some(Walked::Applied));
    assert_eq!(validator.asked(), 1);
    assert_eq!(planned(&prompt, "INDEXER_URL"), None);
}

#[tokio::test]
async fn a_credential_that_fails_is_kept_unproven_where_the_operator_goes_on() {
    let (dir, paths) = scratch("walk-proceed");
    let validator = Answering::with(vec![refused()]);
    let ctx = ctx(
        &paths,
        validator.clone(),
        Arc::new(lemonfiber_adapters::Disk),
    );
    let prompt = Scripted::entering(dir.join("media"), CredentialChoice::Proceed);

    let walked = walked(&ctx, &prompt).await;

    assert_eq!(walked, Some(Walked::Applied));
    assert_eq!(validator.asked(), 1);
    assert_eq!(
        planned(&prompt, "INDEXER_URL").as_deref(),
        Some("http://indexer.test")
    );
}

#[tokio::test]
async fn torrents_without_a_vpn_are_asked_again_until_the_exposure_is_accepted() {
    let (dir, paths) = scratch("walk-unprotected");
    let ctx = proving(&paths);
    let prompt = Scripted {
        protocols: Protocols {
            usenet: false,
            torrent: true,
        },
        vpn: RefCell::new(VecDeque::from([false, false])),
        unprotected: RefCell::new(VecDeque::from([false, true])),
        ..Scripted::workable(dir.join("media"))
    };

    let walked = walked(&ctx, &prompt).await;

    assert_eq!(walked, Some(Walked::Applied));
    assert!(prompt.vpn.borrow().is_empty() && prompt.unprotected.borrow().is_empty());
}

#[tokio::test]
async fn torrents_given_a_vpn_are_put_what_its_guard_asks() {
    let (dir, paths) = scratch("walk-guarded");
    let mut ctx = proving(&paths);
    ctx.stack = lemonfiber_testing::repository_stack();
    let prompt = Scripted {
        protocols: Protocols {
            usenet: false,
            torrent: true,
        },
        vpn: RefCell::new(VecDeque::from([true, false])),
        unprotected: RefCell::new(VecDeque::from([true])),
        ..Scripted::workable(dir.join("media"))
    };

    let _walked = walked(&ctx, &prompt).await;

    assert!(*prompt.guarded.borrow() > 0);
}

#[tokio::test]
async fn a_usenet_only_walk_is_never_asked_about_a_vpn() {
    let (dir, paths) = scratch("walk-usenet-only");
    let ctx = proving(&paths);
    let prompt = Scripted {
        vpn: RefCell::new(VecDeque::from([true])),
        unprotected: RefCell::new(VecDeque::from([true])),
        ..Scripted::workable(dir.join("media"))
    };

    let walked = walked(&ctx, &prompt).await;

    assert_eq!(walked, Some(Walked::Applied));
    assert_eq!(prompt.vpn.borrow().len(), 1);
    assert_eq!(prompt.unprotected.borrow().len(), 1);
}

#[tokio::test]
async fn setup_by_flags_on_linux_with_a_provider_it_cannot_prove_is_applied() {
    let (dir, paths) = scratch("walk-by-flags");
    let mut ctx = ctx(
        &paths,
        Answering::with(vec![refused()]),
        Arc::new(lemonfiber_adapters::Disk),
    );
    ctx.environment = Environment::LinuxNative;
    let given = crate::cli::RawSetup {
        yes: true,
        protocols: Some("usenet".to_owned()),
        data_location: Some(dir.join("media")),
        library: Some("docker".to_owned()),
        vpn: Some(false),
        household: Some(true),
        autostart: Some(false),
        usenet_host: Some("news.lemonfiber.invalid".to_owned()),
        usenet_port: Some(563),
        usenet_user: Some("arm".to_owned()),
        usenet_pass: Some("arm-password".to_owned()),
        usenet_tls: Some(true),
        ..crate::cli::RawSetup::default()
    };
    let flags = crate::setting_up::SetupFlags::parse(given);
    assert!(flags.is_ok());
    let Ok(flags) = flags else {
        return;
    };

    let concluded = crate::setting_up::by_flags(&ctx, &paths, flags).await;

    let after = standing(&ctx).await.ok().flatten();
    assert!(
        concluded.is_ok(),
        "{concluded:?} {:?}",
        after.map(|report| (report.at, report.unanswered, report.phase))
    );
}

#[tokio::test]
async fn a_login_that_fails_is_entered_again_where_the_operator_retries() {
    let (dir, paths) = scratch("walk-provider-retry");
    let validator = Answering::with(vec![refused(), valid()]);
    let ctx = ctx(
        &paths,
        validator.clone(),
        Arc::new(lemonfiber_adapters::Disk),
    );
    let prompt = Scripted::providing(dir.join("media"), CredentialChoice::Retry);

    let walked = walked(&ctx, &prompt).await;

    assert_eq!(walked, Some(Walked::Applied));
    assert_eq!(validator.asked(), 2);
    assert_eq!(*prompt.failed.borrow(), 1);
    assert!(prompt.providers.borrow().is_empty());
}

#[tokio::test]
async fn a_login_that_fails_is_left_unset_where_the_operator_skips_it() {
    let (dir, paths) = scratch("walk-provider-skip");
    let validator = Answering::with(vec![refused()]);
    let ctx = ctx(
        &paths,
        validator.clone(),
        Arc::new(lemonfiber_adapters::Disk),
    );
    let prompt = Scripted::providing(dir.join("media"), CredentialChoice::Skip);

    let walked = walked(&ctx, &prompt).await;

    assert_eq!(walked, Some(Walked::Applied));
    assert_eq!(validator.asked(), 1);
    assert_eq!(prompt.providers.borrow().len(), 1);
}

/// A step refused ends the walk with the refusal, and an answer that is not the walk's
/// ends it with nothing applied.
#[tokio::test]
async fn a_walk_stopped_short_says_how_it_ended() {
    let nowhere = lemonfiber_testing::a_context().build();
    let refused = standing(&nowhere).await;
    assert!(refused.is_err());
    let Err(problem) = refused else {
        return;
    };
    let read = lemonfiber_core::app::Outcome::Logs(lemonfiber_core::app::LogsReport { lines: 0 });

    assert!(matches!(walked_to(read), Err(Ended::Elsewhere)));
    assert!(ended(Err(Ended::from(problem))).is_err());
    assert_eq!(ended(Err(Ended::Elsewhere)).ok(), Some(Walked::Abandoned));
}

#[test]
fn a_walk_is_reported_as_the_way_it_ended() {
    assert_eq!(SetupOutcome::from(Walked::Applied), SetupOutcome::Applied);
    assert_eq!(
        SetupOutcome::from(Walked::Abandoned),
        SetupOutcome::Abandoned
    );
}
