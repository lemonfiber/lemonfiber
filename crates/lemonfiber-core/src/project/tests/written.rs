use serde_norway::Value as Yaml;

use super::super::{generated, Project, Reading};
use super::{carried, entry, environment, project};
use crate::plugin::Installed;
use crate::wiring::Chosen;

#[test]
fn a_part_installed_as_a_third_party_plugin_writes_every_field_as_it_does_first_party() {
    let parts = crate::test_support::adapted::installed_parts();
    let foreign: Vec<Installed> = parts
        .iter()
        .cloned()
        .map(|mut part| {
            part.tree = format!("{}-elsewhere", part.tree);
            part
        })
        .collect();
    let mut manifest = carried();
    manifest.services.retain(|service| {
        !parts
            .iter()
            .flat_map(|part| &part.services)
            .any(|placed| placed.service == service.id)
    });
    let first = project(&manifest, &parts);
    let third = project(&manifest, &foreign);
    let compared: Vec<&str> = parts
        .iter()
        .flat_map(|part| &part.services)
        .map(|placed| placed.service.as_str())
        .filter(|service| !entry(&third, service).is_null())
        .collect();
    assert!(!compared.is_empty());
    let declared = |project: &Project, service: &str| {
        let mut written = entry(project, service);
        if let Some(fields) = written.as_mapping_mut() {
            fields.remove("profiles");
            fields.remove("networks");
        }
        written
    };
    for service in compared {
        assert_eq!(
            declared(&first, service),
            declared(&third, service),
            "{service}"
        );
    }
}

#[test]
fn a_stack_setting_holding_a_dollar_is_written_so_compose_substitutes_nothing() {
    let setting = |name: &str| (name == "flaresolverr:log-level").then(|| "a$b${HOME}".to_owned());
    let reading = Reading {
        setting: &setting,
        ..Reading::bare()
    };
    let project = generated(&carried(), &[], &Chosen::read(None), &reading);
    assert_eq!(
        environment(&entry(&project, "flaresolverr"), "LOG_LEVEL"),
        "a$$b$${HOME}"
    );
}

#[test]
fn only_a_declared_memory_ceiling_is_written_as_a_limit_and_an_estimate_never_is() {
    let manifest = carried();
    let project = project(&manifest, &[]);
    let written: Vec<_> = manifest
        .services
        .iter()
        .map(|service| (service, entry(&project, &service.id)))
        .filter(|(_, written)| !written.is_null())
        .collect();
    assert!(written
        .iter()
        .any(|(service, _)| { service.memory_mib.is_some() && service.mem_limit_mib.is_none() }));
    assert!(written
        .iter()
        .any(|(service, _)| service.mem_limit_mib.is_some()));
    for (service, written) in written {
        assert_eq!(
            written
                .get("mem_limit")
                .and_then(Yaml::as_str)
                .map(str::to_owned),
            service.mem_limit_mib.map(|mib| format!("{mib}m")),
            "{}",
            service.id
        );
    }
}
