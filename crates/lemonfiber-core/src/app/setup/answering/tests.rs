use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;

use super::{setting_up, SetupAction, ALREADY_SET_UP, NOTHING_TO_RECOVER};
use crate::alert::Appetite;
use crate::app::apply::NOT_REVIEWED;
use crate::app::setup::DOES_NOT_APPLY;
use crate::app::Ctx;
use crate::config::paths::Paths;
use crate::config::{
    store, Protocols, Settings, INDEXER_APIKEY_KEY, INDEXER_URL_KEY, PROVIDER_PASS_KEY,
};
use crate::error::Code;
use crate::journal::{Change, Kind};
use crate::model::WizardReport;
use crate::stack::Source;
use crate::test_support::a_context;
use crate::validate::{Credential, Validation, Validator};
use crate::wizard::{
    Answer, Choice, Indexer, Library, Phase, Progress, Provider, Step, Vpn, Wizard,
};

/// A scratch layout unique to this process and case, cleared first.
fn scratch(name: &str) -> Paths {
    let dir = std::env::temp_dir().join(format!("lemonfiber-walk-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    Paths::rooted(&dir.join("config"), &dir.join("data"))
}

/// A validator that answers what it was built with, whatever it is asked.
///
/// What this path has to be shown is that a credential records what the service
/// said rather than what the caller claimed, which needs the outcome and not
/// the service that produced one.
struct Saying(Validation);

#[async_trait]
impl Validator for Saying {
    async fn validate(&self, _credential: &Credential) -> Validation {
        self.0.clone()
    }
}

/// What a service that proved the credential answered.
fn proven() -> Arc<dyn Validator> {
    Arc::new(Saying(Validation::Valid {
        observed: "answered a search — 40 results".to_owned(),
    }))
}

/// A context keeping its files in that layout, on a platform where the
/// container user is never asked about.
///
/// The stack is one already on disk, so applying materialises nothing and what
/// is being driven is the walk rather than a stack being written out.
fn ctx(paths: &Paths) -> Ctx {
    proving(paths, proven())
}

/// The same, proving credentials through a given validator.
fn proving(paths: &Paths, validator: Arc<dyn Validator>) -> Ctx {
    a_context()
        .over(Source::External(Path::new("/lemonfiber-not-a-real-stack")))
        .settings(Settings {
            env_file: Some(paths.env_file()),
            stack_dir: Some(paths.stack()),
            ..Settings::default()
        })
        .build()
        .proving(validator)
}

/// Where a step of the walk left setup, or nothing where it refused.
async fn walked(ctx: &Ctx, action: SetupAction) -> Option<WizardReport> {
    setting_up(ctx, action).await.ok()
}

/// Which refusal a step of the walk met, or nothing where it met none.
async fn refused(ctx: &Ctx, action: SetupAction) -> Option<Code> {
    setting_up(ctx, action)
        .await
        .err()
        .map(|problem| problem.code)
}

/// A value that must not be printed back, assembled rather than written out so
/// no credential sits in this source.
fn withheld_value(word: &str) -> String {
    [word, "not", "real"].join("-")
}

/// The value a report's plan carries for a setting, or nothing where it holds
/// none — and nothing too where the step itself refused.
fn planned(report: Option<&WizardReport>, key: &str) -> Option<String> {
    report?
        .plan
        .iter()
        .find(|setting| setting.key == key)
        .map(|setting| setting.value.clone())
}

/// Every answer this platform asks for, in order.
fn all_of_them(root: &Path) -> [Answer; 9] {
    [
        Answer::Protocols(Protocols::both()),
        Answer::Vpn(Vpn::Carrying),
        Answer::DataLocation(root.to_path_buf()),
        Answer::Credentials(None),
        Answer::Provider(None),
        Answer::Library(Library::None),
        Answer::Household(false),
        Answer::Notifications(Appetite::ProblemsOnly),
        Answer::Autostart(false),
    ]
}

/// Answer every question, one request each, as a surface would.
async fn answer_everything(ctx: &Ctx, root: &Path) {
    for answer in all_of_them(root) {
        assert!(setting_up(ctx, SetupAction::Answer(answer)).await.is_ok());
    }
}

mod applying;
mod walking;
