use std::path::Path;

use super::{declared, identity, refusal};
use crate::config::env::EnvFile;
use crate::test_support::a_context;

/// The stack lemonfiber ships, compiled in.
static SHIPPED: include_dir::Dir<'_> =
    include_dir::include_dir!("$CARGO_MANIFEST_DIR/../../assets/media-stack");

/// A stack compiled in that declares nothing.
static STACKLET: include_dir::Dir<'_> =
    include_dir::include_dir!("$CARGO_MANIFEST_DIR/tests/fixtures/stacklet");

/// A context whose settings file sits in `config`, with its data in `data`.
fn ctx_in(config: &Path, data: &Path) -> crate::app::Ctx {
    a_context()
        .settings(crate::config::Settings {
            env_file: Some(config.join(".env")),
            stack_dir: Some(data.join("stack")),
            home: Some(Path::new("/home/op").to_path_buf()),
            ..crate::config::Settings::default()
        })
        .build()
}

/// A name nothing reads is refused, and one lemonfiber, the stack or the file already
/// reads is weighed as it always was.
#[tokio::test]
async fn a_name_nothing_reads_is_refused() {
    let ctx = ctx_in(Path::new("/home/op/.config/lemonfiber"), Path::new("/data"));
    let held = EnvFile::parse("SONARR_API_KEY=0123\n");

    for refused in ["COMPOSE_FILE", "DOCKER_HOST", "NOTHING_READS_THIS"] {
        assert!(
            refusal(&ctx, &held, refused, "x")
                .await
                .is_some_and(|why| why.contains(refused)),
            "{refused} is refused"
        );
    }
    for known in ["LEMONFIBER_EXPLANATIONS", "TZ", "SONARR_API_KEY"] {
        assert_eq!(
            refusal(&ctx, &held, known, "x").await,
            None,
            "{known} is known"
        );
    }
}

/// The settings a stack declares are read from its own file, embedded or on disk; a
/// stack with no such file declares nothing.
#[tokio::test]
async fn the_settings_a_stack_declares_are_read_from_its_own_file() {
    let embedded = declared(crate::test_support::stack()).await;
    assert!(
        embedded.iter().any(|key| key == "DATA_ROOT"),
        "{embedded:?}"
    );

    let dir = lemonfiber_fixtures::scratch::Scratch::new("declared-external");
    let _ = std::fs::write(dir.join(".env.example"), "# ours\nOURS=1\n");
    let external: &'static Path = Box::leak(dir.to_path_buf().into_boxed_path());
    assert_eq!(
        declared(crate::stack::Source::External(external)).await,
        vec!["OURS"]
    );
    assert!(declared(crate::test_support::nowhere()).await.is_empty());

    let shipped = declared(crate::stack::Source::Embedded(&SHIPPED)).await;
    assert!(shipped.iter().any(|key| key == "DATA_ROOT"), "{shipped:?}");
    assert!(declared(crate::stack::Source::Embedded(&STACKLET))
        .await
        .is_empty());
}

/// The user and group the services run as are numbers, and never root's.
#[test]
fn a_service_user_is_a_number_and_never_root() {
    assert_eq!(identity("PUID", "1000"), None);
    assert!(identity("PUID", "0").is_some_and(|why| why.contains("PUID")));
    assert!(identity("PGID", "staff").is_some());
}

/// A Compose file layered over the stack is taken from lemonfiber's own directories
/// and nowhere else; an empty one clears it.
#[tokio::test]
async fn an_overlay_is_taken_from_lemonfibers_own_directories_only() {
    let ctx = ctx_in(
        Path::new("/home/op/.config/lemonfiber"),
        Path::new("/data/lf"),
    );
    let held = EnvFile::default();
    let key = crate::config::OVERLAY_KEY;

    assert_eq!(
        refusal(&ctx, &held, key, "/home/op/.config/lemonfiber/beside.yml").await,
        None
    );
    assert_eq!(refusal(&ctx, &held, key, "/data/lf/beside.yml").await, None);
    assert_eq!(refusal(&ctx, &held, key, "").await, None);
    for refused in [
        "/tmp/evil.yml",
        "beside.yml",
        "/home/op/.config/lemonfiber/../evil.yml",
    ] {
        assert!(
            refusal(&ctx, &held, key, refused).await.is_some(),
            "{refused}"
        );
    }
}

/// What reaches the host is checked through the one entry point, setting by setting.
#[tokio::test]
async fn what_reaches_the_host_is_checked_by_name() {
    let ctx = ctx_in(Path::new("/home/op/.config/lemonfiber"), Path::new("/data"));
    let held = EnvFile::default();

    assert!(refusal(&ctx, &held, crate::config::DATA_ROOT_KEY, "/")
        .await
        .is_some());
    assert!(refusal(&ctx, &held, crate::config::PGID_KEY, "0")
        .await
        .is_some());
    assert_eq!(
        refusal(&ctx, &held, crate::config::DATA_ROOT_KEY, "/srv/media").await,
        None
    );
}

/// A name only the stack on disk declares is known: its declaration is read without
/// holding the runtime, and the change is weighed rather than refused.
#[tokio::test]
async fn a_name_only_the_stack_on_disk_declares_is_weighed() {
    let dir = lemonfiber_fixtures::scratch::Scratch::new("declared-on-disk");
    let _ = std::fs::write(dir.join(".env.example"), "STACK_ONLY=1\n");
    let external: &'static Path = Box::leak(dir.to_path_buf().into_boxed_path());
    let ctx = a_context()
        .over(crate::stack::Source::External(external))
        .build();
    let held = EnvFile::default();

    assert_eq!(refusal(&ctx, &held, "STACK_ONLY", "2").await, None);
    assert!(refusal(&ctx, &held, "NOT_DECLARED", "2").await.is_some());
}

/// The key a refusal tells the operator to record the household's address under is one
/// `config set` accepts, so the remedy is a command that works.
#[tokio::test]
async fn the_household_address_a_remedy_names_is_a_key_config_set_accepts() {
    let ctx = ctx_in(Path::new("/home/op/.config/lemonfiber"), Path::new("/data"));
    assert_eq!(
        refusal(
            &ctx,
            &EnvFile::parse(""),
            crate::config::HOUSEHOLD_HOST_KEY,
            "192.168.1.20"
        )
        .await,
        None
    );
}
