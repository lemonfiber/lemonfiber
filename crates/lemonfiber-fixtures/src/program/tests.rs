use super::{FileSystem as _, Path, PathBuf, Program, Storage as _};

#[tokio::test]
async fn a_name_resolves_to_where_it_was_pointed_and_otherwise_to_itself() {
    let machine = Program::ordinary().linked("/usr/local/bin/lf", "/opt/Cellar/lf");

    assert_eq!(
        machine
            .canonicalize(Path::new("/usr/local/bin/lf"))
            .await
            .ok(),
        Some(PathBuf::from("/opt/Cellar/lf"))
    );
    assert_eq!(
        machine.canonicalize(Path::new("/elsewhere/lf")).await.ok(),
        Some(PathBuf::from("/elsewhere/lf"))
    );
}

#[tokio::test]
async fn a_machine_that_resolves_nothing_says_so_rather_than_answering() {
    let machine = Program::ordinary().unresolvable();
    assert!(machine.canonicalize(Path::new("/anywhere")).await.is_err());
}

#[tokio::test]
async fn what_a_run_wrote_and_what_it_took_away_are_both_readable_afterwards() {
    let machine = Program::ordinary().holding("/records/updates.json", "{}");

    assert_eq!(
        machine
            .read(Path::new("/records/updates.json"))
            .await
            .as_deref(),
        Some("{}")
    );
    assert_eq!(machine.read(Path::new("/records/nothing")).await, None);
    machine
        .write(Path::new("/records/updates.json"), "later")
        .await;
    machine.remove(Path::new("/bin/.probe")).await;
    assert_eq!(
        machine.written(),
        vec![(PathBuf::from("/records/updates.json"), "later".to_owned())]
    );
    assert_eq!(machine.removed(), vec![PathBuf::from("/bin/.probe")]);
}

#[tokio::test]
async fn a_directory_that_will_not_take_a_file_says_which_path_it_refused() {
    let machine = Program::ordinary().unwritable();
    let refused = machine.touch(Path::new("/usr/bin/.probe")).await;
    assert_eq!(
        refused.err().map(|fault| fault.message),
        Some("permission denied: /usr/bin/.probe".to_owned())
    );
    assert!(Program::ordinary()
        .touch(Path::new("/tmp/.probe"))
        .await
        .is_ok());
}

/// The stub half of the contract, which nothing else reaches. A test that asks
/// this fake for one of them has reached for a capability it was never meant to
/// stand in for, so each refuses rather than answering plausibly.
#[tokio::test]
async fn every_capability_this_fake_does_not_stand_in_for_answers_as_unused() {
    let machine = Program::ordinary();
    let path = Path::new("/usr/bin/lemonfiber");

    assert!(machine.link(path, path).await.is_err());
    assert!(machine.identify(path).await.is_err());
    assert!(machine.ownership(path).await.is_none());
    assert_eq!(machine.describe(path).await.available, 0);
}
