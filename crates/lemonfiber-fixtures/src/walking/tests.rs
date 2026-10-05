use std::path::{Path, PathBuf};

use lemonfiber_ports::filesystem::Identity;
use lemonfiber_ports::occupancy::{Occupancy, Occupant};

use super::Walking;

/// A file at a path, whose size and identity no case here turns on.
fn file(path: &str) -> Occupant {
    Occupant {
        path: PathBuf::from(path),
        bytes: 1,
        identity: Some(Identity { file: 1, links: 1 }),
    }
}

/// Everything a walk of `root` sends, or nothing where it refused.
async fn walked(walking: &Walking, root: &str) -> Option<Vec<Occupant>> {
    let mut sent = walking.beneath(Path::new(root)).await.ok()?;
    let mut found = Vec::new();
    while let Some(occupant) = sent.recv().await {
        found.push(occupant);
    }
    Some(found)
}

#[tokio::test]
async fn a_walk_answers_with_what_is_beneath_the_path_asked_about() {
    let walking = Walking::holding(vec![file("/srv/media/a.mkv"), file("/elsewhere/b.mkv")]);
    assert_eq!(
        walked(&walking, "/srv/media").await,
        Some(vec![file("/srv/media/a.mkv")])
    );
    assert_eq!(walked(&walking, "/nowhere").await, Some(Vec::new()));
}

#[tokio::test]
async fn a_tree_that_will_not_be_read_says_so_in_the_platforms_words() {
    let refused = Walking::refusing("permission denied")
        .beneath(Path::new("/srv/media"))
        .await;
    assert!(refused.is_err_and(|fault| fault.message == "permission denied"));
}
