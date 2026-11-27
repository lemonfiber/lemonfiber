//! Whom a name for work is answered to: whoever started the work, and the operator.
//!
//! Anybody else holding the name is told what a name this run never issued tells them,
//! on every route a name is redeemed, released or forgotten at, so holding somebody
//! else's name says nothing about whether the work exists, and does nothing to it.

use std::sync::Arc;

use axum::http::StatusCode;
use lemonfiber_api::admission::{Caller, Keyed};
use lemonfiber_api::jobs::{Job, Jobs, Standing, A_JOB, FORGET};
use lemonfiber_core::app::Command;
use lemonfiber_core::keys::Scope;

use crate::reading::{refused, Refusal};
use crate::what_became_of_a_job::{at, guarding, minted, there};

/// A key by its name, admitting what `scope` says.
fn key(name: &str, scope: Scope) -> Caller {
    Caller::Key(Keyed {
        name: name.to_owned(),
        scope,
    })
}

/// A household member's own session.
fn member(id: &str) -> Caller {
    Caller::Member(id.to_owned())
}

/// A key scoped to the household member `id`.
fn members_key(id: &str) -> Caller {
    key(
        "phone",
        Scope::Member {
            id: id.to_owned(),
            name: "Robin".to_owned(),
        },
    )
}

/// A guard over a location that will not go away, so still running, started by `by`.
async fn started_by(jobs: &Jobs, by: &Caller) -> Job {
    let job = minted();
    let guard = Command::Watch {
        forms: vec!["library".to_owned()],
    };
    jobs.start(&job, "watch", guard, Arc::new(guarding(there())), by)
        .await;
    job
}

/// What a name this run never issued is answered with.
fn absent() -> Option<String> {
    refused(Refusal::NoSuchJob, "No work in this run goes by that name.").map(|(_, body)| body)
}

#[tokio::test]
async fn a_job_is_answered_to_the_caller_that_started_it() {
    for by in [
        key("ha", Scope::Read),
        key("ha", Scope::Act),
        member("a7f3"),
        members_key("a7f3"),
    ] {
        let jobs = Jobs::default();
        let job = started_by(&jobs, &by).await;
        let (status, body) = at("GET", A_JOB, job.as_str(), jobs, by.clone()).await;
        assert_eq!(status, StatusCode::ACCEPTED.as_u16(), "{by:?}: {body}");
    }
}

#[tokio::test]
async fn the_operator_is_answered_about_work_anybody_started() {
    for asking in [Caller::Machine, Caller::Operator] {
        let jobs = Jobs::default();
        let job = started_by(&jobs, &key("ha", Scope::Act)).await;
        let (status, body) = at("GET", A_JOB, job.as_str(), jobs, asking.clone()).await;
        assert_eq!(status, StatusCode::ACCEPTED.as_u16(), "{asking:?}: {body}");
    }
}

#[tokio::test]
async fn anybody_else_is_answered_as_for_a_name_never_issued_on_every_route() {
    let strangers = [
        key("other", Scope::Read),
        key("ha", Scope::Read),
        member("a7f3"),
        members_key("b9e1"),
    ];
    for stranger in strangers {
        let jobs = Jobs::default();
        let job = started_by(&jobs, &key("ha", Scope::Act)).await;
        for (method, route) in [("GET", A_JOB), ("DELETE", A_JOB), ("POST", FORGET)] {
            let (status, body) =
                at(method, route, job.as_str(), jobs.clone(), stranger.clone()).await;
            assert_eq!(
                status,
                StatusCode::NOT_FOUND.as_u16(),
                "{stranger:?} {method} {route}"
            );
            assert_eq!(Some(body), absent(), "{stranger:?} {method} {route}");
        }
        assert_eq!(
            jobs.about(job.as_str(), &Caller::Machine)
                .await
                .map(|work| work.standing),
            Some(Standing::Running),
            "{stranger:?} neither ended nor forgot work it did not start"
        );
    }
}

#[tokio::test]
async fn a_members_session_and_a_key_scoped_to_them_are_two_callers() {
    let jobs = Jobs::default();
    let job = started_by(&jobs, &member("a7f3")).await;
    let (status, _) = at(
        "GET",
        A_JOB,
        job.as_str(),
        jobs.clone(),
        members_key("a7f3"),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND.as_u16());

    let jobs = Jobs::default();
    let job = started_by(&jobs, &members_key("a7f3")).await;
    let (status, _) = at("GET", A_JOB, job.as_str(), jobs, member("a7f3")).await;
    assert_eq!(status, StatusCode::NOT_FOUND.as_u16());
}
