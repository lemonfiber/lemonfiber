use crate::app::Ctx;
use crate::config::env::quoted;
use crate::config::store;
use crate::plugin::{Holding, Variable};
use crate::test_support::{a_context, env_at};

fn machine(name: &str) -> Ctx {
    let env = env_at(&format!("files-{name}"), "pw");
    a_context()
        .settings(crate::config::Settings {
            env_file: Some(env.clone()),
            stack_dir: Some(env.with_file_name("data").join("stack")),
            ..crate::config::Settings::default()
        })
        .build()
}

fn variable(name: &str, holding: Holding) -> Variable {
    Variable {
        name: name.to_owned(),
        holding,
    }
}

fn kept(ctx: &Ctx, name: &str, value: &str) {
    let at = crate::settings::secrets_at(ctx).unwrap_or_default();
    let keeping = [crate::settings::Keeping {
        name,
        value,
        from: crate::settings::Source::Operator,
        at: 1,
    }];
    assert!(crate::settings::kept(&at, &keeping, |_| Ok(())).is_ok());
}

#[test]
fn every_value_reads_back_as_exactly_itself_so_compose_substitutes_nothing() {
    let ctx = machine("quoted");
    let awkward = r#"a$B${C}"d\e'f"#;
    kept(&ctx, "komga:token", awkward);
    kept(&ctx, "komga:plain", "plain-value");
    let text = crate::settings::text(
        &ctx,
        &[
            variable("TOKEN", Holding::Store("komga:token".to_owned())),
            variable("PLAIN", Holding::Store("komga:plain".to_owned())),
        ],
    )
    .unwrap_or_default();
    let read: Vec<(String, String)> = text
        .lines()
        .filter_map(|line| line.split_once('='))
        .map(|(name, raw)| (name.to_owned(), quoted::read(raw)))
        .collect();
    assert_eq!(
        read,
        [
            ("TOKEN".to_owned(), awkward.to_owned()),
            ("PLAIN".to_owned(), "plain-value".to_owned())
        ]
    );
    assert!(!text.replace("\\$", "").contains('$'), "{text}");
}

#[test]
fn a_secret_nothing_holds_or_that_may_not_cross_is_left_out() {
    let ctx = machine("unheld");
    let env = ctx.settings.env_file.clone().unwrap_or_default();
    assert!(store::set(&env, "SONARR_API_KEY", "abc").is_ok());
    let text = crate::settings::text(
        &ctx,
        &[
            variable("MISSING", Holding::Store("komga:missing".to_owned())),
            variable("WITHHELD", Holding::Withheld),
            variable("CROSSED", Holding::Crossing("SONARR_API_KEY".to_owned())),
        ],
    );
    assert_eq!(text.ok().as_deref(), Some("CROSSED=abc\n"));
    let nothing = crate::settings::text(
        &ctx,
        &[variable(
            "MISSING",
            Holding::Store("komga:missing".to_owned()),
        )],
    );
    assert_eq!(nothing.ok().as_deref(), Some(""));
}

#[test]
fn a_stack_credential_does_not_cross_to_a_plugin_nobody_vouched_for() {
    let ctx = machine("crossing");
    let manifest = ctx.stack.checked_manifest(ctx.today()).ok();
    let installed = crate::test_support::an_installed(
        "comics",
        vec![crate::test_support::a_placed(
            "comics-reader",
            &[],
            None,
            None,
        )],
    );
    let resolved =
        crate::settings::Resolved::of(&ctx, manifest.as_ref(), std::slice::from_ref(&installed));
    assert_eq!(
        resolved.crossing("comics", "sonarr"),
        Some(Holding::Withheld)
    );
    assert_eq!(resolved.crossing("comics", "comics-reader"), None);
    assert_eq!(resolved.crossing("comics", "nothing-here"), None);
}
