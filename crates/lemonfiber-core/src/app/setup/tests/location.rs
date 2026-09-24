//! Whether the data location can hardlink, and what to do when it cannot.

use super::*;

#[tokio::test]
async fn an_answer_that_does_not_apply_here_stops_the_run() {
    let dir = scratch("rejected");
    let paths = layout(&dir);
    // Native Jellyfin buys nothing on native Linux, so a prompt that offers it
    // anyway is rejected rather than applied.
    let mut wizard = Wizard::new(Environment::LinuxNative);
    let prompt = Scripted {
        library: Library::JellyfinNative,
        ..Scripted::workable(dir.join("data-root"))
    };

    let stopped = run(
        &mut wizard,
        &prompt,
        &ProbeFs::links(),
        &proving(),
        &applying(&paths, "t"),
    )
    .await;

    assert!(matches!(stopped, Err(problem) if problem.code == super::super::DOES_NOT_APPLY));
    assert!(!paths.env_file().exists(), "nothing was applied");
}

/// A filesystem that cannot link, of a named type, so the copy-only warning
/// carries the reason.
fn cannot_link(kind: FsKind) -> ProbeFs {
    ProbeFs {
        failing_links: AtomicUsize::new(usize::MAX),
        kind,
        ..ProbeFs::links()
    }
}

#[tokio::test]
async fn a_location_that_cannot_hardlink_is_put_to_the_operator_who_may_use_it() {
    let dir = scratch("copy-only");
    let paths = layout(&dir);
    let mut wizard = Wizard::new(Environment::LinuxNative);
    // The operator, told the location copies and why, chooses to use it anyway.
    let prompt = Scripted {
        accept: Accept::Location,
        ..Scripted::workable(dir.join("data-root"))
    };

    let outcome = run(
        &mut wizard,
        &prompt,
        &cannot_link(FsKind::ExFat),
        &proving(),
        &applying(&paths, "t"),
    )
    .await;

    assert!(matches!(outcome, Ok(Outcome::Applied)));
    // The warning named the filesystem's own reason, so the operator weighed a
    // real fact rather than a bare "cannot hardlink".
    let warnings = prompt.warnings.borrow();
    assert!(matches!(
        warnings.as_slice(),
        [StorageWarning::CopyOnly { limitation: Some(reason) }] if reason.contains("exFAT")
    ));
}

#[tokio::test]
async fn a_copy_only_location_names_no_reason_where_its_type_explains_nothing() {
    let dir = scratch("copy-only-unnamed");
    let paths = layout(&dir);
    let mut wizard = Wizard::new(Environment::LinuxNative);
    // A filesystem that normally links but did not is a fault to report plainly,
    // not one to blame on its type.
    let prompt = Scripted {
        accept: Accept::Location,
        ..Scripted::workable(dir.join("data-root"))
    };

    let outcome = run(
        &mut wizard,
        &prompt,
        &cannot_link(FsKind::Linking("ext4".to_owned())),
        &proving(),
        &applying(&paths, "t"),
    )
    .await;

    assert!(matches!(outcome, Ok(Outcome::Applied)));
    let warnings = prompt.warnings.borrow();
    assert!(matches!(
        warnings.as_slice(),
        [StorageWarning::CopyOnly { limitation: None }]
    ));
}

#[tokio::test]
async fn a_location_that_cannot_link_can_be_swapped_for_one_that_can() {
    let dir = scratch("swap");
    let paths = layout(&dir);
    let mut wizard = Wizard::new(Environment::LinuxNative);
    // Two locations offered: the first cannot link and is declined, the second
    // links and is taken.
    let first = dir.join("copies");
    let second = dir.join("links");
    let prompt = Scripted {
        locations: std::cell::RefCell::new(VecDeque::from([first.clone(), second.clone()])),
        accept: Accept::Elsewhere,
        ..Scripted::workable(second.clone())
    };
    // The first link attempt fails; every one after it takes.
    let filesystem = ProbeFs {
        failing_links: AtomicUsize::new(1),
        ..ProbeFs::links()
    };

    let outcome = run(
        &mut wizard,
        &prompt,
        &filesystem,
        &proving(),
        &applying(&paths, "t"),
    )
    .await;

    assert!(matches!(outcome, Ok(Outcome::Applied)));
    // The declined location was warned about; the one taken was the linking one.
    assert_eq!(prompt.warnings.borrow().len(), 1);
    assert_eq!(
        prompt.hardlinked.borrow().as_slice(),
        [(second.clone(), false)]
    );
    let file = store::read(&paths.env_file()).unwrap_or_default();
    assert_eq!(
        file.get("DATA_ROOT"),
        Some(second.to_string_lossy().as_ref())
    );
}

#[tokio::test]
async fn a_location_that_does_not_exist_yet_is_tested_through_its_parent() {
    let dir = scratch("inferred");
    let paths = layout(&dir);
    let mut wizard = Wizard::new(Environment::LinuxNative);
    let prompt = Scripted::workable(dir.join("data-root"));
    // The chosen leaf is not there yet, so the first resolve fails and its
    // parent stands in — the link is proven on the parent, not the path.
    let filesystem = ProbeFs {
        missing_leaves: AtomicUsize::new(1),
        ..ProbeFs::links()
    };

    let outcome = run(
        &mut wizard,
        &prompt,
        &filesystem,
        &proving(),
        &applying(&paths, "t"),
    )
    .await;

    assert!(matches!(outcome, Ok(Outcome::Applied)));
    // The good result is carried back as inferred, so the operator is not told a
    // path the probe never touched is proven to link.
    assert_eq!(
        prompt.hardlinked.borrow().as_slice(),
        [(dir.join("data-root"), true)]
    );
}

#[tokio::test]
async fn a_location_that_cannot_be_written_is_reported_untestable() {
    let dir = scratch("unwritable");
    let paths = layout(&dir);
    let mut wizard = Wizard::new(Environment::LinuxNative);
    let prompt = Scripted {
        accept: Accept::Location,
        ..Scripted::workable(dir.join("data-root"))
    };
    let filesystem = ProbeFs {
        writable: false,
        ..ProbeFs::links()
    };

    let outcome = run(
        &mut wizard,
        &prompt,
        &filesystem,
        &proving(),
        &applying(&paths, "t"),
    )
    .await;

    assert!(matches!(outcome, Ok(Outcome::Applied)));
    // A location that could not be tested is reported as untested, with why —
    // not silently passed nor called copy-only.
    let warnings = prompt.warnings.borrow();
    assert!(matches!(
        warnings.as_slice(),
        [StorageWarning::Untested { reason }] if reason.contains("permission")
    ));
}

#[tokio::test]
async fn a_location_with_no_reachable_parent_cannot_be_tested() {
    let dir = scratch("unreachable");
    let paths = layout(&dir);
    let mut wizard = Wizard::new(Environment::LinuxNative);
    let prompt = Scripted {
        accept: Accept::Location,
        ..Scripted::workable(dir.join("data-root"))
    };
    // Nothing on the path resolves — not the location, not any parent of it.
    let filesystem = ProbeFs {
        reachable: false,
        ..ProbeFs::links()
    };

    let outcome = run(
        &mut wizard,
        &prompt,
        &filesystem,
        &proving(),
        &applying(&paths, "t"),
    )
    .await;

    assert!(matches!(outcome, Ok(Outcome::Applied)));
    let warnings = prompt.warnings.borrow();
    assert!(matches!(
        warnings.as_slice(),
        [StorageWarning::Untested { reason }] if reason.contains("could not be reached")
    ));
}

#[tokio::test]
async fn the_probe_filesystem_stubs_the_calls_the_probe_never_makes() {
    // The data-location probe reaches only for what proves a hardlink; the rest
    // of the filesystem port it never touches. Pinning the double's answers to
    // those keeps a future probe that did start calling them from meeting a
    // surprise rather than a defined stub.
    let filesystem = ProbeFs::links();
    assert_eq!(filesystem.read(Path::new("/anything")).await, None);
    filesystem.write(Path::new("/anything"), "ignored").await;
    assert_eq!(filesystem.ownership(Path::new("/anything")).await, None);
}

#[tokio::test]
async fn a_link_that_cannot_be_confirmed_is_reported_untestable() {
    let dir = scratch("unconfirmed");
    let paths = layout(&dir);
    let mut wizard = Wizard::new(Environment::LinuxNative);
    let prompt = Scripted {
        accept: Accept::Location,
        ..Scripted::workable(dir.join("data-root"))
    };
    // The link is made, but the two names read back as different files.
    let filesystem = ProbeFs {
        confirmed_file: 999,
        ..ProbeFs::links()
    };

    let outcome = run(
        &mut wizard,
        &prompt,
        &filesystem,
        &proving(),
        &applying(&paths, "t"),
    )
    .await;

    assert!(matches!(outcome, Ok(Outcome::Applied)));
    let warnings = prompt.warnings.borrow();
    assert!(matches!(
        warnings.as_slice(),
        [StorageWarning::Untested { reason }] if reason.contains("could not be confirmed")
    ));
}
