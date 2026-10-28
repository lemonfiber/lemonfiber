use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::service::Failure;

use serde_json::Value;

use super::{answers, cases, Expect, Placeholder, REFUSES_WITHOUT_THE_KEY};
use crate::Contracted;

#[test]
fn a_capability_opens_with_an_unkeyed_case_and_has_one_per_operation() {
    let serve = crate::capabilities::media::serve::capability();
    let listed = cases(&serve);

    assert_eq!(listed.len(), serve.operations.len() + 1);
    assert!(listed
        .first()
        .is_some_and(|first| first.case == "refuses-without-the-key"
            && !first.keyed
            && first.live
            && first.expect == Expect::Status(&[401])));
    assert!(listed.iter().skip(1).all(|case| case.keyed
        && !case.live
        && case.expect == Expect::Conforms
        && case.case == format!("{}-answers", case.operation.name)));
}

#[tokio::test]
async fn every_operation_reads_an_answer_as_the_client_does() {
    for capability in crate::capabilities::all() {
        for operation in &capability.operations {
            let refusing = Contracted::new(
                Fake::always(Answer::reply(418, "")),
                "http://adapter",
                "an-adapter",
                "k3y",
            );
            assert!(
                matches!(
                    operation.judged(&refusing).await,
                    Err(Failure::Refused { .. })
                ),
                "{}.{}",
                capability.name,
                operation.name
            );
        }
    }
    let serve = crate::capabilities::media::serve::capability();
    let signing_out = serve
        .operations
        .iter()
        .find(|operation| operation.name == "sign_out");
    let answering = Contracted::new(
        Fake::always(Answer::reply(204, "")),
        "http://adapter",
        "an-adapter",
        "k3y",
    );
    match signing_out {
        Some(operation) => assert!(operation.judged(&answering).await.is_ok()),
        None => unreachable!("media.serve signs a device out"),
    }
}

#[test]
fn a_placeholder_is_written_as_it_is_read_back() {
    for placeholder in [
        Placeholder::Minted,
        Placeholder::Certificate,
        Placeholder::Pinned,
        Placeholder::Upstream("password".to_owned()),
        Placeholder::MintedUpstream("password".to_owned()),
        Placeholder::answer("identity.source", 1, "invite", "/id"),
        Placeholder::answer("media.serve", 1, "mint", ""),
    ] {
        assert_eq!(
            Placeholder::read(&placeholder.written()),
            Some(placeholder.clone())
        );
        assert_eq!(String::from(placeholder.clone()), placeholder.written());
    }
    assert_eq!(Placeholder::Minted.written(), "{minted}");
    assert_eq!(Placeholder::Certificate.written(), "{certificate}");
    assert_eq!(Placeholder::Pinned.written(), "{certificate:sha256}");
    assert_eq!(
        Placeholder::answer("identity.source", 1, "invite", "/id").written(),
        "{identity.source@1/invite-answers#/id}"
    );
}

#[test]
fn a_string_that_is_not_exactly_one_placeholder_is_posted_as_written() {
    for text in [
        "minted",
        "{minted",
        "minted}",
        "{{minted}}",
        "{upstream:}",
        "{upstream:a:b}",
        "{minted:upstream:}",
        "{minted:other}",
        "{identity.source@1/invite-answers}",
        "{identity.source@x/invite-answers#/id}",
        "{@1/invite-answers#/id}",
        "{identity.source@1/#/id}",
        "{identity.source/invite-answers#/id}",
        "{identity.source@1invite-answers#/id}",
        "{identity.source@1/invite-answers#id}",
        "conformance-member",
    ] {
        assert_eq!(Placeholder::read(text), None, "{text}");
    }
}

/// Every string anywhere in `value`.
fn strings(value: &Value) -> Vec<&str> {
    match value {
        Value::String(text) => vec![text.as_str()],
        Value::Array(items) => items.iter().flat_map(strings).collect(),
        Value::Object(members) => members.values().flat_map(strings).collect(),
        _ => Vec::new(),
    }
}

#[test]
fn every_case_asks_a_body_and_names_only_cases_asked_before_it() {
    let mut asked_before: Vec<String> = Vec::new();
    for capability in crate::capabilities::all() {
        for case in cases(&capability) {
            assert!(case.asks.is_object(), "{}/{}", capability.name, case.case);
            if case.case == REFUSES_WITHOUT_THE_KEY {
                assert_eq!(case.asks, Value::Object(serde_json::Map::new()));
            }
            for text in strings(&case.asks) {
                if let Some(Placeholder::Answered { case: named, .. }) = Placeholder::read(text) {
                    assert!(
                        asked_before.contains(&named),
                        "{}/{} names {named}, which is not asked before it",
                        capability.name,
                        case.case
                    );
                }
            }
            asked_before.push(format!(
                "{}@{}/{}",
                capability.name, capability.major, case.case
            ));
        }
    }
    assert!(asked_before.contains(&format!("media.serve@1/{}", answers("mint"))));
}

#[test]
fn a_curator_is_asked_the_cases_of_each_kind_it_files_and_no_other() {
    let curate = crate::capabilities::library::capability();
    let named = |files: &[&str]| -> Vec<String> {
        let files: Vec<String> = files.iter().map(|one| (*one).to_owned()).collect();
        super::filed(&curate, &files)
            .into_iter()
            .map(|case| case.case)
            .collect()
    };
    let films = named(&["movies"]);
    assert!(
        films.contains(&"lookup-answers-movies".to_owned()),
        "{films:?}"
    );
    assert!(films.contains(&"records-answers-movies".to_owned()));
    assert!(films.contains(&"register_root_folder-answers-movies".to_owned()));
    assert!(films.contains(&"download_clients-answers".to_owned()));
    assert!(!films
        .iter()
        .any(|case| case.ends_with("-tv") || case.ends_with("-music")));

    let music = named(&["music"]);
    assert!(
        music.contains(&"records-answers-music".to_owned()),
        "{music:?}"
    );
    assert!(!music.iter().any(|case| case.starts_with("lookup-answers")));

    let books = named(&["books"]);
    assert!(
        books.contains(&"register_root_folder-answers-books".to_owned()),
        "{books:?}"
    );
    assert!(books.contains(&"records-answers-books".to_owned()));
    assert!(!books.iter().any(|case| case.starts_with("lookup-answers")));

    let every = cases(&curate).len();
    assert_eq!(named(&["tv", "movies", "music", "books"]).len(), every);
}

#[test]
fn a_kind_bound_case_posts_its_own_kind() {
    let curate = crate::capabilities::library::capability();
    let asks = |case: &str| {
        cases(&curate)
            .into_iter()
            .find(|one| one.case == case)
            .map(|one| one.asks)
            .unwrap_or_default()
    };
    assert_eq!(
        asks("lookup-answers-movies").pointer("/kind"),
        Some(&Value::from("movies"))
    );
    assert_eq!(
        asks("lookup-answers-tv").pointer("/kind"),
        Some(&Value::from("tv"))
    );
    assert_eq!(
        asks("carry-answers-music").pointer("/sort"),
        Some(&Value::from("artist"))
    );
    assert_eq!(
        asks("register_root_folder-answers-music").pointer("/folder/media_type"),
        Some(&Value::from("music"))
    );
    assert_eq!(
        asks("carry-answers-books").pointer("/sort"),
        Some(&Value::from("author"))
    );
    assert_eq!(
        asks("register_root_folder-answers-books").pointer("/folder/media_type"),
        Some(&Value::from("books"))
    );
    assert_eq!(
        asks("register_download_client-answers").pointer("/client/protocol"),
        Some(&Value::from("sabnzbd"))
    );
}
