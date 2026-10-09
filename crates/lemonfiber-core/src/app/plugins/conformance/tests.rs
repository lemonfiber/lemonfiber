use lemonfiber_fixtures::scratch::Scratch;

use super::{cleared, fills, held, witness};
use crate::app::Ctx;
use crate::config::paths::NONCONFORMING;
use crate::test_support::a_context;

/// A context whose settings live in a directory of the test's own, and that directory.
fn keeping(tag: &str) -> (Ctx, std::path::PathBuf) {
    let dir = Scratch::named(&format!("conformance-{tag}")).kept();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    let mut ctx = a_context().build();
    ctx.settings.env_file = Some(dir.join(".env"));
    (ctx, dir)
}

#[test]
fn an_answer_outside_the_contract_keeps_the_plugin_from_filling_that_capability_alone() {
    let (ctx, _dir) = keeping("kept");
    assert!(fills(&ctx, "plex", "media.serve"));

    let told = witness(&ctx, "plex", "media.serve");
    told.nonconforming("playing", "418 is not an answer the contract declares");
    told.nonconforming("playing", "the answer did not read");
    told.nonconforming("holdings", "the answer is not text");

    let kept = held(&ctx).unwrap_or_default();
    assert_eq!(
        kept.iter()
            .map(|one| (one.operation.as_str(), one.why.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("playing", "the answer did not read"),
            ("holdings", "the answer is not text")
        ]
    );
    assert!(kept
        .iter()
        .all(|one| one.plugin == "plex" && !one.at.is_empty()));
    assert!(!fills(&ctx, "plex", "media.serve"));
    assert!(fills(&ctx, "plex", "identity.source"));
    assert!(fills(&ctx, "jellyfin", "media.serve"));
}

#[test]
fn clearing_a_plugin_takes_its_answers_and_leaves_the_others() {
    let (ctx, dir) = keeping("cleared");
    witness(&ctx, "plex", "media.serve").nonconforming("playing", "why");
    witness(&ctx, "emby", "identity.source").nonconforming("household", "why");

    assert!(cleared(&ctx, "plex").is_ok());
    assert!(fills(&ctx, "plex", "media.serve"));
    assert!(!fills(&ctx, "emby", "identity.source"));
    assert!(cleared(&ctx, "nobody").is_ok());

    assert!(cleared(&ctx, "emby").is_ok());
    assert!(held(&ctx).is_ok_and(|kept| kept.is_empty()));
    assert!(!dir.join(NONCONFORMING).exists());
}

#[test]
fn a_record_that_cannot_be_read_fills_nothing_and_is_never_written_over() {
    let (ctx, dir) = keeping("unreadable");
    let _ = std::fs::write(dir.join(NONCONFORMING), "not a record");

    assert!(held(&ctx).is_err());
    assert!(!fills(&ctx, "plex", "media.serve"));
    witness(&ctx, "plex", "media.serve").nonconforming("playing", "why");
    assert!(cleared(&ctx, "plex").is_err());
    assert_eq!(
        std::fs::read_to_string(dir.join(NONCONFORMING))
            .ok()
            .as_deref(),
        Some("not a record")
    );
}

#[test]
fn with_nowhere_to_keep_a_record_nothing_is_kept_and_everything_fills() {
    let mut ctx = a_context().build();
    ctx.settings.env_file = None;
    witness(&ctx, "plex", "media.serve").nonconforming("playing", "why");
    assert!(held(&ctx).is_ok_and(|kept| kept.is_empty()));
    assert!(fills(&ctx, "plex", "media.serve"));

    let (unwritable, dir) = keeping("unwritable");
    let _ = std::fs::create_dir_all(dir.join(format!("{NONCONFORMING}.new")));
    witness(&unwritable, "plex", "media.serve").nonconforming("playing", "why");
    assert!(held(&unwritable).is_ok_and(|kept| kept.is_empty()));
}
