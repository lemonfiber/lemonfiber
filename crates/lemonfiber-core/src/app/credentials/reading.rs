//! Building the inventory out of what is on disk, without spending anything.
//!
//! Two reads and no requests. What is recorded comes from the settings file, and
//! what each service holds comes from the file that service wrote its own key into —
//! so an inventory costs nothing, can be taken while the stack is down, and cannot
//! itself be the thing that exhausts a provider's quota.
//!
//! That is also the limit of what it can claim, and the states say so rather than
//! rounding up. A credential is reported active where something on record says it was
//! proven against the service that takes it: the two the operator supplies at setup
//! carry a flag recording whether they were, and the ones lemonfiber mints were set on
//! their service and confirmed there before they were ever written down. Everything
//! else is stale — present, unproven, and worth a sentence rather than an alarm.
//!
//! The comparison in [`service_key`] is the one thing here that finds a fault rather
//! than reporting a fact. A service that regenerates its API key — which some do after
//! an upgrade — leaves the copy the stack's own services read pointing at a key that
//! no longer exists, and every symptom of it looks like something else.

use std::path::{Path, PathBuf};

use lemonfiber_manifest::{ApiKind, Service};

use crate::app::seed::published_as;
use crate::app::targets::{config_path, recorded_secret};
use crate::app::Ctx;
use crate::credential::{catalogue, fingerprint, Entry, Held, Origin, Reached, State};

/// Where a credential lives when there is no settings file resolved to name.
const NOWHERE: &str = "nowhere yet — this machine has no settings file";

/// Every credential this stack holds, whether or not it is present.
///
/// The declared ones first, in the order they are declared, then the ones the
/// services minted for themselves in the order the manifest names them — so two runs
/// against the same stack list the same things in the same places.
pub(super) async fn taken(ctx: &Ctx, services: &[Service], project: Option<&Path>) -> Vec<Held> {
    let mut taken: Vec<Held> = catalogue(ctx.settings.protocols)
        .into_iter()
        .map(|entry| recorded(ctx, entry))
        .collect();
    for service in services {
        let Some((config, read)) = project.and_then(|project| writes_its_own(service, project))
        else {
            continue;
        };
        let setting = published_as(&service.id);
        let held = ctx
            .filesystem
            .read(&config)
            .await
            .and_then(|text| read(&text));
        let published = recorded_secret(ctx, &setting);
        taken.push(service_key(
            &service.name,
            &setting,
            &config,
            held,
            published,
        ));
    }
    taken
}

/// The reader that takes one service's own key out of the file it wrote it into.
///
/// Four shapes and one question. A Servarr-shape service keeps its key in XML, the
/// subtitle finder in YAML, the request service in JSON and the Usenet client in an
/// INI — and every one of them is a file under the service's own configuration
/// directory, named by the manifest, so the shape decides only which reader opens it.
///
/// The rest write no key for anybody to read. Two of them have no account at all
/// until lemonfiber makes one, and the torrent client authenticates with a password
/// rather than a key; all of those are on the declared list instead, which is where a
/// credential lemonfiber minted belongs.
fn reader(kind: ApiKind) -> Option<fn(&str) -> Option<String>> {
    match kind {
        ApiKind::Servarr => Some(crate::servarr::api_key),
        ApiKind::Bazarr => Some(crate::bazarr::api_key),
        ApiKind::Seerr => Some(crate::seerr::api_key),
        ApiKind::Sabnzbd => Some(crate::sabnzbd::api_key),
        ApiKind::Qbittorrent | ApiKind::Bindery | ApiKind::Jellyfin | ApiKind::Audiobookshelf => {
            None
        }
    }
}

/// Where one service keeps the key it wrote for itself, and how to read it — where it
/// is a service that writes one at all.
fn writes_its_own(
    service: &Service,
    project: &Path,
) -> Option<(PathBuf, fn(&str) -> Option<String>)> {
    let api = service.api.as_ref()?;
    let read = reader(api.kind)?;
    let path = config_path(project, service, api.path.as_deref())?;
    Some((path, read))
}

/// The settings file this machine resolved, as something to print.
fn settings_file(ctx: &Ctx) -> String {
    ctx.settings
        .env_file
        .as_deref()
        .map_or_else(|| NOWHERE.to_owned(), |path| path.display().to_string())
}

/// One declared credential, read back from where it is recorded.
fn recorded(ctx: &Ctx, entry: Entry) -> Held {
    let value = recorded_secret(ctx, entry.setting);
    let (state, advisory) = standing(ctx, &entry, value.is_some());
    Held {
        name: entry.name.to_owned(),
        setting: entry.setting.to_owned(),
        consumers: entry
            .consumers
            .iter()
            .map(|one| one.name.to_owned())
            .collect(),
        location: settings_file(ctx),
        origin: entry.origin,
        state,
        fingerprint: value.as_deref().map(fingerprint),
        advisory,
    }
}

/// Where one declared credential stands, and what is worth saying about it.
fn standing(ctx: &Ctx, entry: &Entry, present: bool) -> (State, Option<String>) {
    if !present {
        return (
            State::Absent,
            Some(format!(
                "{} is not set, so anything needing it will refuse to work. It is recorded as {}.",
                entry.name, entry.setting
            )),
        );
    }
    // A credential lemonfiber minted was set on its service and confirmed there before
    // it was written down, so recording it is the proof. Merged with the origin no
    // declared entry carries, which the inventory reads a different way entirely.
    if matches!(entry.origin, Origin::Lemonfiber | Origin::Service) {
        return (State::Active, None);
    }
    match entry.proven_by.and_then(|flag| proven(ctx, flag)) {
        Some(true) => (State::Active, None),
        Some(false) => (
            State::Stale,
            Some(format!(
                "{} was kept without being proven, because that is what you chose when it was \
                 entered. It has not been checked since. Nothing here expires it.",
                entry.name
            )),
        ),
        None => (
            State::Stale,
            Some(format!(
                "{} is set and nothing on record says it has ever been proven against what \
                 takes it. Run `lemonfiber doctor` to find out. Nothing here expires it.",
                entry.name
            )),
        ),
    }
}

/// Whether the flag recording a proof says one happened, where the flag is set at all.
fn proven(ctx: &Ctx, flag: &str) -> Option<bool> {
    recorded_secret(ctx, flag).map(|recorded| crate::config::reads_as_on(&recorded))
}

/// One service's own API key, read from the file it wrote it to and compared with the
/// copy the stack's own services read out of the environment.
///
/// The four answers are four different situations and only one of them is a fault. A
/// service that has written no key yet is starting; one whose key has not been copied
/// out is a stack that has not been seeded since; one whose two copies agree is
/// working. One whose copies disagree has regenerated its key underneath a stack that
/// is still handing out the old one, and every consumer of it is quietly failing to
/// authenticate while the service itself reports perfectly healthy.
fn service_key(
    name: &str,
    setting: &str,
    config: &Path,
    held: Option<String>,
    published: Option<String>,
) -> Held {
    let (state, advisory) = key_standing(name, setting, held.as_deref(), published.as_deref());
    Held {
        name: format!("{name} API key"),
        setting: setting.to_owned(),
        consumers: service_consumers(name, setting)
            .into_iter()
            .map(|(consumer, _)| consumer)
            .collect(),
        location: config.display().to_string(),
        origin: Origin::Service,
        state,
        fingerprint: held.as_deref().map(fingerprint),
        advisory,
    }
}

/// Everything that authenticates with one service's own API key, and how a fresh
/// copy of it gets to each.
///
/// Shared with the rotation rather than written out there again, so what a rotation
/// reports having reached and what the inventory says uses it are one list. Two of
/// them: the service itself, which minted the key and needs nothing done to it, and
/// everything reading it out of the environment, which is fixed when a container is
/// created and so needs one.
pub(super) fn service_consumers(name: &str, setting: &str) -> Vec<(String, Reached)> {
    vec![
        (format!("{name}'s own API"), Reached::AtTheService),
        (
            format!(
                "the stack's own services, which read it from the environment as {setting} — \
                 the dashboard, and the quality sync and archive extractor where they cover \
                 {name}"
            ),
            Reached::FromItsEnvironment {
                restart: "lemonfiber restart",
            },
        ),
    ]
}

/// Where one service key stands, given what the service holds and what was published.
fn key_standing(
    name: &str,
    setting: &str,
    held: Option<&str>,
    published: Option<&str>,
) -> (State, Option<String>) {
    let Some(held) = held else {
        return (
            State::Absent,
            Some(format!(
                "{name} has not written an API key yet, which is what a service still \
                 completing its first start looks like."
            )),
        );
    };
    match published {
        Some(published) if published == held => (State::Active, None),
        Some(published) => (
            State::Invalid,
            Some(format!(
                "{name} has regenerated its API key. What it holds now is {}, and the copy the \
                 stack's own services read as {setting} is still {} — so every one of them is \
                 authenticating with a key that no longer exists. Run `lemonfiber credentials \
                 rotate {name}` to hand them the current one.",
                fingerprint(held),
                fingerprint(published)
            )),
        ),
        None => (
            State::Stale,
            Some(format!(
                "{name} has written an API key and nothing has copied it to {setting}, where \
                 the stack's own services read it. Run `lemonfiber seed`."
            )),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::{key_standing, service_key};
    use crate::credential::{fingerprint, Origin, State};
    use std::path::Path;

    /// Two values that are certainly not each other, built rather than written so a
    /// scanner reads them as what they are.
    fn a_pair() -> (String, String) {
        (
            format!("{}{}", "the-", "key-one"),
            format!("{}{}", "the-", "key-two"),
        )
    }

    #[test]
    fn a_service_that_has_written_no_key_is_starting_rather_than_broken() {
        let (state, advisory) = key_standing("Sonarr", "SONARR_API_KEY", None, Some("anything"));

        assert_eq!(state, State::Absent);
        assert!(advisory.is_some_and(|said| said.contains("first start")));
    }

    #[test]
    fn a_key_nothing_has_copied_out_yet_is_stale_and_points_at_the_seeding() {
        let (one, _) = a_pair();
        let (state, advisory) = key_standing("Sonarr", "SONARR_API_KEY", Some(&one), None);

        assert_eq!(state, State::Stale);
        let said = advisory.unwrap_or_default();
        assert!(said.contains("lemonfiber seed"), "{said}");
        assert!(!said.contains(&one), "{said}");
    }

    #[test]
    fn two_copies_that_agree_are_active_and_say_nothing() {
        let (one, _) = a_pair();
        let (state, advisory) = key_standing("Sonarr", "SONARR_API_KEY", Some(&one), Some(&one));

        assert_eq!(state, State::Active);
        assert_eq!(advisory, None);
    }

    /// The fault this comparison exists to find, and the words it has to say.
    #[test]
    fn a_service_that_regenerated_its_key_is_reported_against_the_stale_copy() {
        let (held, published) = a_pair();
        let (state, advisory) =
            key_standing("Sonarr", "SONARR_API_KEY", Some(&held), Some(&published));

        assert_eq!(state, State::Invalid);
        let said = advisory.unwrap_or_default();
        assert!(said.contains("regenerated"), "{said}");
        assert!(said.contains(&fingerprint(&held)), "{said}");
        assert!(said.contains(&fingerprint(&published)), "{said}");
    }

    /// The advisory names both copies and prints neither.
    #[test]
    fn neither_copy_appears_in_what_the_mismatch_says() {
        let (held, published) = a_pair();
        let (_, advisory) = key_standing("Sonarr", "SONARR_API_KEY", Some(&held), Some(&published));
        let said = advisory.unwrap_or_default();

        assert!(!said.is_empty());
        assert!(!said.contains(&held), "{said}");
        assert!(!said.contains(&published), "{said}");
    }

    #[test]
    fn a_service_key_names_the_service_and_everything_that_reads_it_from_the_environment() {
        let (held, _) = a_pair();
        let entry = service_key(
            "Sonarr",
            "SONARR_API_KEY",
            Path::new("/stack/config/sonarr/config.xml"),
            Some(held.clone()),
            Some(held.clone()),
        );

        assert_eq!(entry.name, "Sonarr API key");
        assert_eq!(entry.origin, Origin::Service);
        assert_eq!(entry.consumers.len(), 2);
        assert!(entry.consumers.join(" ").contains("SONARR_API_KEY"));
        assert_eq!(entry.location, "/stack/config/sonarr/config.xml");
        assert_eq!(entry.fingerprint, Some(fingerprint(&held)));
    }

    #[test]
    fn a_service_key_nobody_has_written_carries_no_likeness_of_a_value() {
        let entry = service_key(
            "Sonarr",
            "SONARR_API_KEY",
            Path::new("/stack/config/sonarr/config.xml"),
            None,
            None,
        );

        assert_eq!(entry.fingerprint, None);
        assert_eq!(entry.state, State::Absent);
    }
}
