use crate::app::Ctx;
use crate::config::store;
use crate::journal::Kind;
use crate::test_support::{a_context, env_without_password};

fn machine(name: &str, dry_run: bool) -> Ctx {
    let env = env_without_password(&format!("renaming-{name}"));
    assert!(store::set(&env, "JELLYFIN_MODE", "native").is_ok());
    let ctx = a_context()
        .settings(crate::config::Settings {
            env_file: Some(env.clone()),
            stack_dir: Some(env.with_file_name("data").join("stack")),
            ..crate::config::Settings::default()
        })
        .build();
    if dry_run {
        ctx.rehearsing()
    } else {
        ctx
    }
}

fn written(ctx: &Ctx, key: &str) -> Option<String> {
    ctx.settings
        .env_file
        .as_deref()
        .and_then(|env| store::read(env).ok())
        .and_then(|file| file.written(key).map(str::to_owned))
}

fn renames(ctx: &Ctx) -> Vec<(String, String)> {
    crate::app::targets::layout(ctx)
        .and_then(|paths| crate::app::recover::journal_at(&paths.journal()).ok())
        .map(|journal| {
            journal
                .changes()
                .iter()
                .filter_map(|change| match &change.kind {
                    Kind::Renamed { from, to } => Some((from.clone(), to.clone())),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn a_value_under_an_earlier_key_is_moved_to_its_own_once_and_journalled() {
    let ctx = machine("once", false);

    assert!(super::renamed(&ctx).is_ok());
    assert!(super::renamed(&ctx).is_ok());

    assert_eq!(
        written(&ctx, "MEDIA_SERVER_MODE").as_deref(),
        Some("native")
    );
    assert_eq!(written(&ctx, "JELLYFIN_MODE"), None);
    assert_eq!(
        renames(&ctx),
        [("JELLYFIN_MODE".to_owned(), "MEDIA_SERVER_MODE".to_owned())]
    );
}

#[test]
fn a_key_that_holds_a_value_already_leaves_the_earlier_one_as_it_is() {
    let ctx = machine("held", false);
    let env = ctx.settings.env_file.clone().unwrap_or_default();
    assert!(store::set(&env, "MEDIA_SERVER_MODE", "docker").is_ok());

    assert!(super::renamed(&ctx).is_ok());

    assert_eq!(
        written(&ctx, "MEDIA_SERVER_MODE").as_deref(),
        Some("docker")
    );
    assert_eq!(written(&ctx, "JELLYFIN_MODE").as_deref(), Some("native"));
}

#[test]
fn a_rehearsal_moves_nothing_and_reads_the_earlier_key() {
    let ctx = machine("rehearsed", true);

    assert!(super::renamed(&ctx).is_ok());

    assert_eq!(written(&ctx, "JELLYFIN_MODE").as_deref(), Some("native"));
    assert_eq!(
        crate::app::targets::recorded_secret(&ctx, crate::config::MEDIA_SERVER_MODE_KEY).as_deref(),
        Some("native")
    );
    assert!(renames(&ctx).is_empty());
}

#[test]
fn a_machine_with_no_environment_file_moves_nothing() {
    let ctx = a_context().build();

    assert!(super::renamed(&ctx).is_ok());
}
