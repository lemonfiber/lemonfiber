use std::path::{Path, PathBuf};
use std::sync::Arc;

use lemonfiber_fixtures::ports::Chance;
use lemonfiber_fixtures::registered::Registered;
use lemonfiber_ports::registry::Tagged;

use crate::app::Ctx;
use crate::config::{Reaching, REACH_REGISTRY_KEY};
use crate::pass::{File, Key, Live, PINNED, SERVICE};
use crate::stack::RUNNING;

const DIGEST: &str = "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945";

/// A machine running the stack this build carries, asking `registry` what it publishes,
/// with a guard installed where `guarded`; answers where the project is.
fn machine(name: &str, registry: &Arc<Registered>, guarded: bool) -> (Ctx, PathBuf) {
    machine_with(name, registry, guarded, &[])
}

/// The same, with a plugin's service under each of `also` installed beside the guard.
fn machine_with(
    name: &str,
    registry: &Arc<Registered>,
    guarded: bool,
    also: &[&str],
) -> (Ctx, PathBuf) {
    let mut ctx = crate::test_support::a_context()
        .over(crate::test_support::embedded())
        .build()
        .with_registry(Arc::clone(registry) as Arc<dyn lemonfiber_ports::registry::Registry>);
    let env = crate::test_support::env_without_password(name);
    ctx.settings.stack_dir = Some(env.with_file_name("data").join("stack"));
    ctx.settings.env_file = Some(env);
    if guarded {
        let door = crate::test_support::a_placed("door", &["stream.guard"], None, Some(8080));
        let mut register = crate::plugin::Register::empty();
        assert!(register
            .record(crate::test_support::an_installed("door", vec![door]))
            .is_ok());
        for id in also {
            let service = crate::test_support::a_placed(id, &[], None, None);
            assert!(register
                .record(crate::test_support::an_installed("other", vec![service]))
                .is_ok());
        }
        assert!(
            crate::app::record::keep(crate::app::plugins::kept_at(&ctx).as_deref(), &register)
                .is_ok()
        );
    }
    let project = super::super::directory(&ctx).unwrap_or_default();
    (ctx, project)
}

async fn readied(ctx: &Ctx) -> Option<String> {
    let manifest = ctx.stack.checked_manifest(ctx.today()).ok()?;
    super::readied(ctx, &manifest).await
}

fn held(project: &Path, file: File) -> Option<String> {
    std::fs::read_to_string(project.join("config").join(SERVICE).join(file.name())).ok()
}

#[tokio::test]
async fn a_guard_pins_this_releases_image_and_makes_the_pass_services_files() {
    let registry = Registered::publishing(DIGEST);
    let (ctx, project) = machine("pass-pinned", &registry, true);

    assert_eq!(readied(&ctx).await, None);

    let pin = crate::pass::pin(RUNNING, DIGEST);
    assert_eq!(
        std::fs::read_to_string(project.join(PINNED)).ok(),
        Some(format!("{pin}\n"))
    );
    assert_eq!(super::super::pinned(&ctx), Some(pin));
    assert_eq!(registry.asked(), vec![Tagged::new(crate::IMAGE, RUNNING)]);
    assert!(held(&project, File::Key).is_some_and(|text| Key::read(&text).is_ok()));
    assert_eq!(
        held(&project, File::Live),
        Some(Live::of(Vec::new()).written())
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = std::fs::metadata(project.join("config").join(SERVICE).join(File::Key.name()))
            .map(|metadata| metadata.permissions().mode() & 0o777)
            .ok();
        assert_eq!(mode, Some(0o600));
    }
}

#[tokio::test]
async fn a_pin_and_a_key_already_held_are_kept() {
    let registry = Registered::publishing(DIGEST);
    let (ctx, project) = machine("pass-kept", &registry, true);
    assert_eq!(readied(&ctx).await, None);
    let key = held(&project, File::Key);

    assert_eq!(readied(&ctx).await, None);

    assert_eq!(registry.asked().len(), 1, "a held pin was asked for again");
    assert_eq!(
        held(&project, File::Key),
        key,
        "a key that reads was replaced"
    );
}

#[tokio::test]
async fn a_key_that_does_not_read_is_made_again() {
    let registry = Registered::publishing(DIGEST);
    let (ctx, project) = machine("pass-rekeyed", &registry, true);
    assert_eq!(readied(&ctx).await, None);
    let key = project.join("config").join(SERVICE).join(File::Key.name());
    assert!(std::fs::write(&key, "not a key\n").is_ok());

    assert_eq!(readied(&ctx).await, None);

    assert!(held(&project, File::Key).is_some_and(|text| Key::read(&text).is_ok()));
}

#[tokio::test]
async fn a_pin_of_another_release_is_asked_again() {
    let registry = Registered::publishing(DIGEST);
    let (ctx, project) = machine("pass-another", &registry, true);
    let _ = std::fs::create_dir_all(&project);
    assert!(std::fs::write(project.join(PINNED), crate::pass::pin("0.0.1", DIGEST)).is_ok());
    assert_eq!(super::super::pinned(&ctx), None);

    assert_eq!(readied(&ctx).await, None);

    assert_eq!(registry.asked().len(), 1);
    assert_eq!(
        super::super::pinned(&ctx),
        Some(crate::pass::pin(RUNNING, DIGEST))
    );
}

#[tokio::test]
async fn a_stack_with_no_guard_pins_and_makes_nothing() {
    let registry = Registered::publishing(DIGEST);
    let (ctx, project) = machine("pass-unguarded", &registry, false);

    assert_eq!(readied(&ctx).await, None);

    assert!(registry.asked().is_empty());
    assert!(!project.join(PINNED).exists());
    assert!(!project.join("config").join(SERVICE).exists());
}

#[tokio::test]
async fn a_release_with_no_published_image_leaves_the_pass_service_out_and_says_so() {
    let registry = Registered::unpublished();
    let (ctx, project) = machine("pass-unpublished", &registry, true);

    let said = readied(&ctx).await.unwrap_or_default();

    assert!(said.contains("no image of lemonfiber"), "{said}");
    assert!(said.contains(RUNNING), "{said}");
    assert!(!project.join(PINNED).exists());
    assert_eq!(held(&project, File::Key), None);
}

#[tokio::test]
async fn a_registry_out_of_reach_leaves_the_pass_service_out_and_says_why() {
    let registry = Registered::unreachable("connection refused");
    let (ctx, project) = machine("pass-unreachable", &registry, true);

    let said = readied(&ctx).await.unwrap_or_default();

    assert!(said.contains("could not be pinned"), "{said}");
    assert!(said.contains("connection refused"), "{said}");
    assert!(!project.join(PINNED).exists());
}

#[tokio::test]
async fn fetching_images_switched_off_asks_nothing_and_says_so() {
    let registry = Registered::publishing(DIGEST);
    let (mut ctx, project) = machine("pass-offline", &registry, true);
    ctx.settings.reaching = Reaching::without(REACH_REGISTRY_KEY);

    let said = readied(&ctx).await.unwrap_or_default();

    assert!(said.contains("switched off"), "{said}");
    assert!(registry.asked().is_empty());
    assert!(!project.join(PINNED).exists());
}

#[tokio::test]
async fn no_randomness_makes_no_key_and_says_so() {
    let registry = Registered::publishing(DIGEST);
    let (ctx, project) = machine("pass-unkeyed", &registry, true);
    let ctx = ctx.with_random(Arc::new(Chance::exactly(None)));

    let said = readied(&ctx).await.unwrap_or_default();

    assert!(said.contains("randomness"), "{said}");
    assert_eq!(held(&project, File::Key), None);
    assert!(
        !project.join(PINNED).exists(),
        "a pin was kept with no key beside it"
    );
}

#[tokio::test]
async fn a_service_holding_the_pass_services_id_leaves_it_out_and_makes_nothing() {
    let registry = Registered::publishing(DIGEST);
    let (ctx, project) = machine_with("pass-taken", &registry, true, &[SERVICE]);

    let said = readied(&ctx).await.unwrap_or_default();

    assert!(said.contains("named pass"), "{said}");
    assert!(registry.asked().is_empty());
    assert!(!project.join(PINNED).exists());
    assert_eq!(held(&project, File::Key), None);
}
