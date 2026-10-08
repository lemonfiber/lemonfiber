use super::{manifest_into, manifest_without, manifest_written};
use crate::scratch::Scratch;

#[test]
fn a_copy_reads_back_as_the_carried_stack() {
    let into = Scratch::named("fixtures-stack-copy").kept();
    manifest_into(&into);
    let read = lemonfiber_manifest::read(&into).ok();
    assert_eq!(
        read,
        lemonfiber_manifest::read(std::path::Path::new(super::CARRIED)).ok()
    );
    assert!(read.is_some());
    let _ = std::fs::remove_dir_all(&into);
}

#[test]
fn a_service_taken_out_takes_its_file_its_entry_and_its_links() {
    let into = Scratch::named("fixtures-stack-without").kept();
    manifest_without(&into, "decline");
    let text = lemonfiber_manifest::read(&into).unwrap_or_default();
    let manifest = lemonfiber_manifest::Manifest::from_toml(&text).ok();
    assert!(manifest
        .iter()
        .flat_map(|one| &one.services)
        .all(|service| service.id != "decline"));
    assert!(manifest.is_some_and(|one| one.wirings.iter().all(|wiring| wiring.by != "decline")));
    let _ = std::fs::remove_dir_all(&into);
}

#[test]
fn a_manifest_in_one_text_is_laid_out_a_file_per_service() {
    let into = Scratch::named("fixtures-stack-written").kept();
    let _ = std::fs::remove_dir_all(&into);
    manifest_written(
        &into,
        "schema_version = 1\n\n[[service]]\nid = \"a\"\n\n[service.api]\nkind = \"x\"\n\n[[wiring]]\nby = \"a\"\n\n[[service]]\nname = \"b\"\n",
    );
    let root = std::fs::read_to_string(into.join("stack.toml")).unwrap_or_default();
    assert!(
        root.starts_with(
            "include = [\"services/a.toml\", \"services/service-1.toml\"]\nschema_version = 1"
        ),
        "{root}"
    );
    assert!(
        root.contains("[[wiring]]") && !root.contains("[[service]]"),
        "{root}"
    );
    let first = std::fs::read_to_string(into.join("services/a.toml")).unwrap_or_default();
    assert!(first.contains("[service.api]"), "{first}");
    assert!(into.join("services/service-1.toml").is_file());
    let _ = std::fs::remove_dir_all(&into);
}

#[test]
fn a_manifest_with_no_services_is_written_as_it_is() {
    let into = Scratch::named("fixtures-stack-bare").kept();
    let _ = std::fs::remove_dir_all(&into);
    manifest_written(&into, "version = \n");
    assert_eq!(
        std::fs::read_to_string(into.join("stack.toml"))
            .ok()
            .as_deref(),
        Some("version = \n")
    );
    let _ = std::fs::remove_dir_all(&into);
}
