use lemonfiber_core::ports::filesystem::Beneath;
use lemonfiber_core::ports::http::{Method, Request};
use lemonfiber_core::ports::media::Kind;
use lemonfiber_core::ports::service::{Watched, Watching};
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::http::Answer;
use lemonfiber_fixtures::scratch::Scratch;

use super::super::context::a_context;
use super::{
    asking, keyed, moved, operation, reaching, running, taken, told, unwatched, watch, watching,
    watching_at, ADAPTER, PLUGIN,
};

#[test]
fn the_moved_plugin_is_bazarrs_with_an_adapter_speaking_the_subtitles_contract() {
    let installed = moved();

    assert_eq!(
        installed.as_ref().map(|one| one.plugin.as_str()),
        Some(PLUGIN)
    );
    assert!(
        installed.is_some_and(|one| one.services.iter().any(|placed| {
            placed.service == ADAPTER
                && placed.fronts.as_deref() == Some(PLUGIN)
                && placed
                    .speaks
                    .iter()
                    .any(|spoken| spoken == "subtitles.fetch@1")
        }))
    );
}

#[test]
fn each_operation_is_asked_on_the_subtitles_contracts_path() {
    assert_eq!(
        operation("watching"),
        "/lemonfiber/subtitles.fetch/v1/watching"
    );
}

#[test]
fn what_the_finder_holds_is_answered_on_the_watching_path_as_the_contract_carries_it() {
    let (at, answer) = watching(&unwatched());
    let held: Option<Watching> = match answer {
        Answer::Reply(200, body) => serde_json::from_str(&body).ok(),
        _ => None,
    };

    assert_eq!(at, operation("watching"));
    assert_eq!(
        held,
        Some(Watching {
            enabled: false,
            host: String::new(),
            port: 0,
            keyed: false,
        })
    );
}

#[test]
fn a_watch_is_answered_on_the_watch_path_with_what_the_script_says() {
    let (at, answer) = watch(taken());

    assert_eq!(at, operation("watch"));
    assert!(matches!(answer, Answer::Reply(204, _)));
}

#[test]
fn a_finder_moved_in_is_keyed_where_its_stack_is_and_one_keyed_holds_its_key_unwritten() {
    let over = |directory: &Scratch| {
        Source::External(Box::leak(directory.to_path_buf().into_boxed_path()))
    };
    let written = Scratch::new("subtitles-moved-in");
    let mut ctx = a_context().over(over(&written)).build();
    ctx.settings.env_file = Some(written.join(".env"));
    let _ = reaching(ctx);

    let shared = Scratch::new("subtitles-keyed");
    let mut beside = a_context().over(over(&shared)).build();
    beside.settings.env_file = Some(shared.join(".env"));
    let beside = keyed(beside);

    let at = |directory: &Scratch| lemonfiber_core::plugin::key_file(directory, ADAPTER);
    assert!(at(&written).exists());
    assert!(!at(&shared).exists());
    assert_eq!(
        beside.seams.confined.read(&at(&shared), &shared),
        Beneath::Read(super::super::adapted::KEY.to_owned())
    );
    assert!(shared
        .join(lemonfiber_core::config::paths::PLUGINS)
        .exists());
}

#[test]
fn a_finder_asked_through_its_fillers_holds_its_key_wherever_its_stack_is_and_writes_nothing() {
    let directory = Scratch::new("subtitles-asking");
    let mut ctx = a_context()
        .over(Source::External(Box::leak(
            directory.to_path_buf().into_boxed_path(),
        )))
        .build();
    ctx.settings.env_file = Some(directory.join(".env"));

    let ctx = asking(ctx);

    let at = lemonfiber_core::plugin::key_file(&directory, ADAPTER);
    assert!(!at.exists());
    assert!(!directory
        .join(lemonfiber_core::config::paths::PLUGINS)
        .exists());
    assert_eq!(
        ctx.seams.confined.read(&at, &directory),
        Beneath::Read(super::super::adapted::KEY.to_owned())
    );
}

#[test]
fn the_running_adapter_is_healthy_and_published_on_the_adapters_port() {
    assert!(super::super::adapted::runs_on_the_adapters_port(
        &running(),
        ADAPTER
    ));
}

#[test]
fn a_curator_watched_is_held_switched_on_at_its_address_with_a_key() {
    assert_eq!(
        watching_at("sonarr", 8989),
        Watching {
            enabled: true,
            host: "sonarr".to_owned(),
            port: 8989,
            keyed: true,
        }
    );
}

/// A request posted to `path` carrying `body`.
fn posted(path: &str, body: &str) -> Request {
    Request {
        method: Method::Post,
        url: format!("http://127.0.0.1:8080{path}"),
        headers: Vec::new(),
        body: Some(body.to_owned()),
        pinned: None,
    }
}

#[test]
fn what_a_pass_told_the_finder_is_each_watch_it_asked_and_nothing_else() {
    let tv = Watched {
        which: Kind::Tv,
        host: "sonarr".to_owned(),
        port: 8989,
        api_key: "the-key".to_owned(),
    };
    let sent = vec![
        posted(operation("watching"), r#"{"which":"tv"}"#),
        posted(
            operation("watch"),
            &serde_json::json!({ "watched": tv }).to_string(),
        ),
        posted(operation("watch"), r#"{"something":"else"}"#),
    ];

    assert_eq!(told(&sent), vec![tv]);
}
