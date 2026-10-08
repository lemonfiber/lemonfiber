//! Following a plugin's install recipes: what is asked for, what runs, what is kept,
//! and what a recipe that does not hold ends the act with.

use std::sync::Mutex;

use super::*;
use crate::error::Diagnose as _;
use crate::error::{Came, Problem};
use crate::plugin::running::{Ran, StepRan};
use crate::ports::asking::{Asking, Nobody};

/// The proving manifest, with a captured secret declared and an install recipe that
/// claims the library with a code the operator types, capturing the token it answers
/// with.
fn reciped() -> String {
    with_recipe(PROVING)
}

/// This manifest, with that secret and that recipe on the end.
fn with_recipe(manifest: &str) -> String {
    format!("{manifest}{RECIPE}")
}

/// The secret and the recipe [`reciped`] adds.
const RECIPE: &str = r#"
[[secret]]
id  = "token"
of  = "komga"
why = "Signs the library in"

[[recipe]]
id    = "adopt"
title = "Adopt the library"
why   = "To hand it over"

[[recipe.input]]
name   = "claim"
origin = "operator"
ask    = "The claim code Komga shows"
secret = true

[[recipe.step]]
id      = "claim"
call    = { method = "POST", to = "komga", path = "/api/claim?code={{claim}}" }
expect  = { status = 200 }
capture = [{ name = "token", from = "token", origin = "stack-service" }]

[[recipe.pair]]
value = "claim"
to    = "komga"

[requires]
capabilities = ["recipe.run"]
"#;

/// Somebody at a keyboard answering every question with one answer, noting each
/// question and whether it was asked as a secret.
struct Typing {
    /// What is typed for every question.
    answer: &'static str,
    /// Every question, and whether it was asked without showing what is typed.
    asked: Mutex<Vec<(String, bool)>>,
}

impl Typing {
    fn answering(answer: &'static str) -> Arc<Self> {
        Arc::new(Self {
            answer,
            asked: Mutex::new(Vec::new()),
        })
    }

    fn asked(&self) -> Vec<(String, bool)> {
        self.asked
            .lock()
            .map(|asked| asked.clone())
            .unwrap_or_default()
    }
}

impl Asking for Typing {
    fn present(&self) -> bool {
        true
    }

    fn ask(&self, question: &str) -> String {
        if let Ok(mut asked) = self.asked.lock() {
            asked.push((question.to_owned(), false));
        }
        self.answer.to_owned()
    }

    fn secret(&self, question: &str) -> String {
        if let Ok(mut asked) = self.asked.lock() {
            asked.push((question.to_owned(), true));
        }
        self.answer.to_owned()
    }
}

/// A transport answering the proof, and the claim with this status and body.
fn claiming(status: u16, body: &'static str) -> Arc<dyn Http> {
    Fake::by_path(vec![
        (
            "/api/v1/libraries",
            lemonfiber_fixtures::http::Answer::reply(200, "[]"),
        ),
        (
            "/api/claim",
            lemonfiber_fixtures::http::Answer::reply(status, body),
        ),
    ])
}

/// What installing the reciped manifest came to, with these inputs given.
async fn installed_with(
    ctx: &Ctx,
    name: &str,
    given: &[(&str, &str)],
) -> Result<Installs, Box<Problem>> {
    let source = crate::plugin::Source::Path(source(name, &reciped()));
    let asked = Asked::Install {
        source: source.clone(),
        consent: super::super::Consent::default(),
    };
    let reading = plugins(ctx, &asked).await?;
    let consent = super::super::Consent {
        agreement: reading.agreement,
        approved: Vec::new(),
        inputs: super::super::Inputs::of(
            given
                .iter()
                .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
                .collect(),
        ),
    };
    plugins(ctx, &Asked::Install { source, consent }).await
}

/// The whole of an install recipe: it runs once the install holds, carries what the
/// operator gave, is reported step by step, and what it captured is kept under the
/// plugin's name, journalled so putting the install back takes it out again.
#[tokio::test]
async fn an_install_recipe_runs_once_the_install_holds_and_what_it_captures_is_kept() {
    let runner = Arc::new(Recording::answering(Ok(spoke(""))));
    let http = claiming(200, r#"{"token":"t0k3n"}"#);
    let ctx = proving("followed", runner, http);
    let shown = report(installed_with(&ctx, "followed", &[("claim", "1234")]).await);
    let install = shown.and_then(|one| one.install);
    assert_eq!(install.as_ref().map(|one| one.recorded), Some(true));
    let ran: Vec<(String, bool, Vec<Came>)> = install
        .map(|one| {
            one.recipes_ran
                .into_iter()
                .map(|ran| {
                    (
                        ran.recipe,
                        ran.held,
                        ran.steps.into_iter().map(|step| step.came).collect(),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    assert_eq!(ran, [("adopt".to_owned(), true, vec![Came::Answered])]);
    let kept: Vec<(String, String)> = journalled(&ctx)
        .into_iter()
        .filter_map(|change| match change.kind {
            crate::journal::Kind::Set { key, current, .. } => Some((key, current)),
            _ => None,
        })
        .filter(|(key, _)| key.ends_with("_SECRET"))
        .collect();
    assert_eq!(kept.len(), 1, "{kept:?}");
    assert!(kept
        .iter()
        .all(|(key, value)| key.contains("TOKEN") && value == "t0k3n"));
    let env = ctx.settings.env_file.clone().unwrap_or_default();
    assert!(
        read(&env).contains("t0k3n"),
        "the token is kept beside the settings"
    );
}

/// At a keyboard, a value a recipe asks for and was not given is asked for in the words
/// the manifest writes, without it showing where the input is a secret.
#[tokio::test]
async fn at_a_keyboard_a_missing_value_is_asked_for_and_a_secret_unseen() {
    let runner = Arc::new(Recording::answering(Ok(spoke(""))));
    let typing = Typing::answering("1234");
    let ctx =
        proving("asked", runner, claiming(200, r#"{"token":"t"}"#)).with_asking(typing.clone());
    let install = report(installed_with(&ctx, "asked", &[]).await).and_then(|one| one.install);
    assert_eq!(install.map(|one| one.recorded), Some(true));
    assert_eq!(
        typing.asked(),
        [("The claim code Komga shows".to_owned(), true)]
    );
}

/// With nobody to ask, a missing value is refused before anything is written, naming
/// the input; so is a value no recipe asks for, and an act carrying one is refused
/// without anybody being asked for anything.
#[tokio::test]
async fn a_missing_or_unasked_value_is_refused_before_anything_is_written() {
    let runner = Arc::new(Recording::answering(Ok(spoke(""))));
    let ctx = proving("unmatched", runner.clone(), claiming(200, "{}"));
    let missing = installed_with(&ctx, "unmatched", &[]).await.err();
    assert_eq!(
        missing.as_ref().map(|one| one.code.to_string()),
        Some("PLUGIN-34".to_owned())
    );
    assert!(missing
        .and_then(|one| one.detail)
        .is_some_and(|detail| detail.contains("missing claim")));

    let typing = Typing::answering("1234");
    let asking =
        proving("unasked", runner.clone(), claiming(200, "{}")).with_asking(typing.clone());
    let unasked = installed_with(&asking, "unasked", &[("other", "s3cret")])
        .await
        .err();
    assert!(unasked.as_ref().is_some_and(|one| {
        one.code.to_string() == "PLUGIN-34"
            && one.detail.as_deref().is_some_and(|detail| {
                detail.contains("not asked for other")
                    && detail.contains("missing claim")
                    && !detail.contains("s3cret")
            })
    }));
    assert!(typing.asked().is_empty());
    assert!(!runner.ran("up"), "nothing was started");
    assert!(!record_of(&ctx).exists());
}

/// Nothing typed is nothing given.
#[test]
fn an_empty_answer_leaves_the_value_missing() {
    let manifest = lemonfiber_plugin::Manifest::from_toml(&reciped());
    let refused = manifest.map(|manifest| {
        super::super::following::gathered(
            "komga",
            &manifest,
            &super::super::Inputs::default(),
            Typing::answering("").as_ref(),
        )
        .err()
        .map(|one| one.code.to_string())
    });
    assert_eq!(refused.ok().flatten(), Some("PLUGIN-34".to_owned()));
    let nobody = lemonfiber_plugin::Manifest::from_toml(&reciped()).map(|manifest| {
        super::super::following::gathered(
            "komga",
            &manifest,
            &super::super::Inputs::default(),
            &Nobody,
        )
        .is_err()
    });
    assert_eq!(nobody.ok(), Some(true));
}

/// A recipe that does not hold puts the install back, nothing is recorded, and the act
/// ends with a problem naming the recipe and the step, carrying what every step came to.
#[tokio::test]
async fn a_recipe_that_does_not_hold_puts_the_install_back() {
    let runner = Arc::new(Recording::answering(Ok(spoke(""))));
    let ctx = proving("unheld", runner.clone(), claiming(500, "{}"));
    let problem = installed_with(&ctx, "unheld", &[("claim", "1234")])
        .await
        .err();
    assert_eq!(
        problem.as_ref().map(|one| one.code.to_string()),
        Some("PLUGIN-36".to_owned())
    );
    assert!(problem.as_ref().is_some_and(|one| {
        one.meaning.starts_with("The install was put back")
            && one.detail.as_deref().is_some_and(|detail| {
                detail.contains("recipe adopt: step claim") && !detail.contains("1234")
            })
            && one.steps.len() == 1
    }));
    assert!(runner.ran("rm"), "its container was taken off");
    assert!(!record_of(&ctx).exists(), "nothing was recorded");
    assert!(!read(&ctx.settings.env_file.clone().unwrap_or_default()).contains("_SECRET"));
}

/// One step as a failed run says it.
fn step(came: Came, landed: bool) -> StepRan {
    StepRan {
        step: "claim".to_owned(),
        to: "sonarr".to_owned(),
        came,
        status: None,
        tries: 1,
        landed,
        why: None,
    }
}

/// The code a failed run ends with is the one for how its step failed, and the detail
/// names what landed, or that nothing did.
#[test]
fn the_step_that_ended_a_run_names_its_code() {
    let ran = |steps: Vec<StepRan>| Ran {
        recipe: "adopt".to_owned(),
        held: false,
        why: Some("step claim failed".to_owned()),
        steps,
    };
    for (came, code) in [
        (Came::Refused, "PLUGIN-35"),
        (Came::Withheld, "PLUGIN-38"),
        (Came::Unexpected, "PLUGIN-36"),
        (Came::Unreachable, "PLUGIN-36"),
    ] {
        let problem = super::super::following::Unfollowed::Failed(vec![ran(vec![
            step(Came::Answered, true),
            step(Came::Skipped, false),
            step(came, false),
            step(Came::NotReached, false),
        ])])
        .problem("komga", "put back".to_owned());
        assert_eq!(problem.code.to_string(), code, "{came:?}");
        assert_eq!(problem.meaning, "put back");
        assert!(problem.detail.as_deref().is_some_and(|detail| detail
            .contains("already landed and not put back from here: adopt claim to sonarr")));
        assert_eq!(problem.steps.len(), 4);
    }
    let nowhere =
        super::super::following::Unfollowed::Failed(vec![ran(vec![step(Came::Unexpected, false)])])
            .problem("komga", String::new());
    assert!(nowhere
        .detail
        .is_some_and(|detail| detail.contains("nothing had landed anywhere")));
}

/// Recipes that held and whose captures could not be kept end with the record's code,
/// carrying why beneath it.
#[test]
fn what_could_not_be_kept_is_said_with_why_beneath_it() {
    let why = crate::config::store::Failure::Nowhere.problem();
    let code = why.code.to_string();
    let problem = super::super::following::Unfollowed::Unkept(Box::new(why))
        .problem("komga", "put back".to_owned());
    assert_eq!(problem.code.to_string(), "PLUGIN-8");
    assert_eq!(
        problem.cause.map(|cause| cause.code.to_string()),
        Some(code)
    );
}

/// What a recipe captured is taken away with the plugin.
#[tokio::test]
async fn what_a_recipe_captured_goes_when_the_plugin_is_removed() {
    let runner = Arc::new(Recording::answering(Ok(spoke(""))));
    let ctx = proving("taken", runner, claiming(200, r#"{"token":"t0k3n"}"#));
    let env = ctx.settings.env_file.clone().unwrap_or_default();
    assert!(report(installed_with(&ctx, "taken", &[("claim", "1234")]).await).is_some());
    assert!(read(&env).contains("t0k3n"));
    assert!(report(removing(&ctx, "komga").await).is_some());
    assert!(!read(&env).contains("t0k3n"), "{}", read(&env));
}

/// Only what a `[[secret]]` declares is kept.
#[test]
fn only_a_declared_capture_is_kept() {
    let captured = std::collections::BTreeMap::from([
        ("token".to_owned(), "t".to_owned()),
        ("stray".to_owned(), "s".to_owned()),
    ]);
    let kept: Vec<String> = lemonfiber_plugin::Manifest::from_toml(&reciped())
        .map(|manifest| {
            super::super::following::keeping("komga", &manifest, &captured)
                .into_iter()
                .map(|(setting, value)| format!("{setting}={value}"))
                .collect()
        })
        .unwrap_or_default();
    assert_eq!(kept.len(), 1, "{kept:?}");
    assert!(
        kept.iter().all(|one| one.ends_with("_SECRET=t")),
        "{kept:?}"
    );
}

/// An install asks only for what its install recipes need; a recipe run on demand is
/// asked for when it runs.
#[tokio::test]
async fn an_install_asks_nothing_a_demand_recipe_needs() {
    let demand = r#"
[[recipe]]
id    = "later"
title = "Later"
why   = "Run when asked"
on    = "demand"

[[recipe.input]]
name   = "later"
origin = "operator"
ask    = "Something for later"

[[recipe.step]]
id   = "later"
call = { method = "POST", to = "komga", path = "/api/later", body = "{{later}}" }

[[recipe.pair]]
value = "later"
to    = "komga"
"#;
    let manifest = lemonfiber_plugin::Manifest::from_toml(&format!("{}{demand}", reciped()));
    let given = super::super::Inputs::of([("claim".to_owned(), "1".to_owned())].into());
    let gathered = manifest.map(|manifest| {
        super::super::following::gathered("komga", &manifest, &given, &Nobody)
            .map(|inputs| inputs.values().len())
    });
    assert_eq!(gathered.ok().map(Result::ok), Some(Some(1)));
}

/// An input not marked secret is asked for as it is typed.
#[test]
fn an_input_that_is_no_secret_is_asked_for_in_plain_sight() {
    let typing = Typing::answering("1234");
    let plain = reciped().replace("secret = true\n", "");
    let gathered = lemonfiber_plugin::Manifest::from_toml(&plain).map(|manifest| {
        super::super::following::gathered(
            "komga",
            &manifest,
            &super::super::Inputs::default(),
            typing.as_ref(),
        )
        .is_ok()
    });
    assert_eq!(gathered.ok(), Some(true));
    assert_eq!(
        typing.asked(),
        [("The claim code Komga shows".to_owned(), false)]
    );
}

/// A value no recipe asks for is refused though nothing is missing, naming only it.
#[test]
fn a_value_nothing_asks_for_is_refused_though_nothing_is_missing() {
    let given = super::super::Inputs::of(
        [
            ("claim".to_owned(), "1".to_owned()),
            ("other".to_owned(), "s3cret".to_owned()),
        ]
        .into(),
    );
    let detail = lemonfiber_plugin::Manifest::from_toml(&reciped()).map(|manifest| {
        super::super::following::gathered("komga", &manifest, &given, &Nobody)
            .err()
            .and_then(|one| one.detail)
    });
    assert_eq!(
        detail.ok().flatten().as_deref(),
        Some("not asked for other")
    );
}

/// Where what a recipe captured cannot be written, the install goes back and says why.
#[tokio::test]
async fn where_a_capture_cannot_be_kept_the_install_goes_back() {
    let runner = Arc::new(Recording::answering(Ok(spoke(""))));
    let ctx = proving("unkept", runner.clone(), claiming(200, r#"{"token":"t"}"#));
    let env = ctx.settings.env_file.clone().unwrap_or_default();
    let locked = unrewritable(&env);
    let problem = installed_with(&ctx, "unkept", &[("claim", "1234")])
        .await
        .err();
    let _ = std::fs::remove_dir_all(staging_of(&env));
    assert!(locked);
    assert_eq!(
        problem.as_ref().map(|one| one.code.to_string()),
        Some("PLUGIN-8".to_owned())
    );
    assert!(problem.is_some_and(|one| one.cause.is_some()
        && one.summary.contains("what they captured could not be kept")
        && one.meaning.starts_with("The install was put back")));
    assert!(runner.ran("rm"), "its container was taken off");
    assert!(!record_of(&ctx).exists());
}

/// A credential the credential store holds for a service of the stack's is the key
/// lemonfiber publishes for it, carried to that service.
#[tokio::test]
async fn a_credential_input_is_the_key_lemonfiber_publishes_for_its_service() {
    let recipe = r#"
[[recipe]]
id    = "tell"
title = "Tell Sonarr"
why   = "To say hello"

[[recipe.input]]
name   = "key"
origin = "credential-store"
of     = "sonarr"

[[recipe.step]]
id     = "status"
call   = { method = "GET", to = "sonarr", path = "/api/v3/system/status", headers = { X-Api-Key = "{{key}}" } }
expect = { status = 200 }

[[recipe.pair]]
value = "key"
to    = "sonarr"

[requires]
capabilities = ["recipe.run"]
"#;
    let runner = Arc::new(Recording::answering(Ok(spoke(""))));
    let http = Fake::by_path(vec![
        (
            "/api/v1/libraries",
            lemonfiber_fixtures::http::Answer::reply(200, "[]"),
        ),
        (
            "/api/v3/system/status",
            lemonfiber_fixtures::http::Answer::reply(200, "{}"),
        ),
    ]);
    let ctx = proving("credential", runner, http.clone());
    let env = ctx.settings.env_file.clone().unwrap_or_default();
    let key = crate::config::for_service("sonarr", crate::config::API_KEY_SUFFIX);
    let _ = std::fs::write(&env, format!("{}{key}=k3y\n", read(&env)));
    let at = crate::plugin::Source::Path(source("credential", &format!("{PROVING}{recipe}")));
    let asked = Asked::Install {
        source: at,
        consent: super::super::Consent::default(),
    };
    let install = report(answered(&ctx, asked).await).and_then(|one| one.install);
    assert_eq!(install.map(|one| one.recorded), Some(true));
    let carried: Vec<String> = http
        .requests()
        .into_iter()
        .filter(|one| one.url.contains("/api/v3/system/status"))
        .flat_map(|one| one.headers)
        .filter(|(name, _)| name == "X-Api-Key")
        .map(|(_, value)| value)
        .collect();
    assert_eq!(carried, ["k3y"]);
}

/// An update runs its new version's install recipes once it holds, and reports them.
#[tokio::test]
async fn an_update_runs_the_new_versions_recipes_and_reports_them() {
    let runner = Arc::new(Recording::answering(Ok(spoke(""))));
    let ctx = proving("update-recipe", runner, claiming(200, r#"{"token":"t"}"#))
        .with_asking(Typing::answering("1234"));
    assert_eq!(
        counted(installing(&ctx, &source("update-recipe", PROVING)).await),
        Some(1)
    );
    let done = update(updating(&ctx, &source("update-recipe-next", &with_recipe(&next()))).await);
    assert_eq!(done.as_ref().map(|one| one.install.recorded), Some(true));
    assert_eq!(
        done.map(|one| one
            .install
            .recipes_ran
            .into_iter()
            .map(|ran| (ran.recipe, ran.held))
            .collect::<Vec<_>>()),
        Some(vec![("adopt".to_owned(), true)])
    );
    assert_eq!(on(&ctx).await.as_deref(), Some("1.3.0"));
}

/// A new version whose recipe does not hold comes off, the one it replaced goes back on,
/// and the act ends saying so.
#[tokio::test]
async fn an_update_whose_recipe_does_not_hold_puts_the_old_version_back() {
    let runner = Arc::new(Recording::answering(Ok(spoke(""))));
    let ctx = proving("update-unheld", runner, claiming(500, "{}"))
        .with_asking(Typing::answering("1234"));
    assert_eq!(
        counted(installing(&ctx, &source("update-unheld", PROVING)).await),
        Some(1)
    );
    let problem = updating(&ctx, &source("update-unheld-next", &with_recipe(&next())))
        .await
        .err();
    assert_eq!(
        problem.as_ref().map(|one| one.code.to_string()),
        Some("PLUGIN-36".to_owned())
    );
    assert_eq!(
        problem.map(|one| one.meaning),
        Some(
            "The update was put back, and komga 1.2.0 is on again, as the record still says."
                .to_owned()
        )
    );
    assert_eq!(on(&ctx).await.as_deref(), Some("1.2.0"));
    assert!(document(&ctx).contains(PINNED));
}

/// What an update put back says what the machine holds, read off the restore and off
/// whatever of the new version is still standing.
#[test]
fn what_an_update_put_back_says_what_the_machine_holds() {
    let restored = |placed: bool, running: bool| crate::plugin::Restored {
        version: "1.2.0".to_owned(),
        placed,
        running,
    };
    let clean = crate::app::putting_back::Reversal::default();
    let said = |back: &crate::app::putting_back::Reversal, placed, running| {
        super::super::updating::brought_back("komga", back, &restored(placed, running))
    };
    assert!(said(&clean, true, false).contains("with its containers not running"));
    assert!(said(&clean, false, true).contains("not everything it placed is on the machine again"));
    let mut left = crate::app::putting_back::Reversal::default();
    left.left.push(crate::app::putting_back::Left {
        target: "komga".to_owned(),
        because: "its container could not be taken off".to_owned(),
    });
    assert!(said(&left, true, true).ends_with(
        "Still standing from the new version: komga — its container could not be taken off."
    ));
}
