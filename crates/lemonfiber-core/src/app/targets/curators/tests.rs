use super::{curator_targets, target_named};
use crate::test_support::curators::{asking, TELEVISION};

/// One service, read back through the manifest parser rather than built as a
/// struct, so what this test calls a declaration is what the stack's own file
/// means by one.
///
/// A list of one rather than the service itself, and the assertion below is why:
/// a parse that failed would otherwise need a way out of its own, and a block no
/// run enters is a line the coverage gate counts against every honest one beside
/// it. Said here rather than left to the cases, because most of them assert that
/// nothing is reported — which an empty list satisfies while proving nothing.
fn service(id: &str, port: Option<u16>) -> Vec<lemonfiber_manifest::Service> {
    let published = port.map_or_else(String::new, |port| format!("port = {port}\n"));
    let written = format!(
        "schema_version = 1\nstack_version = \"0.1.0\"\nmin_cli_version = \"0.1.0\"\n\n\
         [[profile]]\nid = \"tv\"\nname = \"Television\"\ndescription = \"Television\"\n\n\
         [[service]]\nid = \"{id}\"\nname = \"{id}\"\nprofile = \"tv\"\n\
         image = \"example/{id}\"\ntag = \"1.0.0\"\n{published}\
         criticality = \"core\"\nlicense = \"GPL-3.0-only\"\n\
         upstream = \"https://example.invalid/{id}\"\nlast_release = \"2026-01-01\"\n\
         describes = \"Does a thing\"\nwithout_it = \"Do the thing yourself\"\n"
    );
    let read: Vec<_> = lemonfiber_manifest::Manifest::from_toml(&written)
        .ok()
        .map(|manifest| manifest.services)
        .unwrap_or_default();
    assert_eq!(
        read.len(),
        1,
        "a manifest this test wrote is one the parser reads: {written}"
    );
    read
}

/// The fillers `services` come to, with nothing installed beside them.
fn alone(services: Vec<lemonfiber_manifest::Service>) -> crate::wiring::Fillers {
    crate::test_support::stack_fillers(services, &[], None, &[])
}

/// The shipped stack, written to disk, with its television curator moved into its plugin.
fn television_moved() -> crate::wiring::Fillers {
    crate::test_support::stack()
        .manifest()
        .map(|manifest| {
            crate::wiring::Fillers::of(
                &manifest,
                &TELEVISION.installed().into_iter().collect::<Vec<_>>(),
                &crate::wiring::Chosen::default(),
                Some(std::path::Path::new("/opt/lemonfiber/stack")),
            )
        })
        .unwrap_or_default()
}

/// A moved curator is listed under its own name with the adapter it is asked through,
/// and listed still where that adapter cannot be asked.
#[tokio::test]
async fn a_moved_curator_is_listed_whether_or_not_its_adapter_can_be_asked() {
    let fillers = television_moved();
    let unasked_ctx = crate::test_support::context::a_context().build();
    let asked_ctx = asking(
        crate::test_support::context::a_context().build(),
        &[TELEVISION],
    );

    let silent = target_named(&unasked_ctx, &fillers, TELEVISION.plugin).await;
    let reached = target_named(&asked_ctx, &fillers, TELEVISION.plugin).await;

    assert_eq!(
        silent
            .as_ref()
            .map(|target| (target.name.as_str(), target.asked.is_some())),
        Some(("Sonarr", false))
    );
    assert_eq!(
        reached.map(|target| (target.kind, target.asked.is_some())),
        Some((Some(crate::ports::media::Kind::Tv), true))
    );
    assert!(
        curator_targets(&asked_ctx, &alone(service("theirs", Some(8989))))
            .await
            .is_empty()
    );
}
