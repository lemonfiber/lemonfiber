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

use crate::{ApiKind, Date, Manifest, Protocol, Removed, Service};

/// Tags that move under you. A pin meaning "whatever is newest" is not a pin.
const FLOATING_TAGS: &[&str] = &[
    "latest", "stable", "edge", "nightly", "develop", "dev", "main", "master", "rolling",
];

/// The kernel capabilities a service may be granted.
///
/// Deliberately one entry. A kernel capability is a hole in the isolation the
/// stack otherwise relies on, and the tunnel genuinely needs this one to build
/// an interface. Anything else should have to argue for itself in a spec change.
const ALLOWED_GRANTS: &[&str] = &["NET_ADMIN"];

/// The OSI-approved identifiers a service licence may use.
const OSI: &str = include_str!("spdx_osi.txt");

/// The identifiers a vendored list holds, ignoring the prose it explains itself with.
fn identifiers(list: &str) -> BTreeSet<&str> {
    list.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect()
}

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
    check_removed(manifest, &mut found);
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
    let osi = identifiers(OSI);

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
            .chain(outbound(service))
            .chain(depended(service, &of_service));

        let location = format!("service {}", service.id);
        found.extend(faults.map(|message| Violation {
            location: location.clone(),
            message,
        }));
    }
}

/// Everything a record of a dropped service has to get right.
///
/// Chained the way a service's rules are, and for the same reason: a fork that
/// recorded a removal badly should be told everything about it at once rather than
/// once per run.
fn check_removed(manifest: &Manifest, found: &mut Vec<Violation>) {
    let declared: BTreeSet<&str> = manifest
        .services
        .iter()
        .map(|service| service.id.as_str())
        .collect();
    let recorded: BTreeSet<&str> = manifest
        .removed
        .iter()
        .map(|removed| removed.id.as_str())
        .collect();

    let mut seen = BTreeSet::new();
    for removed in &manifest.removed {
        let repeated = (!seen.insert(removed.id.clone()))
            .then(|| "another removal already has this id".to_owned());

        let faults = repeated
            .into_iter()
            .chain(gone(removed, &declared))
            .chain(dated(removed))
            .chain(explained(removed))
            .chain(replaced(removed, &declared, &recorded));

        let location = format!("removed {}", removed.id);
        found.extend(faults.map(|message| Violation {
            location: location.clone(),
            message,
        }));
    }
}

/// A service recorded as removed is not also declared.
///
/// The two records contradict each other outright, and the contradiction is silent
/// where it matters most: an operator asking what became of a service would be told
/// it went, while the stack goes on starting it.
fn gone(removed: &Removed, declared: &BTreeSet<&str>) -> Option<String> {
    declared
        .contains(removed.id.as_str())
        .then(|| "is recorded as removed and is still declared as a service".to_owned())
}

/// A removal says which stack version stopped carrying it.
///
/// Emptiness rather than shape. The manifest validates neither `stack_version` nor
/// `min_cli_version` as semantic versions, and a rule here that parsed one would be
/// stricter about the past than the contract is about the present — so what is checked
/// is that the record names something, since a record that cannot be placed in the
/// stack's own history answers *when did this go* with nothing.
fn dated(removed: &Removed) -> Option<String> {
    removed
        .removed_in
        .trim()
        .is_empty()
        .then(|| "records no stack version it went in".to_owned())
}

/// A removal says why, in something more than an empty string.
///
/// Emptiness rather than presence, because presence is what the parse already
/// guarantees and an empty reason is the shape a record takes when somebody filled
/// the table in to satisfy it. The requirement is that the reason is recorded; a
/// field holding nothing records nothing.
fn explained(removed: &Removed) -> Option<String> {
    removed
        .reason
        .trim()
        .is_empty()
        .then(|| "records no reason for going".to_owned())
}

/// A replacement, where one is named, is something this stack knows about.
///
/// Either a service it declares or another service it recorded as removed — the
/// second because replacements chain, and a stack that dropped the thing that
/// replaced the thing it dropped has told the truth twice. A name that is neither
/// points an operator at nothing, which is worse than recording no replacement at
/// all, since it reads as an answer.
fn replaced(
    removed: &Removed,
    declared: &BTreeSet<&str>,
    recorded: &BTreeSet<&str>,
) -> Option<String> {
    let named = removed.replaced_by.as_deref()?;
    if named.trim().is_empty() {
        return Some("names an empty replacement rather than none at all".to_owned());
    }
    if named == removed.id {
        return Some("is recorded as having replaced itself".to_owned());
    }
    (!declared.contains(named) && !recorded.contains(named)).then(|| {
        format!("says {named} replaced it, and this stack neither declares nor records that")
    })
}

/// A service that says where it reaches says what it asks for there, and the
/// other way round.
///
/// Half of that pair is not a smaller answer, it is a misleading one. An inventory
/// of what leaves a machine reads an empty destination as "this service reaches
/// nothing", so a stack that named a purpose and no destination would have that
/// purpose attributed to a service the same report says goes nowhere. Refused by
/// name here rather than papered over at the point of reading, so whoever wrote the
/// manifest is the one who decides which half was meant.
fn outbound(service: &Service) -> Option<String> {
    match (&service.reaches, &service.asks_for) {
        (Some(_), None) => Some("says where it reaches and not what it asks for".to_owned()),
        (None, Some(_)) => Some("says what it asks for and not where it reaches".to_owned()),
        (Some(_), Some(_)) | (None, None) => None,
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

/// A service asks only for the kernel capabilities the stack is willing to grant.
fn permitted(service: &Service) -> Vec<String> {
    service
        .grants
        .iter()
        .filter(|granted| !ALLOWED_GRANTS.contains(&granted.as_str()))
        .map(|granted| format!("asks for kernel capability {granted}, which is not allowed"))
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
    use super::{identifiers, validate, Date, Violation, OSI};
    use crate::Manifest;

    /// The stack's own copy of the same list, which this one follows.
    const THEIRS: &str = include_str!("../../../assets/media-stack/scripts/spdx_osi.txt");

    /// Two copies of one list, and a sentence at the top of ours saying they agree.
    ///
    /// They do today. Nothing made them, and the drift is silent in the direction it
    /// would actually happen: the stack adds a licence, ships a service under it, and
    /// this validator refuses a manifest the stack's own validator accepted — with an
    /// error naming the licence, which is the one thing that is not wrong.
    ///
    /// The submodule is pinned, so this asks at the moment it is worth asking: when
    /// the stack this binary carries is moved forward.
    #[test]
    fn the_licence_list_holds_what_the_stack_s_own_copy_holds() {
        let ours = identifiers(OSI);
        let theirs = identifiers(THEIRS);
        let only_ours: Vec<&str> = ours.difference(&theirs).copied().collect();
        let only_theirs: Vec<&str> = theirs.difference(&ours).copied().collect();
        assert!(
            only_ours.is_empty() && only_theirs.is_empty(),
            "the two vendored licence lists have drifted, and the stack's copy is the \
             one a maintainer edits: here and not there {only_ours:?}, there and not \
             here {only_theirs:?}"
        );
    }

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
    fn a_grant_outside_the_allow_list_is_caught() {
        let text = edited(r#"grants = ["NET_ADMIN"]"#, r#"grants = ["SYS_ADMIN"]"#);
        assert!(messages(&text)
            .iter()
            .any(|m| m.contains("kernel capability SYS_ADMIN, which is not allowed")));
    }

    /// The line every service in the shipped stack has, which these cases add to.
    const AN_ID: &str = "id = \"prowlarr\"";

    /// The first of the two outbound lines the shipped stack carries, and the second.
    ///
    /// Read out of the stack rather than written here, so a stack that rewords either
    /// of them is a test that fails loudly instead of one quietly proving nothing.
    const REACHES: &str = "reaches = ";
    const ASKS_FOR: &str = "asks_for = ";

    /// The stack with the first line starting `prefix` removed from it.
    fn without(prefix: &str) -> String {
        let at = STACK
            .lines()
            .position(|line| line.starts_with(prefix))
            .unwrap_or_default();
        assert!(at > 0, "the fixture must declare {prefix:?}");
        STACK
            .lines()
            .enumerate()
            .filter_map(|(number, line)| (number != at).then_some(line))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Half a declaration is a misleading answer rather than a smaller one, so it is
    /// refused in either direction.
    ///
    /// Made by taking a half away rather than by adding one. Every service the stack
    /// ships now declares both, so a case built by adding `reaches` to one of them
    /// would be a second copy of a key the entry already has — which the parser
    /// refuses before this rule is ever asked.
    #[test]
    fn declaring_where_a_service_reaches_without_what_it_asks_for_is_caught() {
        let said = messages(&without(ASKS_FOR));
        assert!(
            said.iter().any(|fault| fault.contains("what it asks for")),
            "{said:?}"
        );
    }

    #[test]
    fn declaring_what_a_service_asks_for_without_where_it_reaches_is_caught() {
        let said = messages(&without(REACHES));
        assert!(
            said.iter().any(|fault| fault.contains("where it reaches")),
            "{said:?}"
        );
    }

    /// Both, or neither, and neither is what every stack written before this says.
    #[test]
    fn a_service_that_declares_both_or_neither_says_nothing_is_wrong() {
        let both = messages(&edited(
            AN_ID,
            "id = \"prowlarr\"\nreaches = \"the indexers you configured\"\n\
             asks_for = \"Runs the searches\"",
        ));
        assert!(both.is_empty(), "{both:?}");
        // And an empty destination is an answer rather than an absence: a service
        // that reaches nothing says so, with what it does instead.
        let quiet = messages(&edited(
            AN_ID,
            "id = \"prowlarr\"\nreaches = \"\"\nasks_for = \"Nothing leaves this machine\"",
        ));
        assert!(quiet.is_empty(), "{quiet:?}");
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

    /// One removal recorded against the shipped stack, which declares the service it
    /// names as a replacement.
    ///
    /// Named for nothing that ever existed, on purpose. The stack is a submodule that
    /// moves, and it is about to start recording removals of its own — an id borrowed
    /// from a real one would collide with the stack's own entry the day its pin moves,
    /// and these tests would report a duplicate that is nobody's mistake.
    const DROPPED: &str = r#"
[[removed]]
id = "an-old-thing"
removed_in = "0.1.0"
reason = "Discontinued upstream in 2025; the project is archived and releases nothing."
replaced_by = "bindery"
"#;

    /// The shipped stack with one removal appended to it.
    fn with_removal(record: &str) -> String {
        format!("{STACK}{record}")
    }

    /// A record that says what went, why, and what took its place is a record with
    /// nothing wrong with it — which is the half every other assertion here rests on.
    #[test]
    fn a_removal_recorded_properly_is_no_fault_at_all() {
        assert_eq!(check(&with_removal(DROPPED)), Vec::new());
    }

    #[test]
    fn a_removal_that_is_still_a_declared_service_is_caught() {
        let text = with_removal(&DROPPED.replace(r#"id = "an-old-thing""#, r#"id = "bindery""#));
        assert!(
            messages(&text)
                .iter()
                .any(|m| m.contains("is recorded as removed and is still declared as a service")),
            "a stack that goes on starting what it says it dropped must be named"
        );
    }

    /// Emptiness rather than presence: an empty reason passes a parse and records
    /// nothing, which is the shape a table filled in to satisfy a rule takes.
    #[test]
    fn a_removal_with_an_empty_reason_is_caught() {
        let text = with_removal(&DROPPED.replacen("reason = ", "reason = \"\" # ", 1));
        assert!(messages(&text)
            .iter()
            .any(|m| m.contains("records no reason for going")));
    }

    #[test]
    fn a_replacement_this_stack_knows_nothing_about_is_caught() {
        let text = with_removal(
            &DROPPED.replace(r#"replaced_by = "bindery""#, r#"replaced_by = "ghost""#),
        );
        assert!(
            messages(&text).iter().any(|m| m
                .contains("says ghost replaced it, and this stack neither declares nor records")),
            "a replacement pointing at nothing reads as an answer and is not one"
        );
    }

    /// Replacements chain. A stack that dropped the thing that replaced the thing it
    /// dropped has recorded two true facts, and refusing the second would make the
    /// record less honest the longer the stack lives.
    #[test]
    fn a_removal_replaced_by_another_removal_is_accepted() {
        let chained = format!(
            "{}{}",
            DROPPED.replace(
                r#"replaced_by = "bindery""#,
                r#"replaced_by = "an-older-thing""#
            ),
            r#"
[[removed]]
id = "an-older-thing"
removed_in = "0.1.0"
reason = "Unmaintained upstream; Audiobookshelf covers what it did."
replaced_by = "audiobookshelf"
"#
        );
        assert_eq!(check(&with_removal(&chained)), Vec::new());
    }

    /// Empty is not the same as absent, and the difference is the whole of what a
    /// replacement field records. A removal that names nothing has said the service
    /// went and nothing took over, which is a fact. One naming an empty string has
    /// filled the box in to get past the rule, and an operator reading the record
    /// afterwards is told there was a successor whose name nobody wrote down.
    #[test]
    fn a_replacement_named_as_an_empty_string_is_caught() {
        let text =
            with_removal(&DROPPED.replace(r#"replaced_by = "bindery""#, r#"replaced_by = "  ""#));
        assert!(
            messages(&text)
                .iter()
                .any(|m| m.contains("names an empty replacement rather than none at all")),
            "a blank successor reads as a successor and is not one"
        );
    }

    #[test]
    fn a_removal_recorded_as_replacing_itself_is_caught() {
        let text = with_removal(&DROPPED.replace(
            r#"replaced_by = "bindery""#,
            r#"replaced_by = "an-old-thing""#,
        ));
        assert!(messages(&text)
            .iter()
            .any(|m| m.contains("is recorded as having replaced itself")));
    }

    /// The version it went in is checked for holding something, the way the reason is:
    /// a record that cannot be placed in the stack's own history answers *when* with
    /// nothing at all.
    #[test]
    fn a_removal_with_no_stack_version_behind_it_is_caught() {
        let text = with_removal(&DROPPED.replacen("removed_in = ", "removed_in = \"\" # ", 1));
        assert!(messages(&text)
            .iter()
            .any(|m| m.contains("records no stack version it went in")));
    }

    #[test]
    fn a_duplicate_removal_id_is_caught() {
        let text = with_removal(&format!("{DROPPED}{DROPPED}"));
        assert!(messages(&text)
            .iter()
            .any(|m| m.contains("another removal already has this id")));
    }

    /// The removals are walked in the same pass the services are, so a fork that got
    /// both wrong hears about both — which is the whole reason this file reports
    /// rather than returns at the first thing it finds.
    #[test]
    fn a_fault_in_a_removal_arrives_beside_a_fault_in_a_service() {
        let text = edited(r#"license = "GPL-3.0-only""#, r#"license = "Nonesuch-1.0""#);
        let both = messages(&format!(
            "{text}{}",
            DROPPED.replacen("reason = ", "reason = \"\" # ", 1)
        ));
        assert!(
            both.iter()
                .any(|m| m.contains("not a recognised OSI identifier")),
            "{both:?}"
        );
        assert!(
            both.iter()
                .any(|m| m.contains("records no reason for going")),
            "{both:?}"
        );
    }

    /// Every violation says where it is, and a removal's location names the removal
    /// rather than the service it shares an id with — there is no such service, which
    /// is the point.
    #[test]
    fn a_removal_s_fault_is_placed_by_the_removal_it_is_about() {
        let text = with_removal(&DROPPED.replacen("reason = ", "reason = \"\" # ", 1));
        assert!(check(&text)
            .iter()
            .any(|violation| violation.location == "removed an-old-thing"));
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
