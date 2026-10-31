use std::path::Path;
use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::scratch::Scratch;

use super::{identified, identity, media, served, serving};
use crate::app::Ctx;
use crate::plugin::Installed;
use crate::ports::docker::{Health, Lifecycle};
use crate::test_support::media as moved_in;
use crate::test_support::{a_context, a_placed, an_installed, Reporting};
use crate::wiring::{Chosen, Fillers};

const ADAPTER: &str = "plex-adapter";

/// A plugin whose adapter provides `provides` and speaks `speaks`.
fn adapting(provides: &[&str], speaks: &[&str]) -> Installed {
    let mut placed = a_placed(ADAPTER, provides, Some(8080));
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
    let mut ctx = a_context()
        .engine(Arc::new(engine))
        .build()
        .with_http(fake.clone());
    ctx.settings.env_file = Some(project.join(".env"));
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

/// A context over the repository's stack with its media server moved into its plugin,
/// whose adapter's key is written where `keyed`.
fn moved(tag: &str, keyed: bool) -> Ctx {
    let dir = Scratch::named(&format!("filled-{tag}")).kept();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    let mut ctx = a_context()
        .engine(Arc::new(moved_in::running()))
        .build()
        .with_http(Fake::always(Answer::reply(200, "[]")));
    ctx.settings.env_file = Some(dir.join(".env"));
    moved_in::moved_in(&ctx);
    if !keyed {
        if let Some(key) = moved_in::key_at(&ctx) {
            let _ = std::fs::remove_file(key);
        }
    }
    ctx
}

#[tokio::test]
async fn a_moved_server_is_asked_over_its_contracts_where_its_adapter_holds_its_key() {
    let ctx = moved("moved", true);
    let Ok(manifest) = ctx.stack.checked_manifest(ctx.today()) else {
        unreachable!("the repository's stack reads")
    };

    assert!(media(&ctx, &manifest).await.is_some());
    assert!(identity(&ctx, &manifest).await.is_some());
    assert!(serving(&ctx, &manifest).await.is_some());

    let unkeyed = moved("unkeyed", false);
    assert!(media(&unkeyed, &manifest).await.is_none());
    assert!(identity(&unkeyed, &manifest).await.is_none());
    assert!(serving(&unkeyed, &manifest).await.is_none());
}

#[tokio::test]
async fn an_answer_outside_the_contract_is_kept_and_the_plugin_is_asked_no_more() {
    let (project, ctx, _) = adapted("outside", true);
    let ctx = ctx.with_http(Fake::always(Answer::reply(200, "\"nobody\"")));
    let fillers = settled_on_the_adapter(
        &[adapting(&BOTH, &["identity.source@1", "media.serve@1"])],
        &project,
    );

    let answered = match identified(&ctx, &fillers).await {
        Some(identity) => identity.household().await.is_err(),
        None => false,
    };
    assert!(answered);
    assert!(
        crate::app::plugins::conformance::held(&ctx).is_ok_and(|kept| kept.iter().any(|one| one
            .plugin
            == "plex"
            && one.capability == "identity.source"
            && one.operation == "household"))
    );

    assert!(identified(&ctx, &fillers).await.is_none());
    assert!(served(&ctx, &fillers).await.is_some());
}

/// `ctx`'s moved media server claimed against an account outside the house: its adapter
/// speaking `owner.claim` and handed a code that lapses.
fn coded(ctx: &Ctx) {
    let register =
        crate::app::plugins::read(ctx).unwrap_or_else(|_| crate::plugin::Register::empty());
    let mut rewritten = crate::plugin::Register::empty();
    for one in register.installed() {
        let mut one = one.clone();
        if one
            .services
            .iter()
            .any(|placed| placed.service == moved_in::ADAPTER)
        {
            for placed in &mut one.services {
                if placed.service == moved_in::ADAPTER {
                    placed.speaks.push("owner.claim@1".to_owned());
                    placed
                        .upstream
                        .insert("claim_code".to_owned(), "claim-code".to_owned());
                }
            }
            one.declared.secrets.push(crate::plugin::Secret {
                id: "claim-code".to_owned(),
                of: "jellyfin".to_owned(),
                why: "The code the account shows".to_owned(),
                minted: false,
                serves: None,
                was: None,
                needs: lemonfiber_manifest::container::choices::Needs::new(),
                needed_to_start: false,
                lapses_s: Some(240),
            });
        }
        let _ = rewritten.record(one);
    }
    let _ = crate::app::record::keep(crate::app::plugins::kept_at(ctx).as_deref(), &rewritten);
}

#[tokio::test]
async fn members_reach_a_media_server_claimed_against_an_outside_account_only_once_it_is_ours() {
    let ctx = moved("kept-off", true);
    coded(&ctx);
    let Ok(manifest) = ctx.stack.checked_manifest(ctx.today()) else {
        unreachable!("the repository's stack reads")
    };
    assert!(media(&ctx, &manifest).await.is_none());
    assert!(serving(&ctx, &manifest).await.is_some());

    let fillers = super::fillers_here(&ctx, &manifest);
    let part = super::provider(
        &fillers,
        lemonfiber_contract::capabilities::media::serve::CAPABILITY,
    )
    .map(|served| fillers.fronted_by(served).unwrap_or(served).id.clone())
    .unwrap_or_default();
    let mut baseline = crate::baseline::Baseline::new();
    baseline.record(
        &part,
        crate::seed::run::owning::FIELD,
        "ours",
        "2026-10-31T00:00:00Z",
    );
    crate::seed::run::save_baseline(&ctx, &baseline);
    assert!(media(&ctx, &manifest).await.is_some());
}
