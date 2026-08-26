//! What a read refuses, asked for through the router a request meets.
//!
//! The other half of the same seam. What a read *answers* is next door; this is
//! every way of asking that is not answered — a name the stack does not declare, a
//! parameter a read does not take, a count past what it will gather — and the
//! refusal each one earns.
//!
//! Apart because they are two questions, and together they outgrew what one file
//! is held to: three reads landing at once put the pair past the cap, and no one
//! of them crossed it alone.

mod reading;

use reading::*;
#[tokio::test]
async fn a_form_this_stack_does_not_declare_is_refused_as_missing() {
    // The same reading arrived at from a stack rather than from a compiled-in
    // table: the form named is the whole of what was asked for, and there is no
    // such form.
    let seen = asked(world(running(), stack()), "/api/forms?form=nonsense").await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::NOT_FOUND
            && body.starts_with(r#"{"api_version":1,"kind":"error","data":{"code":"FORM-2""#)),
        "a form the stack does not declare is missing, not a failure of this machine"
    );
}

#[tokio::test]
async fn narrowing_the_services_to_a_form_that_is_not_one_is_refused_as_missing() {
    // Two reads take the same parameter, and a name that names nothing names
    // nothing in both of them.
    let seen = asked(world(running(), stack()), "/api/services?form=nonsense").await;
    assert!(
        seen.is_some_and(
            |(status, body)| status == StatusCode::NOT_FOUND && body.contains(r#""code":"FORM-2""#)
        ),
        "the reading does not turn on which endpoint asked"
    );
}

#[tokio::test]
async fn a_command_that_could_not_be_carried_out_answers_with_the_failure() {
    // The envelope a failure gets under `--json`, because a caller that asked for
    // something it could parse asked about the failures most of all.
    //
    // Nothing about the request was wrong here, so it keeps the status that says
    // so: a stack this machine cannot read is this machine's to fix.
    let seen = asked(world(running(), nowhere()), "/api/status").await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::INTERNAL_SERVER_ERROR
            && body.starts_with(r#"{"api_version":1,"kind":"error","data":{"code":"#)),
        "a failure is an envelope too"
    );
}

#[tokio::test]
async fn a_log_read_that_could_not_be_opened_answers_with_the_failure() {
    let seen = asked(world(Reporting::absent(), stack()), "/api/logs").await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::INTERNAL_SERVER_ERROR
            && body.starts_with(r#"{"api_version":1,"kind":"error""#)),
        "an engine that is not there is said in the same envelope"
    );
}

#[tokio::test]
async fn a_request_carrying_no_token_never_reaches_a_read() {
    assert_eq!(
        answered(
            world(running(), stack()),
            "/api/status",
            &[("host", "127.0.0.1:8471")]
        )
        .await,
        Some((
            StatusCode::FORBIDDEN,
            "This request carried no token or session this run admits.".to_owned()
        ))
    );
}

#[tokio::test]
async fn a_path_this_surface_does_not_serve_is_refused_before_it_is_looked_for() {
    // The guard wraps the whole tree, so which paths exist is not something an
    // unauthenticated caller can map by watching the status change.
    let turned_away = answered(
        world(running(), stack()),
        "/api/secrets",
        &[("host", "127.0.0.1:8471")],
    )
    .await;
    assert!(
        turned_away.is_some_and(|(status, _)| status == StatusCode::FORBIDDEN),
        "an unknown path with no token is refused, not reported missing"
    );

    let carrying = asked(world(running(), stack()), "/api/secrets").await;
    assert!(
        carrying.is_some_and(|(status, _)| status == StatusCode::NOT_FOUND),
        "carrying the token, it is simply not there"
    );
}

#[tokio::test]
async fn an_answer_that_could_not_be_rendered_is_not_invented() {
    // Reachable only by being called: these payloads are plain data, so no command
    // can produce one. Answering with an empty document would be worse than saying
    // plainly that there is no answer.
    let response = enveloped(StatusCode::OK, None);
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

    let body = to_bytes(response.into_body(), usize::MAX).await;
    assert_eq!(
        body.ok().as_deref(),
        Some("This answer could not be rendered.".as_bytes())
    );
}

#[test]
fn a_read_no_name_reaches_is_refused_rather_than_invented() {
    // Reachable only by being called: the router serves a fixed set of paths, so no
    // request arrives here under a name this surface does not answer. What reaches
    // it is another surface asking by name, and a name that reaches no read has to
    // be told so rather than quietly answered with something else.
    assert_eq!(
        reads::named("/api/secrets", reads::Wanted::default()),
        Err(reads::NO_SUCH_READ)
    );
}

#[tokio::test]
async fn every_read_this_surface_offers_by_name_is_one_it_serves() {
    // The converse of the table being reachable by name: a name offered to another
    // surface and served by nothing here would be a read only that surface could
    // make. Asked for rather than read out of the source, so what is proven is that
    // the path answers and not that somebody wrote it down twice.
    let mut missing: Vec<&str> = Vec::new();
    for read in reads::OFFERED {
        let answered = asked(world(running(), stack()), read).await;
        if answered.is_none_or(|(status, _)| status == StatusCode::NOT_FOUND) {
            missing.push(read);
        }
    }

    assert!(missing.is_empty(), "{missing:?}");
}

#[tokio::test]
async fn a_setting_asked_for_under_a_name_this_read_does_not_take_is_refused() {
    // The defect, in the spelling it was found in. `keys` is one letter from `key`,
    // and the answer to it was every setting this stack has: the values are withheld
    // and the names are not, so a mistyped request handed over the map.
    let seen = asked(
        configured("misspelled", &kept()),
        "/api/config?keys=LEMONFIBER_USENET",
    )
    .await;

    assert_eq!(
        seen.as_ref().map(|(status, _)| *status),
        Some(StatusCode::BAD_REQUEST),
        "a parameter this read does not take is refused"
    );
    assert!(
        seen.as_ref()
            .is_some_and(|(_, body)| body.contains(r#""code":"READ-1""#)),
        "and refused under the code that says which mistake it was: {seen:?}"
    );
    assert!(
        seen.is_some_and(|(_, body)| !body.contains("SONARR_API_KEY")),
        "and nothing about the settings is answered on the way past"
    );
}

#[tokio::test]
async fn the_refusal_names_what_the_read_does_take() {
    // A caller here has misspelled something, and what the read takes is short
    // enough to be the answer rather than a pointer at one.
    let seen = asked(world(running(), stack()), "/api/config?keys=x").await;
    assert!(
        seen.is_some_and(|(_, body)| body.contains("It takes key.")),
        "the way forward is the name that was meant"
    );
}

#[tokio::test]
async fn a_read_that_takes_nothing_refuses_a_parameter_all_the_same() {
    // The reads a check written per handler would never have covered: they took no
    // query string, so there was nowhere to write one.
    let seen = asked(world(running(), stack()), "/api/status?nonsense=1").await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::BAD_REQUEST
            && body.contains(r#""code":"READ-1""#)
            && body.contains("takes no parameters at all")),
        "a read with nothing to narrow by is still a read that was asked wrongly"
    );
}

#[tokio::test]
async fn the_glossary_refuses_a_misspelled_word_rather_than_listing_every_word() {
    let seen = asked(world(running(), stack()), "/api/explain?words=indexer").await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::BAD_REQUEST
            && body.contains(r#""code":"READ-1""#)
            && !body.contains(r#""word":"hardlink""#)),
        "the whole vocabulary is not the answer to a question about one word"
    );
}

#[tokio::test]
async fn the_checks_refuse_a_misspelled_narrowing_rather_than_running_the_suite() {
    // A narrowing with two letters swapped, which ran every check there is —
    // including the ones that reach the services — for a request about the disk.
    let seen = asked(world(running(), stack()), "/api/checks?onyl=storage").await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::BAD_REQUEST
            && body.contains(r#""code":"READ-1""#)
            && !body.contains(r#""kind":"doctor""#)),
        "a narrowing that was misspelled is not a request for everything"
    );
}

#[tokio::test]
async fn the_checks_refuse_the_widening_that_would_stop_them_being_a_read() {
    // The disturbing checks are reachable from a browser and not from here. This
    // endpoint answers a `GET`, and a run that took the tunnel away to prove the
    // killswitch would be a `GET` that stopped somebody's downloads — so the word is
    // refused at this door rather than honoured, and the action beside it is where
    // asking for it means asking for it.
    let seen = asked(world(running(), stack()), "/api/checks?disruptive=1").await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::BAD_REQUEST
            && body.contains(r#""code":"READ-1""#)
            && body.contains("It takes only.")
            && !body.contains(r#""kind":"doctor""#)),
        "a read that disturbed something would not be a read"
    );
}

#[tokio::test]
async fn a_log_read_refuses_a_parameter_it_does_not_take() {
    // The one read that reaches no command still arrives at the same door.
    let seen = asked(world(running(), stack()), "/api/logs?lines=10").await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::BAD_REQUEST
            && body.contains(r#""code":"READ-1""#)),
        "a read with no command behind it is not a read exempt from this"
    );
}

#[tokio::test]
async fn what_to_follow_given_twice_is_refused_rather_than_answered_for_the_first() {
    // Two titles named and one traced, with nothing said about the other: the
    // request was answered, about something it did not ask.
    let seen = asked(
        world(running(), stack()),
        "/api/trace?term=the+expanse&term=dune",
    )
    .await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::BAD_REQUEST
            && body.contains(r#""code":"READ-2""#)
            && !body.contains("expanse")),
        "which of the two was meant is not something this can work out"
    );
}

#[tokio::test]
async fn a_form_may_be_named_more_than_once_because_it_names_one_of_several() {
    // The other half of the rule: two parameters carry lists, and naming a second
    // form is a wider request that was asked for.
    let seen = asked(
        world(running(), stack()),
        "/api/services?form=library&form=library",
    )
    .await;
    assert!(
        seen.is_some_and(|(status, _)| status == StatusCode::OK),
        "a repeat is refused only where one value was asked for"
    );
}

#[tokio::test]
async fn every_read_this_surface_serves_refuses_a_parameter_no_read_takes() {
    // The guard the sweep is finished by. Written against the whole list rather than
    // the reads that were known to be wrong, so the next read added arrives holding
    // this or fails here.
    let mut accepted: Vec<&str> = Vec::new();
    for read in reads::OFFERED
        .iter()
        .copied()
        .chain(std::iter::once(reads::LOGS))
    {
        let path = format!("{read}?nonsense=1");
        let seen = asked(world(running(), stack()), &path).await;
        let refused = seen.is_some_and(|(status, body)| {
            status == StatusCode::BAD_REQUEST && body.contains(r#""code":"READ-1""#)
        });
        if !refused {
            accepted.push(read);
        }
    }

    assert!(accepted.is_empty(), "{accepted:?}");
}

#[test]
fn a_name_that_reaches_no_read_takes_no_parameter_either() {
    // Asked of the table rather than through the router, because no path serves a
    // name like this — and a read added without a row must refuse everything rather
    // than accept everything, which is the direction this settles.
    let refused = reads::wanted("/api/secrets", Some("key=LEMONFIBER_USENET"));
    let code = refused.err().map(|problem| problem.code.as_str());

    assert_eq!(code, Some("READ-1"));
}

#[tokio::test]
async fn a_line_count_past_what_this_read_will_gather_is_refused() {
    // The scrollback is gathered whole before any of it is answered, so the number
    // asked for here is the number of lines this machine holds at once.
    assert_eq!(
        asked(world(running(), stack()), "/api/logs?tail=4294967295").await,
        Some((
            StatusCode::BAD_REQUEST,
            "How many lines to begin with must be a number, and no more than 10000.".to_owned()
        ))
    );
}

#[tokio::test]
async fn a_line_count_at_the_ceiling_is_still_asked_for() {
    // The ceiling is a ceiling and not a fence one short of it.
    let seen = asked(world(running(), stack()), "/api/logs?tail=10000").await;
    assert_eq!(seen.map(|(status, _)| status), Some(StatusCode::OK));
}

#[tokio::test]
async fn the_enumeration_takes_nothing_and_refuses_a_parameter() {
    // An enumeration a caller could narrow is one an operator could be shown half
    // of, and half of everything that leaves this machine reads as the whole of it.
    let refused = reads::wanted(reads::OUTBOUND, Some("reach=registry"));

    assert_eq!(
        refused.err().map(|problem| problem.code.as_str()),
        Some("READ-1")
    );
}
