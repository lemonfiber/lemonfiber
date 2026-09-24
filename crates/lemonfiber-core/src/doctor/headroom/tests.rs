use std::sync::Arc;

use super::{HeadroomCheck, HEADROOM_LOW};
use crate::doctor::{Category, Check, Verdict};
use crate::ports::filesystem::{FsKind, StorageFacts};
use crate::quality::Preset;
use crate::test_support::SeedFs;

/// A filesystem that reports the given free and total bytes for any describe.
fn with_space(available: u64, total: u64) -> Arc<SeedFs> {
    Arc::new(SeedFs::keyed(None, None).with_facts(StorageFacts {
        point: std::path::PathBuf::from("/"),
        kind: FsKind::Linking("test".to_owned()),
        removable: false,
        available,
        total,
    }))
}

async fn verdict(check: HeadroomCheck) -> Option<Verdict> {
    check.run().await.into_iter().next().map(|f| f.verdict)
}

const TB: u64 = 1_000_000_000_000;

const GB: u64 = 1_000_000_000;

#[tokio::test]
async fn ample_space_for_the_chosen_quality_passes() {
    // 3 TB free at Maximum (~17.5 GB/hr midpoint) holds ~170 hours — well above
    // the starter floor.
    let check = HeadroomCheck::new(
        with_space(3 * TB, 4 * TB),
        Some("/data".into()),
        Preset::Maximum,
    );
    assert!(matches!(verdict(check).await, Some(Verdict::Pass { .. })));
}

#[tokio::test]
async fn too_little_space_for_the_chosen_quality_warns() {
    // 300 GB free at Maximum holds only ~17 hours — below the starter floor, but
    // far above the absolute low-space floor, so this is a quality-fit warning.
    let check = HeadroomCheck::new(
        with_space(300 * GB, TB),
        Some("/data".into()),
        Preset::Maximum,
    );
    assert!(matches!(
        verdict(check).await,
        Some(Verdict::Warn(problem)) if problem.code == HEADROOM_LOW
    ));
}

#[tokio::test]
async fn the_same_disk_is_ample_for_a_lighter_preset() {
    // 300 GB free is thin for Maximum but roomy for space-saving (~0.75 GB/hr →
    // ~400 hours), so the warning is about the quality, not just the disk.
    let check = HeadroomCheck::new(
        with_space(300 * GB, TB),
        Some("/data".into()),
        Preset::SpaceSaving,
    );
    assert!(matches!(verdict(check).await, Some(Verdict::Pass { .. })));
}

#[tokio::test]
async fn a_disk_below_the_low_space_floor_defers_to_the_space_check() {
    // 5 GB free is a free-space problem the space check already reports; this
    // check does not warn a second time about it.
    let check = HeadroomCheck::new(
        with_space(5 * GB, TB),
        Some("/data".into()),
        Preset::Maximum,
    );
    assert!(matches!(
        verdict(check).await,
        Some(Verdict::Skipped { .. })
    ));
}

#[tokio::test]
async fn a_volume_that_cannot_be_measured_is_unverified() {
    // A zero total means the volume could not be read, not that it is full.
    let check = HeadroomCheck::new(with_space(0, 0), Some("/data".into()), Preset::Balanced);
    assert!(matches!(
        verdict(check).await,
        Some(Verdict::Unverified { .. })
    ));
}

#[tokio::test]
async fn no_data_location_is_skipped() {
    let check = HeadroomCheck::new(with_space(TB, TB), None, Preset::Maximum);
    assert!(matches!(
        verdict(check).await,
        Some(Verdict::Skipped { .. })
    ));
}

#[test]
fn the_check_is_a_storage_check() {
    let check = HeadroomCheck::new(with_space(TB, TB), None, Preset::Balanced);
    assert_eq!(check.category(), Category::Storage);
}
