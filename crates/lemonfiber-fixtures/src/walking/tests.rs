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

#[tokio::test]
async fn a_walk_answers_with_what_is_beneath_the_path_asked_about() {
    let walking = Walking::holding(vec![file("/srv/media/a.mkv"), file("/elsewhere/b.mkv")]);
    let found = walking.beneath(Path::new("/srv/media")).await;
    assert_eq!(found, Ok(vec![file("/srv/media/a.mkv")]));
    assert_eq!(walking.beneath(Path::new("/nowhere")).await, Ok(Vec::new()));
}

#[tokio::test]
async fn a_tree_that_will_not_be_read_says_so_in_the_platforms_words() {
    let refused = Walking::refusing("permission denied")
        .beneath(Path::new("/srv/media"))
        .await;
    assert!(refused.is_err_and(|fault| fault.message == "permission denied"));
}
