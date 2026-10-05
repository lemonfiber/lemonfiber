use std::path::Path;

use crate::Disk;
use lemonfiber_ports::occupancy::{Occupancy, Occupant};

/// Everything a walk beneath `root` sends, or the refusal it gave.
async fn walked(root: &Path) -> Result<Vec<Occupant>, lemonfiber_ports::filesystem::Fault> {
    let mut sent = Disk.beneath(root).await?;
    let mut found = Vec::new();
    while let Some(occupant) = sent.recv().await {
        found.push(occupant);
    }
    found.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(found)
}

#[tokio::test]
async fn a_tree_that_is_not_there_holds_nothing_rather_than_failing() {
    // The ordinary first-run state: a data location named before anything was
    // written into it.
    let walked = walked(Path::new("/a-path-nothing-has-ever-been-at")).await;
    assert_eq!(walked, Ok(Vec::new()));
}

#[tokio::test]
async fn every_file_beneath_a_real_tree_is_found_with_its_size_and_identity() {
    let root = lemonfiber_fixtures::scratch::Scratch::named("walk");
    let nested = root.join("under");
    let _ = std::fs::create_dir_all(&nested);
    let _ = std::fs::write(root.join("top.txt"), "0123456789");
    let _ = std::fs::write(nested.join("deep.txt"), "abc");

    let walked = walked(&root).await.unwrap_or_default();
    let names: Vec<String> = walked
        .iter()
        .filter_map(|occupant| {
            occupant
                .path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .collect();
    // Sent in whatever order the directory reads hand back; put in path order here
    // so the assertion names what was found rather than the order it came in.
    assert_eq!(names, ["top.txt", "deep.txt"], "every file was found");
    assert_eq!(
        walked.iter().map(|occupant| occupant.bytes).sum::<u64>(),
        13
    );
    assert!(
        walked.iter().all(|occupant| occupant
            .identity
            .is_some_and(|identity| identity.links >= 1)),
        "each name says which file it points at"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn a_file_named_as_the_root_is_a_root_with_nothing_beneath_it() {
    let root = lemonfiber_fixtures::scratch::Scratch::named("file");
    let _ = std::fs::write(&root, "x");
    // Reading a file as a directory is not a "not there", so it reaches the
    // operator as the platform's own words rather than as an empty answer.
    assert!(walked(&root).await.is_err());
    let _ = std::fs::remove_file(&root);
}

/// A reader that goes away ends the walk, wherever in the tree it has got to.
///
/// More files than the walk sends ahead of its reader, in the root and in a directory
/// beneath it, so the walk is waiting on the reader when it goes away.
#[tokio::test]
async fn a_reader_that_goes_away_ends_the_walk() {
    let root = lemonfiber_fixtures::scratch::Scratch::named("abandoned");
    let nested = root.join("under");
    let _ = std::fs::create_dir_all(&nested);
    for number in 0..=super::WALKED_AHEAD {
        let _ = std::fs::write(root.join(format!("top-{number}")), "");
        let _ = std::fs::write(nested.join(format!("deep-{number}")), "");
    }

    let mut sent = Disk.beneath(&root).await.ok();
    let first = match sent.as_mut() {
        Some(sent) => sent.recv().await,
        None => None,
    };
    drop(sent);
    assert!(first.is_some(), "the walk sent what it found");

    let _ = Disk.beneath(&nested).await.map(drop);
    let _ = std::fs::remove_dir_all(&root);
}
