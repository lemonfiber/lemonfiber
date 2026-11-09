use super::stack;
use crate::config::Settings;
use crate::model::StackSource;
use crate::stack::Source;
use crate::test_support::{a_context, a_password, env_at, every_part_has_run};

fn ctx(name: &str) -> crate::app::Ctx {
    a_context()
        .settings(Settings {
            env_file: Some(env_at(name, &a_password())),
            ..Settings::default()
        })
        .build()
}

fn shipped(name: &str) -> crate::app::Ctx {
    let mut ctx = ctx(name);
    ctx.stack = Source::Embedded(&crate::stack::BUNDLED);
    ctx.settings.stack_dir = ctx
        .settings
        .env_file
        .as_deref()
        .and_then(std::path::Path::parent)
        .map(|at| at.join("stack"));
    ctx
}

#[tokio::test]
async fn the_shipped_stack_is_said_with_where_it_is_and_the_words_that_start_it() {
    let ctx = shipped("underneath-bundled");
    every_part_has_run(&ctx);

    let read = stack(&ctx).await;

    let Ok(read) = read else {
        unreachable!("the read was refused: {read:?}");
    };
    assert_eq!(read.source, StackSource::Bundled);
    assert_eq!(
        Some(read.env.as_str()),
        ctx.settings
            .env_file
            .as_deref()
            .and_then(std::path::Path::to_str)
    );
    assert_eq!(
        Some(read.project.clone()),
        crate::app::project::directory(&ctx).map(|at| at.display().to_string())
    );
    assert_eq!(read.plain_up.first().map(String::as_str), Some("docker"));
    assert!(read.plain_up.iter().any(|word| word == "compose"));
    assert!(read
        .plain_up
        .windows(2)
        .any(|pair| pair.first().map(String::as_str) == Some("up")
            && pair.get(1).map(String::as_str) == Some("--detach")));
    assert_eq!(read.non_interactive.form, "lemonfiber <command>");
    assert!(read.exceptions.is_empty(), "{:?}", read.exceptions);
}

#[tokio::test]
async fn a_part_this_build_installs_first_is_named_as_what_the_words_alone_do_not_start() {
    let ctx = shipped("underneath-installing");

    let read = stack(&ctx).await;

    let exceptions = read.map(|read| read.exceptions).unwrap_or_default();
    assert!(!exceptions.is_empty(), "{exceptions:?}");
    assert!(exceptions
        .iter()
        .all(|one| !one.part.is_empty() && one.because.contains("installs it from this build")));
}

#[tokio::test]
async fn an_own_stack_that_cannot_be_read_refuses_and_is_never_called_the_shipped_one() {
    let mut ctx = ctx("underneath-unreadable");
    let missing = lemonfiber_fixtures::scratch::Scratch::named("underneath-no-stack").kept();
    let _ = std::fs::remove_dir_all(&missing);
    ctx.stack = Source::External(Box::leak(missing.into_boxed_path()));

    assert!(stack(&ctx).await.is_err());
}

#[tokio::test]
async fn a_machine_with_nothing_set_up_refuses_the_read() {
    let ctx = a_context().build();

    assert!(stack(&ctx).await.is_err());
}
