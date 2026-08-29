//! Checking a manifest against the contract.
//!
//! Every violation is reported in one pass, each naming where it is. Stopping at
//! the first turns fixing a fork into a guessing game: change one line, run
//! again, find the next, repeat — when the whole list was knowable at once.
//!
//! Validation is separate from parsing because they fail for different reasons
//! and deserve different answers. A file that is not a manifest is a syntax
//! error; a manifest that contradicts itself parsed perfectly well.

use std::collections::{BTreeMap, BTreeSet};

use crate::{ApiKind, Date, Manifest, Protocol, Service};

/// Tags that move under you. A pin meaning "whatever is newest" is not a pin.
const FLOATING_TAGS: &[&str] = &[
    "latest", "stable", "edge", "nightly", "develop", "dev", "main", "master", "rolling",
];

/// Kernel capabilities a service may ask for.
///
/// Deliberately one entry. A capability is a hole in the isolation the stack
/// otherwise relies on, and the tunnel genuinely needs this one to build an
/// interface. Anything else should have to argue for itself in a spec change.
const ALLOWED_CAPABILITIES: &[&str] = &["NET_ADMIN"];

/// The OSI-approved identifiers a service licence may use.
const OSI: &str = include_str!("spdx_osi.txt");

/// One thing wrong with a manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    /// Where it is, in the manifest's own terms — `service sonarr`, `form tv`.
    pub location: String,
    /// What is wrong, in one line.
    pub message: String,
}

impl std::fmt::Display for Violation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.location, self.message)
    }
}

/// Check a manifest against the contract, reporting everything wrong with it.
///
/// `today` is supplied rather than read so the result depends only on its
/// inputs — a validator that consults the clock passes and fails on different
/// days for the same file.
#[must_use]
pub fn validate(manifest: &Manifest, today: Date) -> Vec<Violation> {
    let mut found = Vec::new();
    let profiles = check_profiles(manifest, &mut found);
    check_forms(manifest, &profiles, &mut found);
    check_services(manifest, &profiles, today, &mut found);
    found
}

/// Profile ids must be unique, and each protocol claimed at most once.
fn check_profiles(manifest: &Manifest, found: &mut Vec<Violation>) -> BTreeSet<String> {
    let mut declared = BTreeSet::new();
    let mut claimed: BTreeMap<Protocol, String> = BTreeMap::new();

    for profile in &manifest.profiles {
        let location = format!("profile {}", profile.id);
        if !declared.insert(profile.id.clone()) {
            found.push(Violation {
                location: location.clone(),
                message: "another profile already has this id".to_owned(),
            });
        }
        if let Some(protocol) = profile.protocol {
            if let Some(owner) = claimed.get(&protocol) {
                found.push(Violation {
                    location,
                    message: format!("profile {owner} already carries this protocol"),
                });
            } else {
                claimed.insert(protocol, profile.id.clone());
            }
        }
    }
    declared
}

/// Form ids must be unique, and every profile a form names must exist.
fn check_forms(manifest: &Manifest, profiles: &BTreeSet<String>, found: &mut Vec<Violation>) {
    let mut declared = BTreeSet::new();
    for form in &manifest.forms {
        let location = format!("form {}", form.id);
        if !declared.insert(form.id.clone()) {
            found.push(Violation {
                location: location.clone(),
                message: "another form already has this id".to_owned(),
            });
        }
        for named in &form.profiles {
            if !profiles.contains(named) {
                found.push(Violation {
                    location: location.clone(),
                    message: format!("names profile {named}, which is not declared"),
                });
            }
        }
    }
}

/// Everything a service has to get right.
///
/// One rule per function, and the order they are chained in is the order an
/// operator reads them. Each answers for itself and returns what it found, so
/// adding a rule is adding a link rather than editing a body that already holds
/// eight others.
fn check_services(
    manifest: &Manifest,
    profiles: &BTreeSet<String>,
    today: Date,
    found: &mut Vec<Violation>,
) {
    let osi: BTreeSet<&str> = OSI
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect();

    let of_service: BTreeMap<&str, &str> = manifest
        .services
        .iter()
        .map(|service| (service.id.as_str(), service.profile.as_str()))
        .collect();

    let mut declared = BTreeSet::new();
    for service in &manifest.services {
        let repeated = (!declared.insert(service.id.clone()))
            .then(|| "another service already has this id".to_owned());

        let faults = repeated
            .into_iter()
            .chain(placed(service, profiles))
            .chain(pinned(service))
            .chain(published(service))
            .chain(licensed(service, &osi))
            .chain(released(service, today))
            .chain(permitted(service))
            .chain(versioned(service))
            .chain(depended(service, &of_service));

        let location = format!("service {}", service.id);
        found.extend(faults.map(|message| Violation {
            location: location.clone(),
            message,
        }));
    }
}

/// A Servarr-shape service names the version of the API its client speaks.
///
/// The shape spans two — Sonarr and Radarr at v3, Lidarr and Prowlarr at v1 — so
/// the version cannot be assumed. A servarr service that omits it is a fault
/// surfaced here, rather than one that silently drops the service from seeding
/// and the doctor because it cannot be reached at a known path.
fn versioned(service: &Service) -> Option<String> {
    let api = service.api.as_ref()?;
    (api.kind == ApiKind::Servarr && api.version.is_none())
        .then(|| "has the servarr API shape but names no api.version".to_owned())
}

/// A service belongs to a profile the stack declares.
fn placed(service: &Service, profiles: &BTreeSet<String>) -> Option<String> {
    (!profiles.contains(&service.profile))
        .then(|| format!("is in profile {}, which is not declared", service.profile))
}

/// A service names a version rather than a tag that moves under it.
fn pinned(service: &Service) -> Option<String> {
    if service.tag.is_empty() {
        return Some("declares an empty image tag".to_owned());
    }
    FLOATING_TAGS.contains(&service.tag.as_str()).then(|| {
        format!(
            "is pinned to {}, which moves — that is not a pin",
            service.tag
        )
    })
}

/// A service that publishes a port says which interface it publishes on.
fn published(service: &Service) -> Option<String> {
    (service.port.is_some() && service.bind.is_none())
        .then(|| "publishes a port and does not say which interface".to_owned())
}

/// A service declares a licence anyone can look up.
fn licensed(service: &Service, osi: &BTreeSet<&str>) -> Option<String> {
    (!osi.contains(service.license.as_str())).then(|| {
        format!(
            "declares licence {}, which is not a recognised OSI identifier",
            service.license
        )
    })
}

/// A service records a release date that is a date, and has happened.
fn released(service: &Service, today: Date) -> Option<String> {
    match Date::parse(&service.last_release) {
        None => Some(format!(
            "records last_release {}, which is not YYYY-MM-DD",
            service.last_release
        )),
        Some(recorded) if recorded > today => Some(format!(
            "records last_release {}, which is in the future",
            service.last_release
        )),
        Some(_) => None,
    }
}

/// A service asks only for capabilities the stack is willing to grant.
fn permitted(service: &Service) -> Vec<String> {
    service
        .capabilities
        .iter()
        .filter(|capability| !ALLOWED_CAPABILITIES.contains(&capability.as_str()))
        .map(|capability| format!("asks for capability {capability}, which is not allowed"))
        .collect()
}

/// A service waits only on things that will be running when it is.
///
/// Dependencies are allowed to exist; crossing a profile is what is not. A
/// service waiting on something that may not be running is a start-up that
/// hangs for a reason nothing reports.
fn depended(service: &Service, of_service: &BTreeMap<&str, &str>) -> Vec<String> {
    service
        .depends_on
        .iter()
        .filter_map(|needed| match of_service.get(needed.as_str()) {
            None => Some(format!("depends on {needed}, which is not a service here")),
            Some(other) if *other != service.profile => Some(format!(
                "depends on {needed}, which is in profile {other} rather than {}",
                service.profile
            )),
            Some(_) => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{validate, Date, Violation};
    use crate::Manifest;

    const STACK: &str = include_str!("../../../assets/media-stack/stack.toml");

    /// After every date the stack records, and not the real clock.
    ///
    /// A service records the day its upstream last released, and a date after this
    /// one is a fault — so bumping a pin to a newer release puts the stack ahead of
    /// this and the fixture reports a fault that is nobody's mistake.
    /// `the_day_these_tests_use_is_after_everything_the_stack_records` is the guard;
    /// when it fails, move this forward rather than pinning an older image.
    const TODAY: Date = Date {
        year: 2026,
        month: 10,
        day: 1,
    };

    fn check(text: &str) -> Vec<Violation> {
        Manifest::from_toml(text)
            .ok()
            .map(|manifest| validate(&manifest, TODAY))
            .unwrap_or_default()
    }

    fn messages(text: &str) -> Vec<String> {
        check(text).iter().map(ToString::to_string).collect()
    }

    /// Replace the first occurrence of `from` with `to`.
    fn edited(from: &str, to: &str) -> String {
        assert!(STACK.contains(from), "the fixture must contain {from:?}");
        STACK.replacen(from, to, 1)
    }

    /// The day these tests use is after everything the stack records.
    ///
    /// Every fixture here validates the committed stack against `TODAY`, and the
    /// stack is a submodule that moves forward while this constant does not. Without
    /// this, a pin bump makes `the_stack_this_binary_ships_is_valid` fail for a
    /// reason that has nothing to do with the stack being invalid.
    #[test]
    fn the_day_these_tests_use_is_after_everything_the_stack_records() {
        let latest = STACK
            .lines()
            .filter_map(|line| line.trim().strip_prefix("last_release = "))
            .map(|value| value.trim().trim_matches('"').to_owned())
            .max()
            .unwrap_or_default();
        let said = format!("{:04}-{:02}-{:02}", TODAY.year, TODAY.month, TODAY.day);

        assert!(
            said.as_str() >= latest.as_str(),
            "these tests validate against {said} and the stack records a release on \
             {latest}; move TODAY forward past it — the fixtures report a fault that \
             is nobody's mistake otherwise."
        );
    }

    #[test]
    fn the_stack_this_binary_ships_is_valid() {
        assert_eq!(
            check(STACK),
            Vec::new(),
            "the committed stack has no faults"
        );
    }

    #[test]
    fn a_servarr_service_without_an_api_version_is_caught() {
        // The servarr shape spans two API versions, so one omitting it cannot be
        // reached at a known path; dropping the first version the stack records
        // (Sonarr's v3) must be a surfaced fault, not a silent exclusion.
        let text = edited(
            r#"path = "/config/config.xml", version = 3 }"#,
            r#"path = "/config/config.xml" }"#,
        );
        let caught = messages(&text)
            .iter()
            .any(|message| message.contains("names no api.version"));
        assert!(caught);
    }

    #[test]
    fn a_duplicate_profile_id_is_caught() {
        let text = edited(r#"id = "usenet""#, r#"id = "search""#);
        assert!(messages(&text)
            .iter()
            .any(|m| m.contains("another profile already has this id")));
    }

    #[test]
    fn two_profiles_claiming_one_protocol_is_caught() {
        let text = edited(r#"protocol = "usenet""#, r#"protocol = "torrent""#);
        assert!(messages(&text)
            .iter()
            .any(|m| m.contains("already carries this protocol")));
    }

    #[test]
    fn a_form_naming_an_undeclared_profile_is_caught() {
        let text = edited(r#"profiles = ["search"]"#, r#"profiles = ["telly"]"#);
        assert!(messages(&text)
            .iter()
            .any(|m| m.contains("names profile telly, which is not declared")));
    }

    #[test]
    fn a_service_in_an_undeclared_profile_is_caught() {
        let text = edited(r#"profile = "search""#, r#"profile = "telly""#);
        assert!(messages(&text)
            .iter()
            .any(|m| m.contains("is in profile telly, which is not declared")));
    }

    #[test]
    fn a_floating_tag_is_caught() {
        for floating in ["latest", "stable", "nightly", "main"] {
            let text = STACK.replacen("tag = ", &format!("tag = \"{floating}\" # "), 1);
            assert!(
                messages(&text)
                    .iter()
                    .any(|m| m.contains("that is not a pin")),
                "{floating} should not be accepted as a pin"
            );
        }
    }

    #[test]
    fn an_empty_tag_is_caught() {
        let text = STACK.replacen("tag = ", "tag = \"\" # ", 1);
        assert!(
            messages(&text)
                .iter()
                .any(|m| m.contains("empty image tag")),
            "an empty tag is a broken image reference, not a pin"
        );
    }

    #[test]
    fn a_published_port_with_no_binding_is_caught() {
        let text = edited("bind = \"loopback\"\n", "");
        assert!(messages(&text)
            .iter()
            .any(|m| m.contains("does not say which interface")));
    }

    #[test]
    fn a_licence_outside_the_osi_list_is_caught() {
        let text = edited(r#"license = "GPL-3.0-only""#, r#"license = "Nonesuch-1.0""#);
        assert!(messages(&text)
            .iter()
            .any(|m| m.contains("not a recognised OSI identifier")));
    }

    #[test]
    fn a_malformed_last_release_is_caught() {
        let text = STACK.replacen("last_release = ", "last_release = \"26-07-01\" # ", 1);
        assert!(messages(&text)
            .iter()
            .any(|m| m.contains("which is not YYYY-MM-DD")));
    }

    #[test]
    fn a_last_release_in_the_future_is_caught() {
        let text = STACK.replacen("last_release = ", "last_release = \"2099-01-01\" # ", 1);
        assert!(messages(&text)
            .iter()
            .any(|m| m.contains("which is in the future")));
    }

    #[test]
    fn a_capability_outside_the_allow_list_is_caught() {
        let text = edited(
            r#"capabilities = ["NET_ADMIN"]"#,
            r#"capabilities = ["SYS_ADMIN"]"#,
        );
        assert!(messages(&text)
            .iter()
            .any(|m| m.contains("capability SYS_ADMIN, which is not allowed")));
    }

    #[test]
    fn a_dependency_across_a_profile_boundary_is_caught() {
        let text = edited(
            r#"depends_on = ["gluetun"]"#,
            r#"depends_on = ["prowlarr"]"#,
        );
        assert!(
            messages(&text)
                .iter()
                .any(|m| m.contains("which is in profile search rather than torrent")),
            "a dependency across a profile boundary must be named"
        );
    }

    #[test]
    fn a_dependency_on_nothing_at_all_is_caught() {
        let text = edited(r#"depends_on = ["gluetun"]"#, r#"depends_on = ["ghost"]"#);
        assert!(messages(&text)
            .iter()
            .any(|m| m.contains("depends on ghost, which is not a service here")));
    }

    #[test]
    fn every_violation_is_reported_rather_than_only_the_first() {
        let text = edited(r#"license = "GPL-3.0-only""#, r#"license = "Nonesuch-1.0""#).replacen(
            "last_release = ",
            "last_release = \"2099-01-01\" # ",
            1,
        );
        let reported = messages(&text);
        assert!(
            reported.len() >= 2,
            "fixing a fork should not be a guessing game: {reported:?}"
        );
    }

    #[test]
    fn every_violation_says_where_it_is() {
        let text = edited(r#"license = "GPL-3.0-only""#, r#"license = "Nonesuch-1.0""#);
        assert!(check(&text)
            .iter()
            .all(|violation| !violation.location.is_empty()));
    }

    #[test]
    fn a_duplicate_form_id_is_caught() {
        let text = edited(r#"id = "dl""#, r#"id = "search""#);
        assert!(messages(&text)
            .iter()
            .any(|m| m.contains("another form already has this id")));
    }

    #[test]
    fn a_duplicate_service_id_is_caught() {
        let text = edited(r#"id = "sonarr""#, r#"id = "prowlarr""#);
        assert!(
            messages(&text)
                .iter()
                .any(|m| m.contains("another service already has this id")),
            "a repeated service id must be named"
        );
    }
}
