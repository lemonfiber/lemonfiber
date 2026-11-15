use super::*;

/// What is written is read back until it is removed, and every other question is
/// answered as a filesystem that only keeps writes answers it.
#[tokio::test]
async fn what_is_written_is_read_back_until_removed() {
    let disk = Remembering::default();
    let at = Path::new("/stack/claim");

    disk.write(at, "ours").await;
    assert_eq!(disk.read(at).await.as_deref(), Some("ours"));
    disk.remove(at).await;
    assert_eq!(disk.read(at).await, None);

    assert_eq!(disk.canonicalize(at).await.ok(), Some(at.to_path_buf()));
    assert!(disk.touch(at).await.is_ok());
    assert!(disk.link(at, Path::new("/stack/other")).await.is_ok());
    assert!(disk.identify(at).await.is_err());
    assert!(disk.ownership(at).await.is_none());
    assert!(matches!(disk.describe(at).await.kind, FsKind::Linking(_)));
}
