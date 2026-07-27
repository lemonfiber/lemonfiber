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

use crate::{Manifest, Protocol, Service};

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

/// A calendar date, for checking one recorded in a manifest.
///
/// Ordering is derived, which is chronological because the fields are declared
/// most-significant first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Date {
    /// Four-digit year.
    pub year: u16,
    /// Month, 1 to 12.
    pub month: u8,
    /// Day, 1 to 31.
    pub day: u8,
}

impl Date {
    /// Read a `YYYY-MM-DD` date, rejecting anything else.
    ///
    /// Deliberately strict about shape and about whether the parts are possible.
    /// It does not know how long February is: a date being *real* matters less
    /// than it being unambiguous, and a manifest claiming the 30th of February
    /// has a bigger problem than this check.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let mut parts = text.split('-');
        let year = parts.next()?;
        let month = parts.next()?;
        let day = parts.next()?;
        if parts.next().is_some() {
            return None;
        }

        // Each field is a fixed width of digits and nothing else. A bare length
        // check let `2026-006-1` and a `+`-signed part through, because an integer
        // parse accepts either; requiring exact digit widths is what actually
        // pins the one unambiguous shape.
        let shaped = year.len() == 4
            && month.len() == 2
            && day.len() == 2
            && [year, month, day]
                .iter()
                .all(|part| part.bytes().all(|byte| byte.is_ascii_digit()));
        if !shaped {
            return None;
        }

        let year: u16 = year.parse().ok()?;
        let month: u8 = month.parse().ok()?;
        let day: u8 = day.parse().ok()?;
        ((1..=12).contains(&month) && (1..=31).contains(&day)).then_some(Self { year, month, day })
    }
}

impl Date {
    /// The date at a moment, given as seconds since the Unix epoch, in UTC.
    ///
    /// Days-to-calendar conversion rather than a date library: this is the only
    /// calendar arithmetic in the codebase, and a dependency that parses time
    /// zones and formats a dozen ways would be carried for one function.
    ///
    /// The algorithm is Howard Hinnant's `civil_from_days`, which shifts the
    /// year to start in March so leap days fall at the end of it and no month
    /// needs a special case.
    #[must_use]
    pub fn from_unix_seconds(seconds: i64) -> Option<Self> {
        let days = seconds.div_euclid(86_400);
        let shifted = days + 719_468;
        let era = shifted.div_euclid(146_097);
        let day_of_era = shifted - era * 146_097;
        let year_of_era =
            (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
        let mut year = year_of_era + era * 400;
        let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
        let shifted_month = (5 * day_of_year + 2) / 153;
        let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
        let month = if shifted_month < 10 {
            shifted_month + 3
        } else {
            shifted_month - 9
        };
        if month <= 2 {
            year += 1;
        }

        Some(Self {
            year: u16::try_from(year).ok()?,
            month: u8::try_from(month).ok()?,
            day: u8::try_from(day).ok()?,
        })
    }
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
            .chain(depended(service, &of_service));

        let location = format!("service {}", service.id);
        found.extend(faults.map(|message| Violation {
            location: location.clone(),
            message,
        }));
    }
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

    /// Comfortably after every date the stack records, and not the real clock.
    const TODAY: Date = Date {
        year: 2026,
        month: 7,
        day: 25,
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

    #[test]
    fn the_stack_this_binary_ships_is_valid() {
        assert_eq!(
            check(STACK),
            Vec::new(),
            "the committed stack has no faults"
        );
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
    fn a_date_is_read_only_in_the_one_shape_that_is_unambiguous() {
        assert_eq!(
            Date::parse("2026-06-26"),
            Some(Date {
                year: 2026,
                month: 6,
                day: 26
            })
        );
        for bad in [
            "26-06-26",
            "2026-ab-26",
            "2026-06-cd",
            "xxxx-06-26",
            "2026-6-26",
            "2026/06/26",
            "2026-13-01",
            "2026-00-01",
            "2026-06-32",
            "2026-06-00",
            "2026-06",
            "2026-06-26-01",
            // Misaligned digit groups and signed parts reach length ten but are
            // not the one unambiguous shape.
            "2026-006-1",
            "200-006-01",
            "2026-+6-26",
            "+026-06-26",
            "2026-06-2 ",
            "not a date",
            "",
        ] {
            assert_eq!(Date::parse(bad), None, "{bad:?} should not parse");
        }
    }

    #[test]
    fn a_moment_becomes_the_day_it_falls_on() {
        // Checked against known instants rather than against another
        // implementation of the same arithmetic.
        for (seconds, expected) in [
            (0_i64, "1970-01-01"),
            (86_399, "1970-01-01"),
            (86_400, "1970-01-02"),
            (951_782_400, "2000-02-29"),
            (1_609_459_199, "2020-12-31"),
            (1_774_396_800, "2026-03-25"),
            (-1, "1969-12-31"),
        ] {
            assert_eq!(
                Date::from_unix_seconds(seconds),
                Date::parse(expected),
                "{seconds} should be {expected}"
            );
        }
    }

    #[test]
    fn a_moment_no_calendar_can_name_is_refused_rather_than_wrapped() {
        // Far enough ahead that the year does not fit. Wrapping it would put a
        // manifest's dates in the wrong century rather than saying so.
        assert_eq!(Date::from_unix_seconds(i64::MAX / 2), None);
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

    #[test]
    fn dates_order_chronologically() {
        assert!(Date::parse("2025-12-31") < Date::parse("2026-01-01"));
        assert!(Date::parse("2026-01-01") < Date::parse("2026-01-02"));
        assert!(Date::parse("2026-01-01") < Date::parse("2026-02-01"));
    }
}
