use std::path::PathBuf;
use std::sync::Arc;

use super::{verdict, Bench};
use crate::doctor::storage::{Check, Environment, StorageCheck, Verdict};
use crate::error::codes::storage::ROOT_REFUSED;

/// A data root the setting names that is refused fails the storage check naming it, and
/// nothing on this machine is probed in its place.
#[tokio::test]
async fn a_refused_data_root_fails_naming_it_and_nothing_is_probed() {
    let bench = Bench::healthy();
    let recorded = Arc::clone(&bench.recorded);
    let refused = crate::config::storage::Storage::read("nfs://nas local/volume1")
        .err()
        .map(|refused| refused.problem("nfs://nas local/volume1"));
    assert!(refused.is_some());

    let findings = StorageCheck::new(
        Arc::new(bench),
        Some(PathBuf::from("/data")),
        None,
        Environment::MacOs,
        None,
        None,
        Vec::new(),
    )
    .refusing(refused)
    .run()
    .await;

    assert!(matches!(
        verdict(&findings, "storage.hardlinks"),
        Some(Verdict::Fail(problem))
            if problem.code == ROOT_REFUSED && problem.summary.contains("nfs://nas local/volume1")
    ));
    assert!(matches!(
        verdict(&findings, "storage.mode"),
        Some(Verdict::Skipped { .. })
    ));
    assert!(recorded.lock().is_ok_and(|seen| seen.is_empty()));
}
