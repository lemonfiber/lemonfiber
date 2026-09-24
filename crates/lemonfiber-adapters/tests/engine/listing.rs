//! What the engine lists: containers, images, and the API it agrees on.

use crate::{fake, LISTING};
use lemonfiber_adapters::Daemon;
use lemonfiber_ports::docker::{Engine as _, Failure, Health, Images as _, Lifecycle};

#[cfg(unix)]
#[tokio::test]
async fn lists_what_the_engine_says_is_there_and_what_it_left_behind() {
    let engine = fake::engine(
        "list",
        vec![(
            "containers/json",
            fake::Reply::Body(200, LISTING.to_owned()),
        )],
    );

    let listed = Daemon::at(&engine.socket).list("lemonfiber").await;
    assert_eq!(
        listed.ok().map(|containers| containers
            .into_iter()
            .map(|found| (found.service, found.lifecycle, found.health, found.exit))
            .collect::<Vec<_>>()),
        Some(vec![
            (
                "sonarr".to_owned(),
                Lifecycle::Running,
                Health::Healthy,
                None
            ),
            (
                "gluetun".to_owned(),
                Lifecycle::Exited,
                Health::None,
                Some(137)
            ),
        ])
    );
    engine.stop().await;
}

/// What the engine says it has pulled, and what is built on it.
///
/// One image two containers stand on — one of them this project's and one of them
/// nothing's, which is the case a removal has to tell apart — and one image with no
/// tag and a size the daemon never calculated.
#[cfg(unix)]
const IMAGES: &str = concat!(
    r#"[{"Id":"sha256:aa","ParentId":"","RepoTags":["lscr.io/linuxserver/sonarr:4.0.15"],"#,
    r#""RepoDigests":[],"Created":0,"Size":400,"SharedSize":-1,"Labels":{},"Containers":2},"#,
    r#"{"Id":"sha256:bb","ParentId":"","RepoTags":[],"RepoDigests":[],"Created":0,"#,
    r#""Size":-1,"SharedSize":-1,"Labels":{},"Containers":0}]"#
);

/// Every container on the machine, whatever project it is under — which is what the
/// unfiltered listing answers with, and the only way to see the one outside Compose.

#[cfg(unix)]
const EVERYWHERE: &str = concat!(
    r#"[{"Id":"c1","Image":"lscr.io/linuxserver/sonarr:4.0.15","ImageID":"sha256:aa","#,
    r#""Labels":{"com.docker.compose.project":"lemonfiber","#,
    r#""com.docker.compose.service":"sonarr"},"State":"running"},"#,
    r#"{"Id":"c2","Image":"lscr.io/linuxserver/sonarr:4.0.15","ImageID":"sha256:aa","#,
    r#""Labels":{},"State":"running"}]"#
);

/// The images the engine has, with who is standing on each.
///
/// The adapter asks two routes and joins them, which is the whole of what this
/// proves: a name alone would not say whether removing an image takes something
/// outside this project with it, and the join is where that answer comes from.

#[cfg(unix)]
#[tokio::test]
async fn lists_the_images_it_has_pulled_and_who_is_standing_on_each() {
    let engine = fake::engine(
        "images",
        vec![
            ("images/json", fake::Reply::Body(200, IMAGES.to_owned())),
            (
                "containers/json",
                fake::Reply::Body(200, EVERYWHERE.to_owned()),
            ),
        ],
    );

    let listed = Daemon::at(&engine.socket).images().await;
    assert_eq!(
        listed.ok().map(|images| images
            .into_iter()
            .map(|image| (image.tags, image.bytes, image.projects))
            .collect::<Vec<_>>()),
        Some(vec![
            (
                vec!["lscr.io/linuxserver/sonarr:4.0.15".to_owned()],
                400,
                // The empty one is the container under no Compose project, which is
                // exactly as broken by a removal as another project's would be.
                vec![String::new(), "lemonfiber".to_owned()],
            ),
            // Untagged, and a size the daemon says it never calculated — which
            // becomes nothing rather than a wrapped figure.
            (Vec::new(), 0, Vec::new()),
        ])
    );
    engine.stop().await;
}

/// An engine that is not there cannot say what it has pulled, and says so rather
/// than answering with none.
#[cfg(unix)]
#[tokio::test]
async fn an_absent_engine_will_not_say_what_it_has_pulled() {
    let nowhere = std::path::PathBuf::from("/lemonfiber/no/such/engine.sock");

    let refused = Daemon::at(&nowhere).images().await;

    assert!(matches!(refused, Err(Failure::Unreachable { .. })));
}

/// An engine that answers and then refuses the listing is refused too.
///
/// A different path from an engine that was never there, and a real one: the socket
/// is accepted, the API version is agreed, and only then does the route say no. What
/// must not happen is the refusal becoming an empty listing — an operator shown no
/// images would conclude there are none and remove nothing, or worse, believe a
/// cleanup had run.
#[cfg(unix)]
#[tokio::test]
async fn an_engine_that_will_not_list_its_images_refuses_rather_than_listing_none() {
    let engine = fake::engine(
        "images-refused",
        vec![(
            "images/json",
            fake::Reply::Body(500, r#"{"message":"database is locked"}"#.to_owned()),
        )],
    );

    let said = Daemon::at(&engine.socket).images().await.err().map_or_else(
        || "it answered with a listing".to_owned(),
        |failure| failure.to_string(),
    );

    assert!(
        said.contains("not reachable"),
        "a daemon that refuses a route has refused it: {said}"
    );
    engine.stop().await;
}

#[cfg(unix)]
#[tokio::test]
async fn the_engine_s_own_api_version_is_agreed_before_anything_is_asked_of_it() {
    let mut engine = fake::engine(
        "negotiate",
        vec![("containers/json", fake::Reply::Body(200, "[]".to_owned()))],
    );

    let daemon = Daemon::at(&engine.socket);
    let listed = daemon.list("lemonfiber").await;
    assert_eq!(listed.ok().map(|containers| containers.len()), Some(0));

    // Asked twice, to prove the agreement is reached once and then kept. A
    // dashboard polling every second must not reopen that conversation
    // sixty times a minute.
    let _ = daemon.list("lemonfiber").await;

    let asked = engine.asked_for();
    assert_eq!(
        (
            asked.first().map(|path| path.contains("version")),
            asked.iter().filter(|path| path.contains("version")).count(),
            asked
                .iter()
                .filter(|path| path.contains("containers/json"))
                .count(),
        ),
        (Some(true), 1, 2),
        "the version is settled first, and settled once: {asked:?}"
    );
    engine.stop().await;
}

#[cfg(unix)]
#[tokio::test]
async fn the_engine_is_asked_only_about_this_project_s_containers() {
    let mut engine = fake::engine(
        "filter",
        vec![("containers/json", fake::Reply::Body(200, "[]".to_owned()))],
    );

    let _ = Daemon::at(&engine.socket).list("housemedia").await;
    let listing = engine
        .asked_for()
        .into_iter()
        .find(|path| path.contains("containers/json"));

    assert_eq!(
        listing
            .as_deref()
            .map(|path| path.contains("compose.project") && path.contains("housemedia")),
        Some(true),
        "narrowing happens at the engine, not after nineteen containers crossed the socket: {listing:?}"
    );
    engine.stop().await;
}

#[tokio::test]
async fn an_engine_that_is_not_listening_is_reported_as_unreachable() {
    let nowhere = std::path::PathBuf::from("/tmp/lemonfiber-no-such-engine.sock");
    let outcome = Daemon::at(&nowhere).list("lemonfiber").await;
    assert!(
        matches!(outcome, Err(Failure::Unreachable { .. })),
        "{outcome:?}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_daemon_that_refuses_is_quoted_rather_than_paraphrased() {
    let engine = fake::engine(
        "refused",
        vec![(
            "containers/json",
            fake::Reply::Body(
                500,
                r#"{"message":"permission denied while trying to connect"}"#.to_owned(),
            ),
        )],
    );

    let outcome = Daemon::at(&engine.socket).list("lemonfiber").await;
    assert_eq!(
        outcome.err().map(|failure| failure.to_string()),
        Some(
            "the container engine is not reachable: \
             permission denied while trying to connect"
                .to_owned()
        ),
        "the daemon's own sentence is what the operator needs to read"
    );
    engine.stop().await;
}

/// A socket path that is not a socket is reported, not dialled.
///
/// The client library checks only that the path exists, so a plain file there gets
/// past it and fails on the first request — which is the connection failing, not a
/// route. An operator who pointed `DOCKER_HOST` at the wrong thing gets a refusal
/// rather than a decoding error about a reply that never came.
#[cfg(unix)]
#[tokio::test]
async fn a_path_that_exists_and_is_not_a_socket_is_refused_at_the_connection() {
    let path = std::path::PathBuf::from(format!("/tmp/lf-{}-not-a-socket", std::process::id()));
    let _ = std::fs::write(&path, "this is a file");

    let refused = Daemon::at(&path).list("lemonfiber").await;

    let _ = std::fs::remove_file(&path);
    assert!(
        matches!(refused, Err(Failure::Unreachable { .. })),
        "a path that is not a socket is the engine being unreachable"
    );
}

/// An engine that lists its images and will not say what is running refuses.
///
/// The two answers are joined to decide whether removing an image takes something
/// outside this project with it, so half of it is not a smaller answer — it is the
/// wrong one. An operator shown images with nothing standing on them would remove
/// the lot.
#[cfg(unix)]
#[tokio::test]
async fn an_engine_that_will_not_say_what_is_running_refuses_the_image_listing() {
    let engine = fake::engine(
        "images-half",
        vec![
            ("images/json", fake::Reply::Body(200, IMAGES.to_owned())),
            (
                "containers/json",
                fake::Reply::Body(500, r#"{"message":"database is locked"}"#.to_owned()),
            ),
        ],
    );

    let refused = Daemon::at(&engine.socket).images().await;

    assert!(
        matches!(refused, Err(Failure::Unreachable { .. })),
        "half the join is not a listing"
    );
    engine.stop().await;
}
