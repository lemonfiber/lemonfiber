use std::path::Path;
use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::scratch::Scratch;

use super::{identified, identity, media, served, serving};
use crate::app::Ctx;
use crate::plugin::Installed;
use crate::ports::docker::{Health, Lifecycle};
use crate::test_support::{a_context, a_password, a_placed, an_installed, Reporting};
use crate::wiring::{Chosen, Fillers};

const ADAPTER: &str = "plex-adapter";

/// A plugin whose adapter provides `provides` and speaks `speaks`.
fn adapting(provides: &[&str], speaks: &[&str]) -> Installed {
    let mut placed = a_placed(ADAPTER, provides, None, Some(8080));
    placed.speaks = speaks.iter().map(|one| (*one).to_owned()).collect();
    an_installed("plex", vec![placed])
}

/// The shipped stack written to `project`, with `installed` beside it and the identity
/// ask settled on the adapter.
fn settled_on_the_adapter(installed: &[Installed], project: &Path) -> Fillers {
    let chosen = Chosen::read(Some("identity.source=plex-adapter"));
    crate::test_support::stack()
        .manifest()
        .map(|manifest| Fillers::of(&manifest, installed, &chosen, Some(project)))
        .unwrap_or_default()
}

/// A project directory holding the adapter's key where `keyed`, and a context whose
/// engine publishes the adapter on loopback and whose transport answers every call with
/// nothing.
fn adapted(tag: &str, keyed: bool) -> (Scratch, Ctx, Arc<Fake>) {
    let project = Scratch::new(&format!("filled-{tag}"));
    if keyed {
        let at = crate::plugin::key_file(&project, ADAPTER);
        let _ = std::fs::create_dir_all(at.parent().unwrap_or(&at));
        let _ = std::fs::write(&at, "the-key");
    }
    let engine = Reporting::holding(&[ADAPTER], Lifecycle::Running, Health::Healthy)
        .publishing(&[(ADAPTER, "127.0.0.1", 8080)]);
    let fake = Fake::always(Answer::reply(200, "[]"));
    let ctx = a_context()
        .engine(Arc::new(engine))
        .build()
        .with_http(fake.clone());
    (project, ctx, fake)
}

const BOTH: [&str; 2] = ["identity.source", "media.serve"];

#[tokio::test]
async fn an_adapter_filling_the_identity_source_is_asked_over_both_its_contracts() {
    let (project, ctx, fake) = adapted("contracted", true);
    let fillers = settled_on_the_adapter(
        &[adapting(&BOTH, &["identity.source@1", "media.serve@1"])],
        &project,
    );

    let members = match identified(&ctx, &fillers).await {
        Some(identity) => identity.household().await.ok(),
        None => None,
    };
    assert_eq!(members.map(|members| members.len()), Some(0));
    assert_eq!(
        fake.request().map(|request| request.url),
        Some("http://127.0.0.1:8080/lemonfiber/identity.source/v1/household".to_owned())
    );

    let playing = match served(&ctx, &fillers).await {
        Some(serve) => serve.playing(None).await.ok(),
        None => None,
    };
    assert_eq!(playing.map(|playing| playing.len()), Some(0));
    assert_eq!(
        fake.request().map(|request| request.url),
        Some("http://127.0.0.1:8080/lemonfiber/media.serve/v1/playing".to_owned())
    );
}

#[tokio::test]
async fn a_capability_the_filler_does_not_provide_is_asked_of_nobody() {
    let (project, ctx, _) = adapted("unprovided", true);
    let fillers = settled_on_the_adapter(
        &[adapting(&["identity.source"], &["identity.source@1"])],
        &project,
    );

    assert!(identified(&ctx, &fillers).await.is_some());
    assert!(served(&ctx, &fillers).await.is_none());
}

#[tokio::test]
async fn an_adapter_that_cannot_be_reached_is_asked_nothing_and_nothing_else_is_asked() {
    let (project, ctx, fake) = adapted("unkeyed", false);
    let fillers = settled_on_the_adapter(
        &[adapting(&BOTH, &["identity.source@1", "media.serve@1"])],
        &project,
    );

    assert!(identified(&ctx, &fillers).await.is_none());
    assert!(served(&ctx, &fillers).await.is_none());
    assert!(fake.request().is_none());
}

#[tokio::test]
async fn a_filler_speaking_a_major_this_build_does_not_is_never_asked_as_the_bundled_server() {
    let (project, ctx, fake) = adapted("unspoken", true);
    let fillers = settled_on_the_adapter(
        &[adapting(&BOTH, &["identity.source@2", "media.serve@2"])],
        &project,
    );

    assert!(identified(&ctx, &fillers).await.is_none());
    assert!(served(&ctx, &fillers).await.is_none());
    assert!(fake.request().is_none());
}

/// A context over the repository's stack, with an env file of its own holding the
/// media server's administrator's password where one is given.
fn bundled(tag: &str, password: Option<&str>) -> Ctx {
    let dir = Scratch::named(&format!("filled-{tag}")).kept();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    let mut ctx = a_context().build();
    ctx.settings.env_file = Some(dir.join(".env"));
    if let Some(password) = password {
        let _ = crate::app::targets::record_secret(
            &ctx,
            crate::config::JELLYFIN_ADMIN_PASSWORD_KEY,
            password,
        );
    }
    ctx
}

#[tokio::test]
async fn the_bundled_server_is_asked_as_its_administrator_where_its_password_is_recorded() {
    let ctx = bundled("bundled", Some(&a_password()));
    let Ok(manifest) = ctx.stack.checked_manifest(ctx.today()) else {
        unreachable!("the repository's stack reads")
    };

    assert!(media(&ctx, &manifest).await.is_some());
    assert!(identity(&ctx, &manifest).await.is_some());
    assert!(serving(&ctx, &manifest).await.is_some());

    let unseeded = bundled("unseeded", None);
    assert!(media(&unseeded, &manifest).await.is_none());
    assert!(identity(&unseeded, &manifest).await.is_none());
    assert!(serving(&unseeded, &manifest).await.is_none());
}
