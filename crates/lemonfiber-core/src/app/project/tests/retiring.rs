//! The stack's own Compose files, taken away where lemonfiber wrote them and nobody
//! changed them since.

use super::machine;
use crate::materialised::{checksum, Materialised};

/// What lemonfiber wrote as the stack's own Compose file.
const OURS: &str = "services: {}\n";

#[test]
fn what_lemonfiber_wrote_goes_and_what_somebody_changed_stays_for_the_doctor() {
    let (ctx, project) = machine("retired");
    let fragments = project.join("compose");
    let _ = std::fs::create_dir_all(fragments.join("plugins"));
    let _ = std::fs::write(project.join("compose.yml"), OURS);
    let _ = std::fs::write(fragments.join("media.yml"), "services:\n  theirs: {}\n");
    let _ = std::fs::write(fragments.join("plugins").join("komga.yml"), OURS);
    let at = super::super::record_path(&ctx);
    let mut record = Materialised::default();
    record.record("compose.yml", checksum(OURS.as_bytes()));
    record.record("compose/media.yml", checksum(OURS.as_bytes()));
    record.record("stacks/compose.storage.nas.yml", checksum(OURS.as_bytes()));
    assert!(crate::app::record::keep(at.as_deref(), &record).is_ok());
    let komga = crate::test_support::an_installed("komga", Vec::new());

    super::super::retired(&ctx, &[komga]);

    assert!(!project.join("compose.yml").exists());
    assert!(!fragments.join("plugins").exists());
    assert_eq!(
        super::super::still_there(&project),
        [fragments.join("media.yml")]
    );
    let kept: Materialised = crate::app::record::kept(at.as_deref());
    assert_eq!(kept.checksum("compose.yml"), None);
    assert!(kept.checksum("compose/media.yml").is_some());
    assert_eq!(
        kept.checksum("stacks/compose.storage.nas.yml"),
        None,
        "a file already gone is forgotten"
    );
}

#[test]
fn nothing_is_taken_from_a_stack_directory_the_operator_named() {
    let (mut ctx, _) = machine("retired-external");
    ctx.stack = crate::test_support::stack();
    let project = super::super::directory(&ctx).unwrap_or_default();
    let _ = std::fs::write(project.join("compose.yml"), OURS);

    super::super::retired(&ctx, &[]);

    assert!(project.join("compose.yml").is_file());
}

/// The manifest an earlier release wrote out for the embedded stack, which nothing reads
/// once the stack is read where it is carried.
const MANIFEST: &str = "schema_version = 1\n";

#[test]
fn the_manifest_files_an_earlier_release_wrote_go_where_nobody_changed_them() {
    let (ctx, project) = machine("retired-manifest");
    let services = project.join("services");
    let _ = std::fs::create_dir_all(&services);
    let _ = std::fs::write(project.join("stack.toml"), MANIFEST);
    let _ = std::fs::write(services.join("sonarr.toml"), MANIFEST);
    let _ = std::fs::write(
        services.join("radarr.toml"),
        "[[service]]\nid = \"theirs\"\n",
    );
    let at = super::super::record_path(&ctx);
    let mut record = Materialised::default();
    record.record("stack.toml", checksum(MANIFEST.as_bytes()));
    record.record("services/sonarr.toml", checksum(MANIFEST.as_bytes()));
    record.record("services/radarr.toml", checksum(MANIFEST.as_bytes()));
    assert!(crate::app::record::keep(at.as_deref(), &record).is_ok());

    super::super::retired(&ctx, &[]);

    assert!(!project.join("stack.toml").exists());
    assert!(!services.join("sonarr.toml").exists());
    assert!(
        services.join("radarr.toml").is_file(),
        "a changed file stays"
    );
    let kept: Materialised = crate::app::record::kept(at.as_deref());
    assert_eq!(kept.checksum("stack.toml"), None);
    assert!(kept.checksum("services/radarr.toml").is_some());
}
