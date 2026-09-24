//! What leaves this machine, where the services come from, the disk and the line.

use crate::reading;
use crate::reading::*;

#[tokio::test]
async fn what_leaves_this_machine_is_the_envelope_the_command_renders() {
    // The read a browser has the most reason to want and the least ability to
    // answer for itself: a page sees the requests it makes, and nothing at all of
    // what the process behind it does.
    let expected = as_the_command_renders_it(&world(running(), stack()), Command::Outbound).await;

    assert!(expected.is_some(), "the command answered");
    assert_eq!(
        asked(world(running(), stack()), reads::OUTBOUND).await,
        expected.map(|body| (StatusCode::OK, body))
    );
}

#[tokio::test]
async fn what_leaves_this_machine_carries_the_switch_beside_each_request() {
    // Written out rather than derived, so a second serialisation could not pass
    // this by agreeing with itself. The switch is the field that makes the list
    // something an operator can act on rather than something they can only read.
    let seen = asked(world(running(), stack()), reads::OUTBOUND).await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::OK
            && body.starts_with(r#"{"api_version":1,"kind":"outbound","data":{"ours":["#)
            && body.contains(r#""reach":"registry""#)
            && body.contains(r#""switch":"LEMONFIBER_REACH_REGISTRY""#)
            && body.contains(r#""theirs":[{"service":"prowlarr""#)),
        "the list a browser is served"
    );
}

#[tokio::test]
async fn where_the_services_come_from_is_the_envelope_the_command_renders() {
    // The read that makes the open-source claim checkable, and the one a page has no
    // way of answering for itself: what is bundled is written in the stack this
    // machine runs, and a browser can see neither.
    let expected = as_the_command_renders_it(&world(running(), stack()), Command::Provenance).await;

    assert!(expected.is_some(), "the command answered");
    assert_eq!(
        asked(world(running(), stack()), reads::PROVENANCE).await,
        expected.map(|body| (StatusCode::OK, body))
    );
}

#[tokio::test]
async fn what_each_service_is_for_is_the_envelope_the_command_renders() {
    // The read that answers what a list of nineteen names cannot: a page can print
    // `bazarr` beside `prowlarr` and has no way of saying which of them the person
    // reading it would miss.
    let expected = as_the_command_renders_it(&world(running(), stack()), Command::Catalogue).await;

    assert!(expected.is_some(), "the command answered");
    assert_eq!(
        asked(world(running(), stack()), reads::CATALOGUE).await,
        expected.map(|body| (StatusCode::OK, body))
    );
}

#[tokio::test]
async fn where_the_services_come_from_carries_the_licence_the_project_and_the_pin() {
    // Written out rather than derived, so a second serialisation could not pass this
    // by agreeing with itself. All three fields, because each on its own is something
    // somebody would have to take on trust: an identifier with no project to check it
    // against, or a project with no version the check would be about.
    let seen = asked(world(running(), stack()), reads::PROVENANCE).await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::OK
            && body.starts_with(r#"{"api_version":1,"kind":"provenance","data":{"services":[{"#)
            && body.contains(r#""license":"GPL-3.0-only""#)
            && body.contains(r#""upstream":"https://github.com/Prowlarr/Prowlarr""#)
            && body.contains(r#""image":"lscr.io/linuxserver/prowlarr""#)),
        "where each service comes from, as a browser is served it"
    );
}

#[tokio::test]
async fn what_each_service_is_for_carries_the_cost_of_going_without_it() {
    // Written out rather than derived, so a second serialisation could not pass this
    // by agreeing with itself. Both fields, because a description on its own does not
    // tell an operator whether a failure is serious — and the key the removals arrive
    // under, because a caller cannot tell a stack that dropped nothing from one that
    // keeps no record unless the key is there either way. The key rather than its
    // contents: the stack is a submodule and what it records moves with its pin.
    let seen = asked(world(running(), stack()), reads::CATALOGUE).await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::OK
            && body.starts_with(r#"{"api_version":1,"kind":"catalogue","data":{"services":[{"#)
            && body.contains(r#""id":"prowlarr""#)
            && body.contains(r#""without_it":"#)
            && body.contains(r#""criticality":"critical""#)
            && body.contains(r#""removed":["#)),
        "the listing a browser is served"
    );
}

/// A machine whose data location is a real directory this test made, holding one
/// file — so the walk has something to answer about.
///
/// The volume itself is described through a fake rather than by this machine's own
/// disk, and that is what makes the parity below mean anything: the free space on a
/// real volume moves between two readings taken a moment apart, so comparing what a
/// browser is served against what a shell prints would be comparing two moments and
/// would fail on a machine that happened to be busy.
fn measuring(named: &str) -> Ctx {
    let dir = std::env::temp_dir().join(format!("lemonfiber-space-{}-{named}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(dir.join("films"));
    let _ = std::fs::write(dir.join("films").join("a.mkv"), "0123456789");
    holding(
        running(),
        stack(),
        Settings {
            data_root: Some(dir),
            ..Settings::default()
        },
    )
    .with_filesystem(lemonfiber_fixtures::files::Files::empty())
}

#[tokio::test]
async fn where_the_disk_went_is_the_envelope_the_command_renders() {
    // A page can be told how much room is left; it cannot walk a tree or count the
    // names pointing at a file, and the whole answer here is built out of those.
    let expected =
        as_the_command_renders_it(&measuring("rendered"), Command::Space { confirm: false }).await;

    assert!(expected.is_some(), "the command answered");
    assert_eq!(
        asked(measuring("rendered"), reads::SPACE).await,
        expected.map(|body| (StatusCode::OK, body))
    );
}

#[tokio::test]
async fn how_the_line_is_shared_is_the_envelope_the_command_renders() {
    // A page can be told a number; it cannot sign in to a download client, read
    // what that client is limited to and read what it is moving beside it, and the
    // whole answer here is built out of those three.
    let asking = || Command::Bandwidth(lemonfiber_core::app::BandwidthAsked::default());
    let expected = as_the_command_renders_it(&measuring("line"), asking()).await;

    assert!(expected.is_some(), "the command answered");
    assert_eq!(
        asked(measuring("line"), reads::BANDWIDTH).await,
        expected.map(|body| (StatusCode::OK, body))
    );
}

#[tokio::test]
async fn how_the_line_is_shared_always_says_what_it_will_never_limit() {
    // Written out rather than derived. The two fears an operator brings to a
    // bandwidth feature are that it throttles the household's own viewing and that
    // it meddles with the machine, and a report that left either to be inferred is
    // one that gets read as doing them.
    let seen = asked(measuring("untouched"), reads::BANDWIDTH).await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::OK
            && body.starts_with(r#"{"api_version":1,"kind":"bandwidth","data":{"restraint":"#)
            && body.contains("watching from your own library")
            && body.contains("does not shape the machine's traffic")
            && body.contains(r#""applied":false"#)),
        "the account a browser is served, with nothing declared"
    );
}

#[tokio::test]
async fn where_the_disk_went_carries_the_volume_and_takes_nothing() {
    // Written out rather than derived, so a second serialisation could not pass this
    // by agreeing with itself. A read never removes anything, so the field that says
    // what a cleanup took is absent rather than empty.
    let seen = asked(measuring("carried"), reads::SPACE).await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::OK
            && body.starts_with(
                r#"{"api_version":1,"kind":"space","data":{"volumes":[{"role":"data""#
            )
            && body.contains(r#""of":"tree","name":"films""#)
            && body.contains(r#""reclaimed":null"#)),
        "the account a browser is served, with nothing taken"
    );
}
