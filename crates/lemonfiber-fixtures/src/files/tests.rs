use super::*;

/// A filesystem given modes reports them for the paths it was given and for no
/// others, so a check about permissions can tell a widened file from an absent one.
#[tokio::test]
async fn a_mode_is_reported_for_a_path_it_was_given_and_for_no_other() {
    let files = Files::owning(vec![(".env", 0o644)]);

    let found = files
        .ownership(Path::new("/somewhere/lemonfiber/.env"))
        .await;
    assert_eq!(found.map(|one| one.mode), Some(0o644));
    assert!(files
        .ownership(Path::new("/somewhere/lemonfiber/admission.json"))
        .await
        .is_none());
}

/// The stub half of the contract, which the doc above states and nothing checked.
///
/// A test that reaches one of these has reached for a capability this fake was
/// never meant to stand in for, so each must refuse rather than answer plausibly.
/// Asserting that here is also what keeps them reachable: nothing else calls them,
/// and an unreached line is one the gate counts against a file it cannot see is
/// deliberate. A filesystem given no modes reports none for the same reason.
#[tokio::test]
async fn every_capability_but_reading_answers_as_unused() {
    let files = Files::anywhere("held");
    let path = Path::new("/srv/config.xml");

    assert_eq!(
        files.canonicalize(path).await.ok().as_deref(),
        Some(path),
        "a path canonicalises to itself: there is no filesystem to resolve it against"
    );
    assert!(files.touch(path).await.is_err());
    assert!(files.link(path, path).await.is_err());
    assert!(files.identify(path).await.is_err());
    assert!(files.ownership(path).await.is_none());

    // Neither answers anything, and neither may panic: a test that writes through
    // this fake is one whose subject is what it read back, not what it wrote.
    files.remove(path).await;
    files.write(path, "ignored").await;
    assert_eq!(
        files.read(path).await.as_deref(),
        Some("held"),
        "a write changes nothing: what is held is what the test scripted"
    );

    let facts = files.describe(path).await;
    assert!(
        matches!(facts.kind, FsKind::Linking(_)) && !facts.removable,
        "a filesystem that links and is not removable, so the storage probe reads \
         as the ordinary case rather than a warning a test did not ask for"
    );
}
