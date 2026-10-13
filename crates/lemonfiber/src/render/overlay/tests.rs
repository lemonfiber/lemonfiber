use lemonfiber_core::model::Unvetted;

use super::unvetted;

#[test]
fn the_overlay_is_named_with_every_service_it_changes_and_nothing_without_one() {
    assert!(unvetted(None).text().is_empty());
    let said = unvetted(Some(&Unvetted {
        overlay: "/home/op/.config/lemonfiber/overlay.yml".to_owned(),
        services: vec!["sonarr".to_owned(), "radarr".to_owned()],
    }))
    .text();
    assert!(
        said.contains("Unvetted: your overlay /home/op/.config/lemonfiber/overlay.yml"),
        "{said}"
    );
    assert!(said.contains("changes sonarr, radarr"), "{said}");
    let none = unvetted(Some(&Unvetted {
        overlay: "overlay.yml".to_owned(),
        services: Vec::new(),
    }))
    .text();
    assert!(none.contains("changes no service by name"), "{none}");
}

#[test]
fn a_lifecycle_run_names_the_overlay_it_starts_the_project_under() {
    let mut report = crate::render::fixtures::a_lifecycle(
        "up",
        crate::render::fixtures::a_plan("library", Vec::new()),
    );
    let plain = crate::render::stack::lifecycle(&report).text();
    assert!(!plain.contains("Unvetted"), "{plain}");
    report.unvetted = Some(Unvetted {
        overlay: "overlay.yml".to_owned(),
        services: vec!["sonarr".to_owned()],
    });
    let said = crate::render::stack::lifecycle(&report).text();
    assert!(
        said.contains("Unvetted: your overlay overlay.yml"),
        "{said}"
    );
}
