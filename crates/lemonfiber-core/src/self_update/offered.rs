//! Asking what has been released, and making as little of the answer as possible.
//!
//! The request carries nothing. No credential, no version, no setting, nothing that
//! would tell one installation from another — the only header beyond the two any
//! reader of this address sends is a name, constant in every copy of this program,
//! which the address requires of anybody asking and which distinguishes nobody.
//!
//! **The list rather than the latest.** There is an address that serves "the latest
//! release" and it is the wrong one here: every release of this project is published
//! as a pre-release, and that address passes over pre-releases, so it answers with
//! nothing at all. Reading the list and ordering it here is not a preference — it is
//! the only reading that has an answer.
//!
//! Ordering is [`crate::migration::version`]'s, which answers `Untellable` rather
//! than guessing which of two strings is later. A wrong answer here would tell an
//! operator to move to a version that is behind the one they have, so a tag this
//! cannot take whole is passed over rather than ranked.

use serde::Deserialize;

use crate::migration::version::{against, Standing};
use crate::ports::http::{Method, Request};

/// How many of the most recent releases to read.
///
/// Enough that the newest is certainly among them and small enough that the answer
/// is one small document. The address serves them newest-first, so this is a bound
/// on the reply rather than a search that could run past the answer.
const HOW_MANY: usize = 10;

/// The name this program gives when it asks.
///
/// The address refuses a request that gives none, so there is one to choose and the
/// choice is what it says. The product's name and nothing after it: a version here
/// would be a fact about this installation travelling on every check, and one
/// constant string shared by every copy tells whoever reads it nothing it did not
/// already know from being asked.
const CALLED: &str = "lemonfiber";

/// Where the release page puts the boundary between what changed and how to install it.
///
/// `release-changelog.yml` writes the notes above this line and leaves what cargo-dist
/// wrote below it. Splitting on the marker rather than taking the whole body is what
/// keeps install boilerplate out of an answer about what changed — and a release whose
/// body has no marker is one written before the notes came back, which is answered with
/// nothing rather than with the boilerplate.
const BOUNDARY: &str = "<!-- the changelog is above; cargo-dist wrote what follows -->";

/// The prefix a release publishes its stack's manifest generation under.
///
/// The declaration is the asset's *name*, and its contents are never read. That is
/// what makes this free: names arrive in the same reply the version is read out of,
/// so nothing is fetched a second time and nothing new is sent to learn it.
const DECLARED: &str = "stack-schema-";

/// One file published with a release. Only the name is read.
#[derive(Debug, Deserialize)]
struct Asset {
    /// What the file is called, which is where the declaration lives.
    name: String,
}

/// One release, as much of it as this reads.
#[derive(Debug, Deserialize)]
struct Release {
    /// The tag the release was cut from.
    tag_name: String,
    /// Whether it is still a draft, and so not released at all.
    draft: bool,
    /// The files published with it.
    ///
    /// Defaulted rather than required: a reply that omits them is a reply that
    /// declares nothing, which is a thing this has an answer for.
    #[serde(default)]
    assets: Vec<Asset>,
    /// What the release page says, where it says anything.
    #[serde(default)]
    body: Option<String>,
}

/// The request that asks what has been released.
#[must_use]
pub fn asking(at: &str) -> Request {
    Request {
        method: Method::Get,
        url: format!("{at}?per_page={HOW_MANY}"),
        headers: vec![
            (
                "Accept".to_owned(),
                "application/vnd.github+json".to_owned(),
            ),
            ("User-Agent".to_owned(), CALLED.to_owned()),
        ],
        body: None,
    }
}

/// The newest version in what the address answered, where one can be told.
///
/// Nothing where the answer was not a list of releases, held none that was
/// published, or held none whose tag this can order. Each of those is the same thing
/// to a caller — availability could not be determined — and none of them is a
/// failure the operator has to do anything about.
#[must_use]
pub fn newest(answered: &str) -> Option<String> {
    let released: Vec<Release> = serde_json::from_str(answered).ok()?;
    let mut best: Option<String> = None;
    for release in released.into_iter().filter(|release| !release.draft) {
        let version = release.tag_name.trim_start_matches('v').to_owned();
        let later = match &best {
            None => ordered(&version),
            Some(held) => against(&version, held) == Standing::Later,
        };
        if later {
            best = Some(version);
        }
    }
    best
}

/// What the release for one version says changed, where it says anything.
///
/// Read out of the same answer `newest` is read out of, so learning what is available
/// and learning what it brought are one request rather than two. An operator weighing
/// an update is asking both questions at once, and the second is the one they decide
/// on.
#[must_use]
pub fn changed(answered: &str, version: &str) -> Option<String> {
    let released: Vec<Release> = serde_json::from_str(answered).ok()?;
    let release = released
        .into_iter()
        .find(|release| !release.draft && release.tag_name.trim_start_matches('v') == version)?;
    let body = release.body?;
    let (notes, _) = body.split_once(BOUNDARY)?;
    let notes = notes.trim();
    (!notes.is_empty()).then(|| notes.to_owned())
}

/// The manifest generation the release for `version` says its stack carries.
///
/// Read out of the same answer `newest` and `changed` are read out of, so what is
/// available, what it changed and what it carries are one request rather than three.
///
/// Nothing where the release declares none. That is not the same as declaring zero
/// and must not read like it: a release published before this was declared, or by a
/// fork that publishes none, is one whose stack generation is unknown — and a caller
/// that guessed would say something false about an update rather than say it cannot
/// tell.
#[must_use]
pub fn schema(answered: &str, version: &str) -> Option<u32> {
    let released: Vec<Release> = serde_json::from_str(answered).ok()?;
    let release = released
        .into_iter()
        .find(|release| !release.draft && release.tag_name.trim_start_matches('v') == version)?;
    release
        .assets
        .iter()
        .find_map(|asset| asset.name.strip_prefix(DECLARED))
        .and_then(|declared| declared.parse().ok())
}

/// Whether a version is one this can order at all.
///
/// Asked of the first candidate, because a list holding one unorderable tag would
/// otherwise be answered with that tag — the comparison never runs, so nothing
/// catches it. Every later candidate is caught by the comparison itself.
fn ordered(version: &str) -> bool {
    against(version, "0.0.0") != Standing::Untellable
}

/// Where the running version stands against the newest released one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Availability {
    /// Nothing newer has been released.
    Current,
    /// This version has been released since.
    Newer(String),
    /// The two cannot be ordered, so nothing is claimed about either.
    Untellable,
}

/// Where `running` stands against `offered`.
#[must_use]
pub fn standing(running: &str, offered: &str) -> Availability {
    match against(running, offered) {
        Standing::Earlier => Availability::Newer(offered.to_owned()),
        Standing::Same | Standing::Later => Availability::Current,
        Standing::Untellable => Availability::Untellable,
    }
}

#[cfg(test)]
mod tests {
    use super::{asking, changed, newest, schema, standing, Availability, BOUNDARY, HOW_MANY};
    use crate::ports::http::Method;

    /// The shape the address answers with, with a release page's body on each.
    fn published(tags: &[(&str, &str)]) -> String {
        let entries: Vec<String> = tags
            .iter()
            .map(|(tag, body)| {
                let body = serde_json::to_string(body).unwrap_or_default();
                format!(r#"{{"tag_name":"{tag}","draft":false,"body":{body}}}"#)
            })
            .collect();
        format!("[{}]", entries.join(","))
    }

    /// A release page as this project writes one: the notes, the marker, the installers.
    fn page(notes: &str) -> String {
        format!("{notes}\n\n{BOUNDARY}\n\ncurl -LsSf https://example.test/install.sh | sh\n")
    }

    #[test]
    fn the_notes_for_the_version_asked_about_are_read_out_of_the_same_answer() {
        let answered = published(&[
            (
                "v0.14.0",
                &page("### New\n- The panel shows the forwarded port"),
            ),
            ("v0.13.0", &page("### Fixed\n- Something older")),
        ]);
        assert_eq!(
            changed(&answered, "0.14.0").as_deref(),
            Some("### New\n- The panel shows the forwarded port")
        );
        assert_eq!(
            changed(&answered, "0.13.0").as_deref(),
            Some("### Fixed\n- Something older")
        );
    }

    #[test]
    fn install_boilerplate_is_never_answered_as_what_changed() {
        // A release published before the notes came back has a body and none of it
        // is a changelog; the marker is what separates the two, and there is none.
        let answered = published(&[("v0.5.0", "curl -LsSf https://example.test/install.sh | sh")]);
        assert_eq!(changed(&answered, "0.5.0"), None);
    }

    #[test]
    fn a_release_with_nothing_above_the_marker_is_answered_with_nothing() {
        let answered = published(&[("v0.5.0", &page("   "))]);
        assert_eq!(changed(&answered, "0.5.0"), None);
    }

    #[test]
    fn a_version_the_answer_does_not_hold_has_no_notes_and_is_not_a_failure() {
        let answered = published(&[("v0.14.0", &page("### New\n- Something"))]);
        assert_eq!(changed(&answered, "0.12.0"), None);
        assert_eq!(changed("not a release list at all", "0.14.0"), None);
    }

    #[test]
    fn a_draft_is_no_more_a_source_of_notes_than_it_is_of_a_version() {
        let notes = serde_json::to_string(&page("### New\n- Not out yet")).unwrap_or_default();
        let answered = format!(r#"[{{"tag_name":"v0.14.0","draft":true,"body":{notes}}}]"#);
        assert_eq!(changed(&answered, "0.14.0"), None);
    }

    #[test]
    fn a_release_that_says_nothing_at_all_is_read_without_complaint() {
        let answered = r#"[{"tag_name":"v0.14.0","draft":false}]"#;
        assert_eq!(changed(answered, "0.14.0"), None);
    }

    /// The shape the address answers with, cut down to the two fields read.
    fn released(tags: &[(&str, bool)]) -> String {
        let entries: Vec<String> = tags
            .iter()
            .map(|(tag, draft)| format!(r#"{{"tag_name":"{tag}","draft":{draft}}}"#))
            .collect();
        format!("[{}]", entries.join(","))
    }

    #[test]
    fn the_request_asks_for_the_list_and_carries_nothing_about_this_machine() {
        let request = asking("https://api.example.test/releases");
        assert_eq!(request.method, Method::Get);
        assert!(request.url.ends_with(&format!("?per_page={HOW_MANY}")));
        assert_eq!(request.body, None);
        let names: Vec<&str> = request
            .headers
            .iter()
            .map(|(name, _)| name.as_str())
            .collect();
        assert_eq!(names, vec!["Accept", "User-Agent"]);
        let sent = request
            .headers
            .iter()
            .map(|(_, value)| value.clone())
            .collect::<Vec<String>>()
            .join(" ");
        assert!(!sent.contains(env!("CARGO_PKG_VERSION")), "{sent}");
    }

    #[test]
    fn a_pre_release_is_never_what_an_operator_is_offered() {
        // A pre-release goes out while its version is still staged, carrying the goals
        // the release gate calls unmet. Offering it would be recommending it, and the
        // tag is what says not to: `0.15.0-pre.1` is not a dotted run of numbers, so it
        // cannot be ordered and is passed over rather than ranked. The behaviour is the
        // general rule about untellable tags, and this holds it to this case.
        let answered = released(&[("v0.15.0-pre.1", false), ("v0.14.0", false)]);
        assert_eq!(newest(&answered).as_deref(), Some("0.14.0"));

        // And with nothing else in the list there is no newest at all, rather than a
        // pre-release standing in for one.
        let alone = released(&[("v0.15.0-pre.1", false)]);
        assert_eq!(newest(&alone), None);
    }

    #[test]
    fn the_newest_published_release_is_the_answer_whatever_order_they_arrive_in() {
        let answered = released(&[("v0.12.0", false), ("v0.13.0", false), ("v0.9.1", false)]);
        assert_eq!(newest(&answered).as_deref(), Some("0.13.0"));
    }

    #[test]
    fn a_draft_is_not_a_release_and_is_passed_over() {
        let answered = released(&[("v0.14.0", true), ("v0.13.0", false)]);
        assert_eq!(newest(&answered).as_deref(), Some("0.13.0"));
    }

    #[test]
    fn a_tag_that_cannot_be_ordered_is_passed_over_rather_than_ranked() {
        let answered = released(&[("nightly", false), ("v0.13.0", false)]);
        assert_eq!(newest(&answered).as_deref(), Some("0.13.0"));
    }

    /// The one an ordering alone cannot catch: nothing is compared against a first
    /// candidate, so a list of only unorderable tags would otherwise answer with one.
    #[test]
    fn a_list_of_nothing_orderable_answers_with_nothing() {
        let answered = released(&[("nightly", false), ("latest", false)]);
        assert_eq!(newest(&answered), None);
    }

    #[test]
    fn an_answer_that_is_not_a_list_of_releases_is_not_made_into_one() {
        for answered in ["", "{}", r#"{"message":"Not Found"}"#, "[{}]"] {
            assert_eq!(newest(answered), None, "{answered}");
        }
    }

    #[test]
    fn a_list_with_nothing_published_in_it_answers_with_nothing() {
        assert_eq!(newest(&released(&[("v0.13.0", true)])), None);
        assert_eq!(newest("[]"), None);
    }

    #[test]
    fn a_running_version_behind_the_newest_has_one_to_move_to() {
        assert_eq!(
            standing("0.12.0", "0.13.0"),
            Availability::Newer("0.13.0".to_owned())
        );
    }

    #[test]
    fn a_running_version_at_or_past_the_newest_has_nothing_to_move_to() {
        assert_eq!(standing("0.13.0", "0.13.0"), Availability::Current);
        assert_eq!(standing("0.14.0", "0.13.0"), Availability::Current);
    }

    #[test]
    fn two_versions_that_cannot_be_ordered_claim_nothing_about_either() {
        assert_eq!(standing("0.13.0-rc1", "0.13.0"), Availability::Untellable);
    }

    /// The same reply shape, with the files a release publishes beside each entry.
    fn publishing(tag: &str, assets: &[&str]) -> String {
        let named: Vec<String> = assets
            .iter()
            .map(|name| format!(r#"{{"name":"{name}"}}"#))
            .collect();
        format!(
            r#"[{{"tag_name":"{tag}","draft":false,"assets":[{}]}}]"#,
            named.join(",")
        )
    }

    /// The declaration is a name in a reply already fetched, which is what makes
    /// reading it free — no second request, and nothing new sent to learn it.
    #[test]
    fn a_release_declares_its_stack_generation_in_the_name_of_a_published_file() {
        let answered = publishing(
            "v0.15.0",
            &["lemonfiber-installer.sh", "stack-schema-2", "sha256.sum"],
        );
        assert_eq!(schema(&answered, "0.15.0"), Some(2));
    }

    /// Absent, not zero. A release that declares nothing is one whose generation is
    /// unknown, and a number would be an answer nobody gave.
    #[test]
    fn a_release_publishing_no_declaration_reads_as_nothing_rather_than_a_generation() {
        let answered = publishing("v0.15.0", &["lemonfiber-installer.sh", "sha256.sum"]);
        assert_eq!(schema(&answered, "0.15.0"), None);
    }

    /// Read for the release asked about, not for whichever one happens to declare one.
    #[test]
    fn the_declaration_read_is_the_one_belonging_to_the_version_asked_about() {
        let answered = r#"[{"tag_name":"v0.15.0","draft":false,"assets":[{"name":"stack-schema-2"}]},{"tag_name":"v0.14.0","draft":false,"assets":[{"name":"stack-schema-1"}]}]"#;
        assert_eq!(
            (schema(answered, "0.15.0"), schema(answered, "0.14.0")),
            (Some(2), Some(1))
        );
    }

    /// A name that is not a number is not a generation. Anything unreadable answers
    /// the same way a missing declaration does rather than being guessed at.
    #[test]
    fn a_declaration_that_is_not_a_number_reads_as_nothing() {
        let answered = publishing("v0.15.0", &["stack-schema-next"]);
        assert_eq!(schema(&answered, "0.15.0"), None);
    }

    /// A reply with no assets at all still parses. The field is defaulted precisely so
    /// that a shape without it is a release declaring nothing rather than an error.
    #[test]
    fn a_reply_with_no_files_at_all_is_a_release_declaring_nothing() {
        assert_eq!(schema(&published(&[("v0.15.0", "notes")]), "0.15.0"), None);
    }

    /// A draft is not a release, here as everywhere else in this module.
    #[test]
    fn a_draft_declares_nothing_because_it_is_not_released() {
        let answered =
            r#"[{"tag_name":"v0.15.0","draft":true,"assets":[{"name":"stack-schema-2"}]}]"#;
        assert_eq!(schema(answered, "0.15.0"), None);
    }
}
