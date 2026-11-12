use std::path::{Path, PathBuf};

use lemonfiber_contract::Capability;

use super::{judged, judged_now, owed, recording_of, Judged, Speaker, Verdict};

/// The upstream's pin, as a recording names it.
const PINNED: &str =
    "example.invalid/subber@sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945";

/// The contract the adapter speaks.
const CONTRACT: &str = "subtitles.fetch@1";

const WATCHING: &str = "/lemonfiber/subtitles.fetch/v1/watching";
const WATCH: &str = "/lemonfiber/subtitles.fetch/v1/watch";

/// An adapter speaking `subtitles.fetch@1` and a contract nothing publishes, in front of
/// an upstream pinned at `pinned`.
fn speaker(pinned: Option<&str>) -> Vec<Speaker> {
    vec![Speaker {
        service: "subber-adapter".to_owned(),
        speaks: vec![CONTRACT.to_owned(), "nothing.published@1".to_owned()],
        pinned: pinned.map(str::to_owned),
        files: Vec::new(),
    }]
}

/// A recording of `path` taken from `from`, answered with `response`.
fn recording(from: &str, path: &str, response: &str) -> String {
    format!(
        r#"{{"recorded_from": "{from}", "note": "what the case asks", "request": {{"method": "POST", "path": "{path}", "accept": "application/json"}}, "response": {response}}}"#
    )
}

/// Every case, recorded as an adapter that conforms answers it.
fn conforming() -> Vec<(&'static str, String)> {
    vec![
        (
            "refuses-without-the-key",
            recording(PINNED, WATCHING, r#"{"status": 401}"#),
        ),
        (
            "watching-answers",
            recording(
                PINNED,
                WATCHING,
                r#"{"status": 200, "json": {"enabled": true, "host": "tv", "port": 8989, "keyed": true}}"#,
            ),
        ),
        (
            "watch-answers",
            recording(PINNED, WATCH, r#"{"status": 204}"#),
        ),
    ]
}

/// A plugin's source of this test's own, holding these recordings by case.
fn source(tag: &str, recorded: &[(&str, String)]) -> PathBuf {
    let root = std::env::temp_dir()
        .join("lemonfiber-conformance-tests")
        .join(format!("judging-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::create_dir_all(root.join("conformance").join(CONTRACT));
    for (case, text) in recorded {
        let _ = std::fs::write(root.join(recording_of(CONTRACT, case)), text);
    }
    root
}

/// `subtitles.fetch@1`, with each operation in `added` added to it at minor 1.
fn fetch_adding(added: &[&str]) -> Vec<Capability> {
    let mut fetch = lemonfiber_contract::capabilities::subtitles::fetch::capability();
    for operation in &mut fetch.operations {
        if added.contains(&operation.name) {
            operation.since = 1;
        }
    }
    vec![fetch]
}

/// The verdict of `case`, where it was judged.
fn of<'a>(judged: &'a [Judged], case: &str) -> Option<&'a Verdict> {
    judged
        .iter()
        .find(|one| one.case == case)
        .map(|one| &one.verdict)
}

/// What judging the recordings at `root` came to for the adapter pinned at [`PINNED`].
async fn read(root: &Path, published: &[Capability]) -> Vec<Judged> {
    judged(root, &speaker(Some(PINNED)), published).await
}

#[test]
fn an_adapter_owes_a_recording_of_each_case_of_each_contract_published_it_speaks() {
    let published = lemonfiber_contract::capabilities::all();

    let owed = owed(&speaker(Some(PINNED)), &published);

    assert_eq!(
        owed.iter()
            .map(|one| (one.contract.as_str(), one.recording.as_str()))
            .collect::<Vec<_>>(),
        vec![
            (
                CONTRACT,
                "conformance/subtitles.fetch@1/refuses-without-the-key.json"
            ),
            (
                CONTRACT,
                "conformance/subtitles.fetch@1/watching-answers.json"
            ),
            (CONTRACT, "conformance/subtitles.fetch@1/watch-answers.json"),
        ]
    );
    assert!(owed
        .iter()
        .all(|one| one.service == "subber-adapter" && one.pinned.as_deref() == Some(PINNED)));
}

#[tokio::test]
async fn every_case_answered_as_the_contract_declares_passes() {
    let judged = read(&source("conforms", &conforming()), &fetch_adding(&[])).await;

    assert_eq!(
        judged
            .iter()
            .map(|one| (one.case.as_str(), &one.verdict))
            .collect::<Vec<_>>(),
        vec![
            ("refuses-without-the-key", &Verdict::Passed),
            ("watching-answers", &Verdict::Passed),
            ("watch-answers", &Verdict::Passed),
        ]
    );
}

#[tokio::test]
async fn a_case_with_no_recording_is_unrecorded_and_one_not_judged_is_unproven() {
    let recorded = vec![
        (
            "refuses-without-the-key",
            recording(
                "example.invalid/other@sha256:00",
                WATCHING,
                r#"{"status": 401}"#,
            ),
        ),
        ("watching-answers", "not a recording".to_owned()),
    ];

    let judged = read(&source("unrecorded", &recorded), &fetch_adding(&[])).await;

    assert!(
        matches!(
            &judged[..],
            [
                Judged { verdict: Verdict::Unproven { why: other }, .. },
                Judged { verdict: Verdict::Unproven { why: unread }, .. },
                Judged { verdict: Verdict::Unrecorded, .. },
            ] if other.contains("is pinned at") && unread.contains("is not a recording")
        ),
        "{judged:?}"
    );
}

#[tokio::test]
async fn an_answer_outside_the_contract_fails_and_a_refusal_of_the_plugins_own_key_fails() {
    let recorded: Vec<(&str, String)> = conforming()
        .into_iter()
        .map(|(case, text)| match case {
            "watching-answers" => (case, recording(PINNED, WATCHING, r#"{"status": 401}"#)),
            "watch-answers" => (
                case,
                recording(PINNED, WATCH, r#"{"status": 200, "json": 7}"#),
            ),
            _ => (case, text),
        })
        .collect();

    let judged = read(&source("failing", &recorded), &fetch_adding(&[])).await;

    assert!(matches!(
        of(&judged, "watching-answers"),
        Some(Verdict::Failed { faults }) if faults == &["refused the plugin's own key".to_owned()]
    ));
    assert!(matches!(
        of(&judged, "watch-answers"),
        Some(Verdict::Failed { faults }) if !faults.is_empty()
    ));
}

#[tokio::test]
async fn an_operation_added_after_the_minor_recorded_against_is_no_case_of_it() {
    let recorded: Vec<(&str, String)> = conforming()
        .into_iter()
        .filter(|(case, _)| *case != "watch-answers")
        .collect();
    let root = source("added", &recorded);

    let added = read(&root, &fetch_adding(&["watch"])).await;
    let published_with = read(&root, &fetch_adding(&[])).await;

    assert_eq!(of(&added, "watch-answers"), Some(&Verdict::Passed));
    assert_eq!(
        of(&published_with, "watch-answers"),
        Some(&Verdict::Unrecorded)
    );
}

#[tokio::test]
async fn an_adapter_answering_for_no_pinned_upstream_is_unproven() {
    let judged = judged(
        &source("unpinned", &conforming()),
        &speaker(None),
        &fetch_adding(&[]),
    )
    .await;

    assert!(!judged.is_empty());
    assert!(judged.iter().all(|one| matches!(
        &one.verdict,
        Verdict::Unproven { why } if why.contains("names no upstream")
    )));
}

#[test]
fn a_case_is_named_with_its_contract_and_what_it_came_to() {
    let named = |verdict: Verdict| {
        Judged {
            contract: CONTRACT.to_owned(),
            case: "watch-answers".to_owned(),
            recording: String::new(),
            verdict,
        }
        .to_string()
    };

    assert_eq!(
        named(Verdict::Passed),
        "subtitles.fetch@1/watch-answers — passed"
    );
    assert_eq!(
        named(Verdict::Failed {
            faults: vec!["one".to_owned(), "two".to_owned()]
        }),
        "subtitles.fetch@1/watch-answers — failed: one; two"
    );
    assert_eq!(
        named(Verdict::Unrecorded),
        "subtitles.fetch@1/watch-answers — no recording answers it"
    );
    assert_eq!(
        named(Verdict::Unproven {
            why: "why".to_owned()
        }),
        "subtitles.fetch@1/watch-answers — unproven: why"
    );
}

#[test]
fn a_build_script_with_no_runtime_of_its_own_judges_it_to_the_end() {
    let root = source("now", &conforming());

    let judged = judged_now(&root, &speaker(Some(PINNED)), &fetch_adding(&[])).unwrap_or_default();

    assert_eq!(judged.len(), 3);
    assert!(judged.iter().all(|one| one.verdict == Verdict::Passed));
}

#[tokio::test]
async fn a_media_type_added_at_a_later_minor_is_owed_only_by_an_adapter_recorded_at_it() {
    let curator = vec![Speaker {
        service: "lidarr-adapter".to_owned(),
        speaks: vec!["library.curate@1".to_owned()],
        pinned: Some(PINNED.to_owned()),
        files: vec!["music".to_owned()],
    }];
    let published = vec![lemonfiber_contract::capabilities::library::capability()];

    let judged = judged(&source("albums", &[]), &curator, &published).await;

    assert_eq!(of(&judged, "library-answers-music"), Some(&Verdict::Passed));
    assert_eq!(
        of(&judged, "records-answers-music"),
        Some(&Verdict::Unrecorded)
    );
}
