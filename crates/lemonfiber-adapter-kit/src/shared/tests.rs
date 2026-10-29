use std::os::unix::fs::{symlink, PermissionsExt as _};
use std::path::PathBuf;

use super::{Shared, SharedName, Unshared, LARGEST};

/// A directory of its own, and one beside it the upstream would like written, both
/// removed when dropped.
struct Directories {
    shared: PathBuf,
    outside: PathBuf,
}

impl Directories {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!("kit-shared-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let shared = root.join("shared");
        let outside = root.join("outside");
        let _ = std::fs::create_dir_all(&shared);
        let _ = std::fs::create_dir_all(&outside);
        Self { shared, outside }
    }

    fn outside(&self) -> String {
        std::fs::read_to_string(self.outside.join("target")).unwrap_or_default()
    }
}

impl Drop for Directories {
    fn drop(&mut self) {
        if let Some(root) = self.shared.parent() {
            let _ = std::fs::remove_dir_all(root);
        }
    }
}

fn named(name: &str) -> SharedName {
    SharedName::new(name).unwrap_or_else(|_| unreachable!("{name:?} is one file name"))
}

#[tokio::test(flavor = "multi_thread")]
async fn a_file_is_written_whole_with_its_mode_and_read_back() {
    let directories = Directories::new("written");
    let shared = Shared::at(&directories.shared);

    assert_eq!(
        shared
            .write(&named("caddy.json"), b"{}".to_vec(), 0o600)
            .await,
        Ok(())
    );

    assert_eq!(
        shared.read(&named("caddy.json")).await,
        Ok(Some(b"{}".to_vec()))
    );
    let mode = std::fs::metadata(directories.shared.join("caddy.json"))
        .map(|held| held.permissions().mode() & 0o777)
        .unwrap_or_default();
    assert_eq!(mode, 0o600);
    assert_eq!(shared.read(&named("absent.json")).await, Ok(None));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_mode_keeps_only_the_owners_write_and_everyones_read() {
    let directories = Directories::new("mode");
    let shared = Shared::at(&directories.shared);

    assert_eq!(
        shared.write(&named("open"), b"x".to_vec(), 0o777).await,
        Ok(())
    );

    let mode = std::fs::metadata(directories.shared.join("open"))
        .map(|held| held.permissions().mode() & 0o777)
        .unwrap_or_default();
    assert_eq!(mode, 0o644);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_link_planted_at_the_staging_name_is_removed_and_never_written_through() {
    let directories = Directories::new("staging");
    let _ = std::fs::write(directories.outside.join("target"), "theirs");
    let _ = symlink(
        directories.outside.join("target"),
        directories.shared.join(".caddy.json.writing"),
    );

    assert_eq!(
        Shared::at(&directories.shared)
            .write(&named("caddy.json"), b"ours".to_vec(), 0o600)
            .await,
        Ok(())
    );

    assert_eq!(directories.outside(), "theirs");
    assert_eq!(
        std::fs::read_to_string(directories.shared.join("caddy.json")).unwrap_or_default(),
        "ours"
    );
    assert!(std::fs::symlink_metadata(directories.shared.join(".caddy.json.writing")).is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_link_planted_at_the_target_is_replaced_and_never_written_through() {
    let directories = Directories::new("target");
    let _ = std::fs::write(directories.outside.join("target"), "theirs");
    let _ = symlink(
        directories.outside.join("target"),
        directories.shared.join("caddy.json"),
    );
    let shared = Shared::at(&directories.shared);

    assert_eq!(
        shared.read(&named("caddy.json")).await,
        Err(Unshared::Linked("caddy.json".to_owned()))
    );
    assert_eq!(
        shared
            .write(&named("caddy.json"), b"ours".to_vec(), 0o600)
            .await,
        Ok(())
    );

    assert_eq!(directories.outside(), "theirs");
    let held = std::fs::symlink_metadata(directories.shared.join("caddy.json"));
    assert!(held.is_ok_and(|held| held.file_type().is_file()));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_link_in_place_of_the_directory_is_refused() {
    let directories = Directories::new("directory");
    let linked = directories.shared.with_file_name("linked");
    let _ = symlink(&directories.outside, &linked);
    let shared = Shared::at(&linked);

    assert!(matches!(
        shared
            .write(&named("caddy.json"), b"ours".to_vec(), 0o600)
            .await,
        Err(Unshared::Linked(_))
    ));
    assert!(matches!(
        shared.read(&named("caddy.json")).await,
        Err(Unshared::Linked(_))
    ));
    assert!(std::fs::read_dir(&directories.outside).is_ok_and(|mut held| held.next().is_none()));
}

#[test]
fn a_name_that_is_not_one_path_component_is_never_built() {
    for name in [
        "",
        ".",
        "..",
        "../x",
        "../outside/target",
        "a/b",
        "/etc/passwd",
        "a\0b",
        ".caddy.json.writing",
    ] {
        assert_eq!(SharedName::new(name), Err(Unshared::Named(name.to_owned())));
    }
    assert_eq!(named("caddy.json").as_str(), "caddy.json");
    assert!(SharedName::new(&"a".repeat(255)).is_ok());
    let long = "a".repeat(256);
    assert_eq!(SharedName::new(&long), Err(Unshared::Named(long.clone())));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_file_hardlinked_in_from_elsewhere_is_never_read_and_a_write_replaces_its_name() {
    let directories = Directories::new("hardlinked");
    let _ = std::fs::write(directories.outside.join("target"), "theirs");
    let _ = std::fs::hard_link(
        directories.outside.join("target"),
        directories.shared.join("caddy.json"),
    );
    let shared = Shared::at(&directories.shared);

    assert_eq!(
        shared.read(&named("caddy.json")).await,
        Err(Unshared::Linked("caddy.json".to_owned()))
    );
    assert_eq!(
        shared
            .write(&named("caddy.json"), b"ours".to_vec(), 0o600)
            .await,
        Ok(())
    );

    assert_eq!(directories.outside(), "theirs");
    assert_eq!(
        shared.read(&named("caddy.json")).await,
        Ok(Some(b"ours".to_vec()))
    );
    let names = std::fs::metadata(directories.outside.join("target"))
        .map(|held| std::os::unix::fs::MetadataExt::nlink(&held))
        .unwrap_or_default();
    assert_eq!(names, 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_pipe_at_the_name_is_refused_without_blocking() {
    let directories = Directories::new("pipe");
    let made = std::process::Command::new("mkfifo")
        .arg(directories.shared.join("caddy.json"))
        .status();
    assert!(made.is_ok_and(|status| status.success()));

    assert_eq!(
        Shared::at(&directories.shared)
            .read(&named("caddy.json"))
            .await,
        Err(Unshared::Linked("caddy.json".to_owned()))
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_directory_at_the_staging_name_or_the_target_is_refused() {
    let directories = Directories::new("directories");
    let _ = std::fs::create_dir_all(directories.shared.join(".a.writing"));
    let _ = std::fs::create_dir_all(directories.shared.join("b"));
    let shared = Shared::at(&directories.shared);

    assert!(matches!(
        shared.write(&named("a"), b"x".to_vec(), 0o600).await,
        Err(Unshared::Linked(_))
    ));
    assert!(matches!(
        shared.write(&named("b"), b"x".to_vec(), 0o600).await,
        Err(Unshared::Failed(..))
    ));
    assert!(matches!(
        shared.read(&named("b")).await,
        Err(Unshared::Linked(_))
    ));
    assert!(std::fs::symlink_metadata(directories.shared.join(".b.writing")).is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_file_larger_than_the_largest_is_refused() {
    let directories = Directories::new("large");
    let large = vec![b'a'; usize::try_from(LARGEST + 1).unwrap_or_default()];
    let _ = std::fs::write(directories.shared.join("large"), large);

    assert_eq!(
        Shared::at(&directories.shared).read(&named("large")).await,
        Err(Unshared::Large("large".to_owned()))
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_directory_that_is_not_there_fails_naming_it() {
    let shared = Shared::at(std::path::Path::new("/nonexistent/kit-shared"));

    assert!(matches!(
        shared.write(&named("a"), b"x".to_vec(), 0o600).await,
        Err(Unshared::Failed(..))
    ));
}

#[test]
fn every_refusal_says_what_it_refused() {
    for (refused, said) in [
        (
            Unshared::Named("a/b".to_owned()),
            "\"a/b\" is not one plain file name",
        ),
        (
            Unshared::Linked("a".to_owned()),
            "a is a link or not a plain file",
        ),
        (
            Unshared::Large("a".to_owned()),
            "a is larger than 4194304 bytes",
        ),
        (
            Unshared::Failed("a".to_owned(), "denied".to_owned()),
            "a could not be reached: denied",
        ),
    ] {
        assert_eq!(refused.to_string(), said);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_directory_the_adapter_may_not_write_fails_naming_the_file() {
    let directories = Directories::new("unwritable");
    let _ = std::fs::write(directories.shared.join(".kept.writing"), "stale");
    let _ = std::fs::write(directories.shared.join("unread"), "x");
    let _ = std::fs::set_permissions(
        directories.shared.join("unread"),
        std::fs::Permissions::from_mode(0o000),
    );
    let _ = std::fs::set_permissions(&directories.shared, std::fs::Permissions::from_mode(0o555));
    let shared = Shared::at(&directories.shared);

    let stale = shared.write(&named("kept"), b"x".to_vec(), 0o600).await;
    let fresh = shared.write(&named("fresh"), b"x".to_vec(), 0o600).await;
    let unread = shared.read(&named("unread")).await;
    let _ = std::fs::set_permissions(&directories.shared, std::fs::Permissions::from_mode(0o755));

    assert!(matches!(stale, Err(Unshared::Failed(name, _)) if name == "kept"));
    assert!(matches!(fresh, Err(Unshared::Failed(name, _)) if name == "fresh"));
    assert!(matches!(unread, Err(Unshared::Failed(name, _)) if name == "unread"));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_directory_the_adapter_may_not_search_fails_naming_the_file() {
    let directories = Directories::new("unsearchable");
    let _ = std::fs::set_permissions(&directories.shared, std::fs::Permissions::from_mode(0o600));

    let unseen = Shared::at(&directories.shared)
        .read(&named("caddy.json"))
        .await;
    let _ = std::fs::set_permissions(&directories.shared, std::fs::Permissions::from_mode(0o755));

    assert!(matches!(unseen, Err(Unshared::Failed(name, _)) if name == "caddy.json"));
}
