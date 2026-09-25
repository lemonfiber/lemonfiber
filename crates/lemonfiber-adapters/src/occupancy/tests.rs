use std::path::Path;

use crate::Disk;
use lemonfiber_ports::occupancy::Occupancy;

#[tokio::test]
async fn a_tree_that_is_not_there_holds_nothing_rather_than_failing() {
    // The ordinary first-run state: a data location named before anything was
    // written into it.
    let walked = Disk
        .beneath(Path::new("/a-path-nothing-has-ever-been-at"))
        .await;
    assert_eq!(walked, Ok(Vec::new()));
}

#[tokio::test]
async fn every_file_beneath_a_real_tree_is_found_with_its_size_and_identity() {
    let root = lemonfiber_fixtures::scratch::Scratch::named("walk");
    let nested = root.join("under");
    let _ = std::fs::create_dir_all(&nested);
    let _ = std::fs::write(root.join("top.txt"), "0123456789");
    let _ = std::fs::write(nested.join("deep.txt"), "abc");

    let walked = Disk.beneath(&root).await.unwrap_or_default();
    let names: Vec<String> = walked
        .iter()
        .filter_map(|occupant| {
            occupant
                .path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .collect();
    // Ordered by the whole path rather than by the name at the end of it, which
    // is why the file in the root comes before the one under `under/`: `top`
    // sorts before `under`. What matters is that it is the same order twice,
    // whatever a directory read happened to hand back.
    assert_eq!(
        names,
        ["top.txt", "deep.txt"],
        "read in one order every run"
    );
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
    assert!(Disk.beneath(&root).await.is_err());
    let _ = std::fs::remove_file(&root);
}
