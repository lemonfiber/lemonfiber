//! What a capture leaves out of the archive: what an item names, a source that is not
//! there, and a link planted in a captured directory.

use std::fs::File;
use std::path::PathBuf;

use lemonfiber_core::archive::{Archive, Reader};
use lemonfiber_core::backup::{self, Item, Manifest, Scope};

use super::{install, scratch, write_file};
use crate::archive::Tar;

#[tokio::test]
async fn what_an_item_leaves_out_is_neither_packed_nor_measured() {
    let root = scratch("left-out");
    let paths = install(&root);
    write_file(
        &paths
            .service_config()
            .join("jellyfin/cache/images/poster.jpg"),
        "12345",
    );
    write_file(
        &paths.service_config().join("jellyfin/data/library.db"),
        "db",
    );
    let dest = root.join("backups/left-out.tar.gz");
    let tar = Tar;
    let items = vec![Item {
        source: paths.service_config(),
        archive_path: "services".to_owned(),
        label: "services".to_owned(),
        left_out: vec![PathBuf::from("jellyfin/cache")],
    }];
    let measured = tar.space(&root, &items).await.map(|space| space.needed);
    let mut everything = items.clone();
    if let Some(item) = everything.first_mut() {
        item.left_out.clear();
    }
    let all = tar
        .space(&root, &everything)
        .await
        .map(|space| space.needed);
    assert_eq!(
        all.ok()
            .zip(measured.ok())
            .map(|(all, measured)| all - measured),
        Some(5),
        "the left-out cache was measured"
    );

    let plan = backup::plan(&paths, &Scope::WholeStack);
    let manifest = Manifest::describe(&plan, "0.3.0", "t", "/srv/media");
    assert!(tar.write(&dest, &manifest, &items).await.is_ok());
    let back = scratch("left-out-back");
    let restored = install(&back);
    assert!(tar
        .extract(&dest, &backup::destinations(&restored))
        .await
        .is_ok());
    assert!(restored
        .service_config()
        .join("jellyfin/data/library.db")
        .exists());
    assert!(!restored.service_config().join("jellyfin/cache").exists());
}

#[tokio::test]
async fn an_item_whose_source_is_not_there_is_left_out_rather_than_failing() {
    // A stack an operator runs from their own directory, or a service that has
    // not written its configuration yet, simply is not in the archive — the
    // capture still succeeds.
    let root = scratch("absent-item");
    let paths = install(&root);
    let dest = root.join("backups/partial.tar.gz");
    let plan = backup::plan(&paths, &Scope::WholeStack);
    let manifest = Manifest::describe(&plan, "0.3.0", "t", "/srv/media");
    let items = vec![Item {
        source: root.join("never-written"),
        archive_path: "services".to_owned(),
        label: "services".to_owned(),
        left_out: Vec::new(),
    }];

    let tar = Tar;
    assert!(tar.write(&dest, &manifest, &items).await.is_ok());
    // It wrote an archive holding the manifest and nothing else.
    assert!(tar.read_manifest(&dest).await.is_ok());
}

#[cfg(unix)]
#[tokio::test]
async fn a_link_in_a_captured_directory_is_left_out() {
    let root = scratch("linked-item");
    let paths = install(&root);
    let elsewhere = root.join("outside/private.txt");
    write_file(&elsewhere, "not the stack's");
    let linked = paths.service_config().join("sonarr/planted");
    assert!(std::os::unix::fs::symlink(&elsewhere, &linked).is_ok());
    let dest = root.join("backups/linked.tar.gz");
    let items = vec![Item {
        source: paths.service_config(),
        archive_path: "services".to_owned(),
        label: "services".to_owned(),
        left_out: Vec::new(),
    }];
    let plan = backup::plan(&paths, &Scope::WholeStack);
    let manifest = Manifest::describe(&plan, "0.3.0", "t", "/srv/media");
    assert!(Tar.write(&dest, &manifest, &items).await.is_ok());

    let names: Vec<String> = File::open(&dest)
        .ok()
        .map(|file| {
            let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(file));
            archive
                .entries()
                .map(|entries| {
                    entries
                        .filter_map(Result::ok)
                        .filter_map(|entry| {
                            entry.path().ok().map(|path| path.display().to_string())
                        })
                        .collect()
                })
                .unwrap_or_default()
        })
        .unwrap_or_default();
    assert!(
        names.iter().any(|name| name.ends_with("sonarr/config.xml")),
        "{names:?}"
    );
    assert!(
        !names.iter().any(|name| name.ends_with("planted")),
        "{names:?}"
    );
}
