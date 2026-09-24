use super::{link_of, linked, note};
use crate::storage::Linked;
use crate::walkthrough::{Link, Reason, Step};

#[test]
fn a_copy_is_explained_where_the_operator_can_see_what_it_happened_to() {
    // The abstract explanation is a documentation page nobody reads; the same
    // explanation attached to a file that just landed is a thing understood.
    assert!(Link::Copied.consequence().contains("twice"));
    assert!(Link::Copied.remedy().is_some());
    assert!(Link::Hardlinked.remedy().is_none());
}

#[test]
fn imported_with_nowhere_to_play_it_is_its_own_ending() {
    // Not a broken pipeline: a form that does not include a media server, said as
    // that rather than as a failure of the import that plainly worked.
    assert_eq!(Reason::NoMediaServer.step(), Step::Scanning);
    assert!(Reason::NoMediaServer.said().contains("on disk"));
    assert_ne!(Reason::NoMediaServer.remedy(), Reason::NotVisible.remedy());
}

#[tokio::test]
async fn a_location_that_links_is_reported_as_linking_and_one_that_cannot_is_not() {
    // The empirical probe, not a guess from the filesystem's name — the same test
    // setup runs when it accepts a data location.
    let root = std::env::temp_dir().join(format!("lemonfiber-walk-link-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&root);
    let mut ctx = rooted_at(&root);
    assert_eq!(linked(&ctx).await, Some(Link::Hardlinked));

    // Nowhere to probe leaves the question unanswered rather than guessed: an operator
    // told "this was copied" when it was not would go and fix a correct volume.
    ctx.settings.env_file = None;
    assert_eq!(linked(&ctx).await, None);
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn a_location_that_cannot_be_written_says_nothing_about_linking() {
    let ctx = rooted_at(std::path::Path::new("/lemonfiber-not-a-directory"));
    assert_eq!(linked(&ctx).await, None);
}

/// A stack whose recorded data location is `root`, over the real filesystem — the
/// link probe is empirical, so a fake one would prove nothing about linking.
fn rooted_at(root: &std::path::Path) -> crate::app::Ctx {
    let dir = std::env::temp_dir().join(format!(
        "lemonfiber-walk-rooted-{}-{}",
        std::process::id(),
        root.display().to_string().len()
    ));
    let _ = std::fs::create_dir_all(&dir);
    let env = dir.join(".env");
    let _ = std::fs::write(
        &env,
        format!("{}={}\n", crate::config::DATA_ROOT_KEY, root.display()),
    );
    let mut ctx = super::super::fixtures::ctx_with(&super::super::fixtures::Fake::default());
    ctx.settings.env_file = Some(env);
    ctx.filesystem = std::sync::Arc::new(lemonfiber_adapters::Disk);
    ctx
}

#[test]
fn a_probe_that_could_not_run_is_a_different_answer_from_one_that_said_no() {
    // Told "this was copied" when it was not, an operator goes and fixes a volume
    // that is already correct.
    assert_eq!(link_of(&Linked::Yes { links: 2 }), Some(Link::Hardlinked));
    assert_eq!(link_of(&Linked::No), Some(Link::Copied));
    assert_eq!(link_of(&Linked::Unconfirmed), None);
    assert_eq!(
        link_of(&Linked::Unwritable {
            message: "read-only".to_owned()
        }),
        None
    );
}

#[test]
fn a_file_nothing_is_known_about_is_narrated_without_a_claim() {
    assert_eq!(note(Some(Link::Copied)), Link::Copied.consequence());
    assert_eq!(note(Some(Link::Hardlinked)), Link::Hardlinked.consequence());
    assert!(note(None).is_empty(), "no claim where there is no answer");
}
