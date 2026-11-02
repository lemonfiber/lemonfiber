use super::{bundle, catalogue, files, refused, PART, TABLES};
use crate::read;

/// The fence's image as the bundle pins it, before the pins.
fn fence(image: &str, tag: &str, digest: &str) -> String {
    format!("[fence]\nimage = \"{image}\"\ntag = \"{tag}\"\ndigest = \"{digest}\"\n\n")
}

/// A part with a fenced service is read only where the bundle pins the fence's image, and
/// that pin is held to where the fence is published, a release or a commit image, and a
/// digest.
#[test]
fn a_fenced_part_needs_the_fence_image_pinned_and_held_to_its_shape() {
    let fenced = PART.replace(
        "asks_for = \"The articles each download is made of\"",
        "asks_for = \"The articles each download is made of\"\nnetwork = { fenced = true }",
    );
    let files = files();
    let digest = format!("sha256:{}", "a".repeat(64));
    let with = |pinned: &str| format!("{TABLES}\n{pinned}");
    let catalogued = |name: &str, tables: &str| {
        catalogue(
            name,
            &bundle(&fenced, &files, &[], tables),
            "sabnzbd",
            &fenced,
            &files,
        )
    };

    let unpinned = refused(&catalogued("fence-unpinned", TABLES));
    assert!(
        unpinned
            .iter()
            .any(|line| line.contains("service sabnzbd is fenced") && line.contains("[fence]")),
        "{unpinned:?}"
    );
    let pinned = read(&catalogued(
        "fence-pinned",
        &with(&fence("ghcr.io/lemonfiber/fence", "0.18.0", &digest)),
    ));
    assert_eq!(
        pinned
            .ok()
            .and_then(|(bundle, _)| bundle.fence)
            .map(|image| image.reference()),
        Some(format!("ghcr.io/lemonfiber/fence:0.18.0@{digest}"))
    );
    let pre = read(&catalogued(
        "fence-pre",
        &with(&fence("ghcr.io/lemonfiber/fence", "0.18.0-beta.1", &digest)),
    ));
    assert!(pre.is_ok(), "{pre:?}");
    let commit = read(&catalogued(
        "fence-commit",
        &with(&fence(
            "ghcr.io/lemonfiber/fence",
            "main-8de8b3aa2b24",
            &digest,
        )),
    ));
    assert!(commit.is_ok(), "{commit:?}");

    for (name, pinned, said) in [
        (
            "fence-elsewhere",
            fence("ghcr.io/else/fence", "0.18.0", &digest),
            "fence.image is ghcr.io/else/fence",
        ),
        (
            "fence-tag",
            fence("ghcr.io/lemonfiber/fence", "v0.18.0", &digest),
            "fence.tag v0.18.0 is not a release",
        ),
        (
            "fence-short-commit",
            fence("ghcr.io/lemonfiber/fence", "main-8de8b3aa2b2", &digest),
            "fence.tag main-8de8b3aa2b2 is not a release",
        ),
        (
            "fence-upper-commit",
            fence("ghcr.io/lemonfiber/fence", "main-8DE8B3AA2B24", &digest),
            "fence.tag main-8DE8B3AA2B24 is not a release",
        ),
        (
            "fence-branch",
            fence("ghcr.io/lemonfiber/fence", "next-8de8b3aa2b24", &digest),
            "fence.tag next-8de8b3aa2b24 is not a release",
        ),
        (
            "fence-digest",
            fence("ghcr.io/lemonfiber/fence", "0.18.0", "sha256:abc"),
            "fence.digest is not written",
        ),
    ] {
        let broken = refused(&catalogued(name, &with(&pinned)));
        assert!(
            broken.iter().any(|line| line.contains(said)),
            "{name}: {broken:?}"
        );
    }
}

#[test]
fn a_part_taking_render_needs_the_fence_image_pinned_to_read_its_group_with() {
    let rendering = PART.replace(
        "asks_for = \"The articles each download is made of\"",
        "asks_for = \"The articles each download is made of\"\nshape = \"render\"",
    );
    let files = files();
    let digest = format!("sha256:{}", "a".repeat(64));
    let catalogued = |name: &str, tables: &str| {
        catalogue(
            name,
            &bundle(&rendering, &files, &[], tables),
            "sabnzbd",
            &rendering,
            &files,
        )
    };
    let naming = |lines: &[String]| {
        lines
            .iter()
            .any(|line| line.contains("service sabnzbd takes render") && line.contains("[fence]"))
    };

    let unpinned = refused(&catalogued("render-unpinned", TABLES));
    assert!(naming(&unpinned), "{unpinned:?}");
    let pinned = refused(&catalogued(
        "render-pinned",
        &format!(
            "{TABLES}\n{}",
            fence("ghcr.io/lemonfiber/fence", "0.18.0", &digest)
        ),
    ));
    assert!(!naming(&pinned), "{pinned:?}");
}
