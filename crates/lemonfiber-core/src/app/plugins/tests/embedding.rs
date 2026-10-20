//! Installing a plugin the bundle installs: read from the tree this build carries, with
//! nothing fetched, and recorded at the revision the bundle pins.

use super::fetching::{from_git, served, Serving};
use super::*;

/// The plugin the bundle installs.
const BUNDLED: &str = "jellyfin";

/// What the bundle pins for it.
fn pinned() -> Option<&'static lemonfiber_bundled::Pinned> {
    super::super::embedding::pinned(BUNDLED)
}

/// A context over the shipped stack with the bundled plugin's own service taken out.
fn moved(name: &str, serving: &Arc<Serving>) -> Ctx {
    let mut ctx = served(name, serving)
        .with_http(lemonfiber_fixtures::http::Fake::silent())
        .with_patience(std::time::Duration::ZERO);
    let fork = lemonfiber_fixtures::scratch::Scratch::named(&format!("{name}-fork")).kept();
    let _ = std::fs::remove_dir_all(&fork);
    lemonfiber_fixtures::stack::manifest_without(&fork, BUNDLED);
    ctx.stack = crate::stack::Source::External(Box::leak(fork.into_boxed_path()));
    ctx
}

#[tokio::test]
async fn a_plugin_the_bundle_installs_is_read_from_this_build_at_its_pin() {
    let serving = Arc::new(Serving::listing(""));
    let ctx = moved("embedded-installed", &serving);

    let would = report(from_git(&ctx, BUNDLED).await)
        .and_then(|one| one.install)
        .map(|install| install.would);

    assert_eq!(
        would
            .as_ref()
            .map(|one| (one.from.as_str(), one.revision.as_str(), one.tree.as_str())),
        pinned().map(|one| (one.origin, one.revision, one.first_party.tree))
    );
    assert!(serving.checkouts().is_empty(), "nothing was checked out");
    assert!(!serving
        .asked()
        .iter()
        .any(|one| one.contains(&"fetch".to_owned())));
    let checkouts = crate::app::targets::layout(&ctx)
        .map(|paths| paths.data_dir().join("checkouts"))
        .and_then(|at| std::fs::read_dir(at).ok())
        .map(Iterator::count);
    assert_eq!(checkouts, Some(0), "the tree written out is removed again");
}

#[tokio::test]
async fn a_plugin_the_bundle_installs_is_held_by_the_stack_service_it_replaces() {
    let serving = Arc::new(Serving::listing(""));
    let ctx = served("embedded-held", &serving);

    assert_eq!(refusal(from_git(&ctx, BUNDLED).await), "PLUGIN-28");
    assert_eq!(counted(reading(&ctx).await), Some(0));
}

#[tokio::test]
async fn a_machine_not_set_up_has_nowhere_to_write_the_tree() {
    let ctx = a_context().build();

    assert_eq!(refusal(from_git(&ctx, BUNDLED).await), "CONFIG-3");
}

#[tokio::test]
async fn a_tree_that_cannot_be_written_out_installs_nothing() {
    let serving = Arc::new(Serving::listing(""));
    let ctx = moved("embedded-unwritten", &serving);
    let data = crate::app::targets::layout(&ctx).map(|paths| paths.data_dir().to_path_buf());
    let _ = data.map(|data| {
        let _ = std::fs::create_dir_all(&data);
        std::fs::write(data.join("checkouts"), "in the way")
    });

    assert_eq!(refusal(from_git(&ctx, BUNDLED).await), "PLUGIN-16");
    assert_eq!(counted(reading(&ctx).await), Some(0));
}
