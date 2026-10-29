use std::os::unix::fs::{symlink, PermissionsExt as _};
use std::path::PathBuf;

use super::{Shared, Unshared, LARGEST};

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

#[tokio::test(flavor = "multi_thread")]
async fn a_file_is_written_whole_with_its_mode_and_read_back() {
    let directories = Directories::new("written");
    let shared = Shared::at(&directories.shared);

    assert_eq!(
        shared.write("caddy.json", b"{}".to_vec(), 0o600).await,
        Ok(())
    );

    assert_eq!(shared.read("caddy.json").await, Ok(Some(b"{}".to_vec())));
    let mode = std::fs::metadata(directories.shared.join("caddy.json"))
        .map(|held| held.permissions().mode() & 0o777)
        .unwrap_or_default();
    assert_eq!(mode, 0o600);
    assert_eq!(shared.read("absent.json").await, Ok(None));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_mode_keeps_only_the_owners_write_and_everyones_read() {
    let directories = Directories::new("mode");
    let shared = Shared::at(&directories.shared);

    assert_eq!(shared.write("open", b"x".to_vec(), 0o777).await, Ok(()));

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
            .write("caddy.json", b"ours".to_vec(), 0o600)
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
        shared.read("caddy.json").await,
        Err(Unshared::Linked("caddy.json".to_owned()))
    );
    assert_eq!(
        shared.write("caddy.json", b"ours".to_vec(), 0o600).await,
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
        shared.write("caddy.json", b"ours".to_vec(), 0o600).await,
        Err(Unshared::Linked(_))
    ));
    assert!(matches!(
        shared.read("caddy.json").await,
        Err(Unshared::Linked(_))
    ));
    assert!(std::fs::read_dir(&directories.outside).is_ok_and(|mut held| held.next().is_none()));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_name_that_is_not_one_plain_file_name_is_refused() {
    let directories = Directories::new("named");
    let shared = Shared::at(&directories.shared);

    for name in [
        "",
        ".",
        "..",
        "../outside/target",
        "a/b",
        "a\0b",
        ".caddy.json.writing",
    ] {
        assert_eq!(
            shared.write(name, b"x".to_vec(), 0o600).await,
            Err(Unshared::Named(name.to_owned()))
        );
        assert_eq!(
            shared.read(name).await,
            Err(Unshared::Named(name.to_owned()))
        );
    }
    assert!(std::fs::read_dir(&directories.outside).is_ok_and(|mut held| held.next().is_none()));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_directory_at_the_staging_name_or_the_target_is_refused() {
    let directories = Directories::new("directories");
    let _ = std::fs::create_dir_all(directories.shared.join(".a.writing"));
    let _ = std::fs::create_dir_all(directories.shared.join("b"));
    let shared = Shared::at(&directories.shared);

    assert!(matches!(
        shared.write("a", b"x".to_vec(), 0o600).await,
        Err(Unshared::Linked(_))
    ));
    assert!(matches!(
        shared.write("b", b"x".to_vec(), 0o600).await,
        Err(Unshared::Failed(..))
    ));
    assert!(matches!(shared.read("b").await, Err(Unshared::Linked(_))));
    assert!(std::fs::symlink_metadata(directories.shared.join(".b.writing")).is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_file_larger_than_the_largest_is_refused() {
    let directories = Directories::new("large");
    let large = vec![b'a'; usize::try_from(LARGEST + 1).unwrap_or_default()];
    let _ = std::fs::write(directories.shared.join("large"), large);

    assert_eq!(
        Shared::at(&directories.shared).read("large").await,
        Err(Unshared::Large("large".to_owned()))
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_directory_that_is_not_there_fails_naming_it() {
    let shared = Shared::at(std::path::Path::new("/nonexistent/kit-shared"));

    assert!(matches!(
        shared.write("a", b"x".to_vec(), 0o600).await,
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

    let stale = shared.write("kept", b"x".to_vec(), 0o600).await;
    let fresh = shared.write("fresh", b"x".to_vec(), 0o600).await;
    let unread = shared.read("unread").await;
    let _ = std::fs::set_permissions(&directories.shared, std::fs::Permissions::from_mode(0o755));

    assert!(matches!(stale, Err(Unshared::Failed(name, _)) if name == "kept"));
    assert!(matches!(fresh, Err(Unshared::Failed(name, _)) if name == "fresh"));
    assert!(matches!(unread, Err(Unshared::Failed(name, _)) if name == "unread"));
}
