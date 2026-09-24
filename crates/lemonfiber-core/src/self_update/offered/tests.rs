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
fn a_build_cut_ahead_of_a_release_is_offered_that_release() {
    // This used to answer `Untellable`, and the reason was an accident: the tag
    // is not a dotted run of numbers, so nothing could order it. Somebody running
    // a build cut ahead of 0.13.0 is exactly the person who wants 0.13.0 when it
    // arrives, and telling them the question cannot be answered is a worse reply
    // than the true one.
    assert_eq!(
        standing("0.13.0-pre.1", "0.13.0"),
        Availability::Newer("0.13.0".to_owned())
    );
    assert_eq!(standing("0.13.0-pre.1", "0.12.0"), Availability::Current);
}

#[test]
fn two_versions_that_cannot_be_ordered_claim_nothing_about_either() {
    assert_eq!(standing("nightly", "0.13.0"), Availability::Untellable);
    assert_eq!(standing("0.13.0", "nightly"), Availability::Untellable);
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
    let answered = r#"[{"tag_name":"v0.15.0","draft":true,"assets":[{"name":"stack-schema-2"}]}]"#;
    assert_eq!(schema(answered, "0.15.0"), None);
}
