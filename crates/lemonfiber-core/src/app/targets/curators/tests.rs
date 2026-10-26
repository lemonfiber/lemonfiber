use super::{curator_targets, target_named, unasked};
use crate::test_support::curators::{asking, TELEVISION};

/// One service, read back through the manifest parser rather than built as a
/// struct, so what this test calls a declaration is what the stack's own file
/// means by one.
///
/// `api` is the whole of what varies, written out by the caller: the cases here
/// are all about which part of a declaration is missing.
/// A list of one rather than the service itself, and the assertion below is why:
/// a parse that failed would otherwise need a way out of its own, and a block no
/// run enters is a line the coverage gate counts against every honest one beside
/// it. Said here rather than left to the cases, because most of them assert that
/// nothing is reported — which an empty list satisfies while proving nothing.
fn service(id: &str, port: Option<u16>, api: &str) -> Vec<lemonfiber_manifest::Service> {
    let published = port.map_or_else(String::new, |port| format!("port = {port}\n"));
    let written = format!(
        "schema_version = 1\nstack_version = \"0.1.0\"\nmin_cli_version = \"0.1.0\"\n\n\
         [[profile]]\nid = \"tv\"\nname = \"Television\"\ndescription = \"Television\"\n\n\
         [[service]]\nid = \"{id}\"\nname = \"{id}\"\nprofile = \"tv\"\n\
         image = \"example/{id}\"\ntag = \"1.0.0\"\n{published}\
         criticality = \"core\"\nlicense = \"GPL-3.0-only\"\n\
         upstream = \"https://example.invalid/{id}\"\nlast_release = \"2026-01-01\"\n\
         describes = \"Does a thing\"\nwithout_it = \"Do the thing yourself\"\n{api}"
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

/// A curator declaration.
const WHOLE: &str =
    "\n[service.api]\nkind = \"servarr\"\nkey_source = \"config-xml\"\npath = \"/config/config.xml\"\nversion = 3\n";

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

/// The service is named by the id the stack declares it under, with why nothing here
/// asks it, so an operator can go and find the declaration.
#[test]
fn a_stack_service_declaring_a_native_api_is_named_with_why_nothing_asks_it() {
    let reports = unasked(&alone(service("theirs", Some(8989), WHOLE)));

    let [report] = reports.as_slice() else {
        unreachable!("one report was not made: {reports:?}");
    };
    assert_eq!(report.what, "theirs");
    assert!(report.because.contains("servarr API"), "{}", report.because);
    assert!(report.because.contains("adapter"), "{}", report.because);
}

/// The manifest saying nothing is the stack's own statement that there is nothing
/// to integrate with, which is not lemonfiber failing to do something.
#[test]
fn a_service_declaring_no_api_at_all_is_not_reported_as_unsupported() {
    assert!(unasked(&alone(service("caddy", Some(80), ""))).is_empty());
}

/// A first-party plugin's service moved in place of the stack's is asked through the
/// adapter in front of it, so it is not reported.
#[test]
fn a_service_moved_into_its_plugin_is_not_reported() {
    let reports = unasked(&television_moved());

    assert!(
        reports
            .iter()
            .all(|report| report.what != TELEVISION.plugin),
        "{reports:?}"
    );
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
        curator_targets(&asked_ctx, &alone(service("theirs", Some(8989), WHOLE)))
            .await
            .is_empty()
    );
}
