//! A read a stream polls writes nothing, however often it is asked.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::app::command::polling::tests::every_polled;
use crate::app::{dispatch, Command, Ctx, Surfacing};
use crate::config::store;
use crate::test_support::{a_context, env_without_password};

/// A machine whose environment file still holds a credential, which settling the stack
/// files away: a write any settling leaves behind.
fn unsettled(name: &str) -> Ctx {
    let env = env_without_password(name);
    assert!(store::set(&env, "INDEXER_APIKEY", "indexer-v4lue").is_ok());
    a_context()
        .settings(crate::config::Settings {
            env_file: Some(env.to_path_buf()),
            ..crate::config::Settings::default()
        })
        .holding(env)
        .build()
}

/// Every file under `dir`, and what it holds.
fn tree(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut held = BTreeMap::new();
    let mut waiting = vec![dir.to_path_buf()];
    while let Some(at) = waiting.pop() {
        for entry in std::fs::read_dir(&at).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                waiting.push(path);
            } else {
                held.insert(path.clone(), std::fs::read(&path).unwrap_or_default());
            }
        }
    }
    held
}

#[tokio::test]
async fn no_polled_read_writes_anything_however_often_it_is_asked() {
    let ctx = unsettled("polled-writes-nothing");
    let dir = ctx
        .settings
        .env_file
        .as_deref()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_default();
    let before = tree(&dir);
    assert!(!before.is_empty(), "the machine holds no files to compare");

    for _ in 0..3 {
        for polled in every_polled() {
            drop(dispatch(Command::Surface(Surfacing::Polled(polled)), &ctx).await);
        }
    }
    assert_eq!(tree(&dir), before, "a polled read wrote to the machine");

    drop(dispatch(Command::Copying, &ctx).await);
    assert_ne!(
        tree(&dir),
        before,
        "the read a route asks settled nothing either, so this machine shows no settling"
    );
}
