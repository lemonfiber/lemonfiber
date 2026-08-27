//! What each read answers with, asked for through the router a request meets.
//!
//! Driven through the assembled router rather than by calling a handler, because
//! what a caller can reach is the thing worth holding still — and because the
//! guard every endpoint sits behind is part of what an endpoint answers.

mod reading;

use reading::*;
#[tokio::test]
async fn what_the_stack_declares_is_the_envelope_the_command_renders() {
    // The gap this read closed: the surface offers to start, stop and switch forms,
    // and until this endpoint existed a caller had to already know their names.
    let expected = as_the_command_renders_it(&world(running(), stack()), Command::Forms).await;

    assert!(expected.is_some(), "the command answered");
    assert_eq!(
        asked(world(running(), stack()), "/api/forms").await,
        expected.map(|body| (StatusCode::OK, body))
    );
}

#[tokio::test]
async fn the_forms_a_stack_declares_are_carried_in_their_own_envelope() {
    // Written out rather than derived, so a second serialisation could not pass
    // this by agreeing with itself.
    let seen = asked(world(running(), stack()), "/api/forms").await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::OK
            && body.starts_with(r#"{"api_version":1,"kind":"forms","data":{"forms":[{"#)
            && body.contains(r#""id":"library""#)),
        "every form the stack declares, under the forms kind"
    );
}

#[tokio::test]
async fn naming_a_form_says_what_starting_it_would_come_to() {
    // One endpoint over two commands, because the command line spells the two with
    // one word: naming none lists them, naming some resolves them.
    let expected = as_the_command_renders_it(
        &world(running(), stack()),
        Command::Preview {
            forms: vec!["library".to_owned()],
        },
    )
    .await;

    assert!(expected.is_some(), "the command answered");
    assert_eq!(
        asked(world(running(), stack()), "/api/forms?form=library").await,
        expected.map(|body| (StatusCode::OK, body))
    );
}

#[tokio::test]
async fn a_form_that_is_named_is_not_answered_with_the_whole_list() {
    // The mistake the two commands exist to keep apart: a request that named a
    // form and was handed the catalogue would look like it had been answered.
    let seen = asked(world(running(), stack()), "/api/forms?form=library").await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::OK
            && body.starts_with(r#"{"api_version":1,"kind":"preview""#)),
        "a named form is resolved rather than listed"
    );
}

#[tokio::test]
async fn what_the_stack_is_doing_is_the_envelope_the_command_renders() {
    // The whole of the contract in one assertion: the bytes a browser reads are
    // the bytes a script would have piped, produced by the same three calls.
    let expected = as_the_command_renders_it(
        &world(running(), stack()),
        Command::Ps { forms: Vec::new() },
    )
    .await;

    assert!(expected.is_some(), "the command answered");
    assert_eq!(
        asked(world(running(), stack()), "/api/status").await,
        expected.map(|body| (StatusCode::OK, body))
    );
}

#[tokio::test]
async fn what_the_stack_is_doing_is_carried_in_the_envelope_the_contract_states() {
    // Written out rather than derived, so a second serialisation could not pass
    // this by agreeing with itself.
    let seen = asked(world(running(), stack()), "/api/status").await;
    assert!(
        seen.is_some_and(
            |(_, body)| body.starts_with(r#"{"api_version":1,"kind":"status","data":{"forms":[],"#)
        ),
        "the envelope the whole stack is reported in"
    );
}

#[tokio::test]
async fn naming_a_form_narrows_what_the_services_read_reports() {
    let seen = asked(world(running(), stack()), "/api/services?form=library").await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::OK
            && body.contains(r#""forms":["library"]"#)
            && body.contains(r#""id":"jellyfin""#)),
        "a named form is the form reported on"
    );
}

#[tokio::test]
async fn naming_no_form_reports_on_the_whole_stack() {
    let seen = asked(world(running(), stack()), "/api/services").await;
    assert!(
        seen.is_some_and(
            |(status, body)| status == StatusCode::OK && body.contains(r#""forms":[],"#)
        ),
        "a read that narrows to nothing narrows to nothing"
    );
}

#[tokio::test]
async fn the_checks_answer_under_their_own_kind() {
    let seen = asked(world(running(), stack()), "/api/checks?only=vpn").await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::OK
            && body.starts_with(r#"{"api_version":1,"kind":"doctor","data":{"#)),
        "a diagnosis is answered in a diagnosis's envelope"
    );
}

#[tokio::test]
async fn a_whole_diagnosis_is_what_a_read_naming_no_group_asks_for() {
    let seen = asked(world(running(), stack()), "/api/checks").await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::OK
            && body.starts_with(r#"{"api_version":1,"kind":"doctor""#)),
        "naming no group runs every check there is"
    );
}

/// One check, asked for by the identifier its own finding carries.
#[tokio::test]
async fn a_single_check_is_asked_for_the_way_a_finding_names_it() {
    let seen = asked(
        world(running(), stack()),
        "/api/checks?only=environment.engine",
    )
    .await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::OK
            && body.contains(r#""check":"environment.engine""#)
            && !body.contains(r#""check":"environment.compose""#)),
        "a read narrowed to one check answers with that check"
    );
}

#[tokio::test]
async fn a_group_of_checks_that_is_not_one_is_not_run() {
    // A name lemonfiber does not know is a mistake to correct, not a request to
    // answer with everything — the judgement the command line makes too.
    assert_eq!(
        asked(world(running(), stack()), "/api/checks?only=nonsense").await,
        Some((
            StatusCode::BAD_REQUEST,
            "There is no group of checks and no check by that name.".to_owned()
        ))
    );
}

#[tokio::test]
async fn the_disk_is_read_through_the_checks_that_are_about_it() {
    let seen = asked(world(running(), stack()), "/api/storage").await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::OK
            && body.starts_with(r#"{"api_version":1,"kind":"doctor""#)
            && body.contains("storage")),
        "the disk's endpoint is the disk's group of checks"
    );
}

#[tokio::test]
async fn what_the_household_asked_for_is_answered_under_its_own_kind() {
    let seen = asked(world(running(), stack()), "/api/requests").await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::OK
            && body.starts_with(r#"{"api_version":1,"kind":"household","data":{"#)),
        "the household's requests are answered in the household's envelope"
    );
}

#[tokio::test]
async fn naming_a_member_narrows_what_the_household_read_reports() {
    let seen = asked(world(running(), stack()), "/api/requests?member=ada").await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::OK
            && body.starts_with(r#"{"api_version":1,"kind":"household""#)),
        "a named member is still the household's envelope"
    );
}

/// Naming none is the whole household, so an empty name cannot be read as naming
/// none. Left to reach the core it matched nobody and answered with a household
/// that has asked for nothing — the one reading this report is written to refuse.
#[tokio::test]
async fn a_member_given_and_left_empty_narrowed_to_nobody() {
    assert_eq!(
        asked(world(running(), stack()), "/api/requests?member=").await,
        Some((
            StatusCode::BAD_REQUEST,
            "Which member to narrow to must be named.".to_owned()
        ))
    );
}

#[tokio::test]
async fn what_the_services_said_arrives_as_one_envelope_a_line() {
    // A stream has no last element to close a document with, so the command line
    // emits an envelope a line and this answers with the same.
    let engine = Reporting::holding(&["sonarr"], Lifecycle::Running, Health::Healthy)
        .saying_at("sonarr", "2026-01-01T00:00:00Z", "started")
        .saying_at("sonarr", "2026-01-01T00:00:01Z", "importing");

    let seen = asked(world(engine, stack()), "/api/logs?service=sonarr&tail=10").await;
    assert_eq!(
        seen,
        Some((
            StatusCode::OK,
            concat!(
                r#"{"api_version":1,"kind":"log","data":{"service":"sonarr","stream":"stdout","#,
                r#""at":"2026-01-01T00:00:00Z","line":"started"}}"#,
                "\n",
                r#"{"api_version":1,"kind":"log","data":{"service":"sonarr","stream":"stdout","#,
                r#""at":"2026-01-01T00:00:01Z","line":"importing"}}"#,
                "\n",
            )
            .to_owned()
        ))
    );
}

#[tokio::test]
async fn a_service_with_nothing_to_say_answers_with_nothing() {
    // Not "no output": that sentence is for a person, and nobody is reading this.
    let engine = Reporting::holding(&["sonarr"], Lifecycle::Running, Health::Healthy);
    assert_eq!(
        asked(world(engine, stack()), "/api/logs").await,
        Some((StatusCode::OK, String::new()))
    );
}

#[tokio::test]
async fn a_form_narrows_a_log_read_the_way_it_narrows_the_command() {
    let engine = Reporting::holding(&["jellyfin"], Lifecycle::Running, Health::Healthy).saying_at(
        "jellyfin",
        "2026-01-01T00:00:00Z",
        "listening",
    );

    let seen = asked(world(engine, stack()), "/api/logs?form=library").await;
    assert!(
        seen.is_some_and(
            |(status, body)| status == StatusCode::OK && body.contains(r#""service":"jellyfin""#)
        ),
        "a form is the services it declares"
    );
}

#[tokio::test]
async fn a_line_count_that_is_not_a_number_is_refused() {
    assert_eq!(
        asked(world(running(), stack()), "/api/logs?tail=plenty").await,
        Some((
            StatusCode::BAD_REQUEST,
            "How many lines to begin with must be a number, and no more than 10000.".to_owned()
        ))
    );
}

#[tokio::test]
async fn the_versions_in_play_are_the_envelope_the_command_renders() {
    // The cheapest read there is: no arguments, and an answer the core already
    // renders for the command line.
    let expected = as_the_command_renders_it(&world(running(), stack()), Command::Version).await;

    assert!(expected.is_some(), "the command answered");
    assert_eq!(
        asked(world(running(), stack()), "/api/version").await,
        expected.map(|body| (StatusCode::OK, body))
    );
}

#[tokio::test]
async fn the_versions_in_play_are_carried_in_their_own_envelope() {
    // Written out rather than derived, so a second serialisation could not pass
    // this by agreeing with itself.
    let seen = asked(world(running(), stack()), "/api/version").await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::OK
            && body.starts_with(r#"{"api_version":1,"kind":"version","data":{"binary":"#)),
        "the versions in play, under the version kind"
    );
}

#[tokio::test]
async fn following_one_item_is_the_envelope_the_command_renders() {
    let expected = as_the_command_renders_it(
        &world(running(), stack()),
        Command::Trace {
            term: "the expanse".to_owned(),
            season: None,
        },
    )
    .await;

    assert!(expected.is_some(), "the command answered");
    assert_eq!(
        asked(world(running(), stack()), "/api/trace?term=the+expanse").await,
        expected.map(|body| (StatusCode::OK, body))
    );
}

#[tokio::test]
async fn the_term_a_trace_followed_is_the_term_that_was_asked_for() {
    // The whole request is its argument, so a read that dropped it would answer
    // about something else and look like it had answered.
    let seen = asked(world(running(), stack()), "/api/trace?term=the+expanse").await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::OK
            && body.starts_with(r#"{"api_version":1,"kind":"trace","data":{"#)
            && body.contains(r#""item":"the expanse""#)),
        "the item followed is the one named"
    );
}

#[tokio::test]
async fn a_season_narrows_a_trace_the_way_it_narrows_the_command() {
    let expected = as_the_command_renders_it(
        &world(running(), stack()),
        Command::Trace {
            term: "the expanse".to_owned(),
            season: Some(2),
        },
    )
    .await;

    assert!(expected.is_some(), "the command answered");
    assert_eq!(
        asked(
            world(running(), stack()),
            "/api/trace?term=the+expanse&season=2"
        )
        .await,
        expected.map(|body| (StatusCode::OK, body))
    );
}

#[tokio::test]
async fn a_trace_that_named_nothing_to_follow_is_refused() {
    // The command line requires the term too. A trace of everything is not a
    // smaller request than a trace of one thing; it is a different one.
    assert_eq!(
        asked(world(running(), stack()), "/api/trace").await,
        Some((
            StatusCode::BAD_REQUEST,
            "What to follow must be named.".to_owned()
        ))
    );
}

#[tokio::test]
async fn a_term_given_and_left_empty_named_nothing_to_follow() {
    assert_eq!(
        asked(world(running(), stack()), "/api/trace?term=").await,
        Some((
            StatusCode::BAD_REQUEST,
            "What to follow must be named.".to_owned()
        ))
    );
}

#[tokio::test]
async fn a_season_that_is_not_a_number_is_refused() {
    assert_eq!(
        asked(
            world(running(), stack()),
            "/api/trace?term=the+expanse&season=latest"
        )
        .await,
        Some((
            StatusCode::BAD_REQUEST,
            "Which season to narrow to must be a number.".to_owned()
        ))
    );
}

#[tokio::test]
async fn what_has_stopped_is_answered_under_its_own_kind() {
    // The landing point for the dashboard's own count of what is stuck, which
    // until this endpoint existed had nowhere on the web to go.
    let seen = asked(world(running(), stack()), "/api/stuck").await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::OK
            && body.starts_with(r#"{"api_version":1,"kind":"stuck","data":{"items":"#)),
        "the stuck items are answered in the stuck envelope"
    );
}

#[tokio::test]
async fn the_one_address_for_the_household_is_answered_under_its_own_kind() {
    // The question a browser has no other way to ask: which one link to send. The
    // answer names the request service and names the index over every service as
    // something that is not a way in, rather than leaving a browser to decide.
    let household = Reporting::holding(
        &["jellyfin", "seerr", "homepage"],
        Lifecycle::Running,
        Health::Healthy,
    );
    // The machine is scripted: what this one is called differs on every machine the
    // tests run on.
    let here = world(household, stack()).with_site(Renamed::called(Some("kitchen-nas")));
    let seen = asked(here, "/api/front-door").await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::OK
            && body.starts_with(r#"{"api_version":1,"kind":"front-door","data":{"#)
            && body.contains(r#""standing":"established""#)
            && body.contains(r#""service":"Seerr""#)
            && body.contains(r#""url":"http://kitchen-nas.local:5055","caution":null"#)
            && body.contains(r#""facing":"asking""#)
            && body.contains(r#""service":"Homepage","facing":"operators""#)),
        "the front door is answered in the front-door envelope"
    );
}

#[tokio::test]
async fn every_setting_is_the_envelope_the_command_renders() {
    let contents = kept();
    let expected =
        as_the_command_renders_it(&configured("shown-command", &contents), Command::ConfigShow)
            .await;

    assert!(expected.is_some(), "the command answered");
    assert_eq!(
        asked(configured("shown-endpoint", &contents), "/api/config").await,
        expected.map(|body| (StatusCode::OK, body))
    );
}

#[tokio::test]
async fn a_setting_whose_name_reads_as_a_credential_is_withheld() {
    // The withholding is the core's, so it is in force wherever the settings are
    // read from. This is the endpoint that would have published them.
    let seen = asked(configured("withheld", &kept()), "/api/config").await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::OK
            && !body.contains(&a_value())
            && body.contains(&format!(r#""value":"{REDACTED}","secret":true"#))
            && body.contains(r#""key":"LEMONFIBER_USENET","value":"on","secret":false"#)),
        "the credential is withheld and the setting beside it is not"
    );
}

#[tokio::test]
async fn naming_a_setting_reads_that_one_rather_than_all_of_them() {
    let seen = asked(
        configured("one-setting", &kept()),
        "/api/config?key=LEMONFIBER_USENET",
    )
    .await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::OK
            && body.contains(r#""key":"LEMONFIBER_USENET""#)
            && !body.contains("SONARR_API_KEY")),
        "a named setting is the setting reported on"
    );
}

/// The same rule for a setting, and the answer it used to give was quieter: an
/// empty name matched no setting and came back as a listing of none, which reads
/// as "there is no such setting" about a setting nobody named.
#[tokio::test]
async fn a_setting_given_and_left_empty_named_no_setting() {
    assert_eq!(
        asked(configured("no-setting", &kept()), "/api/config?key=").await,
        Some((
            StatusCode::BAD_REQUEST,
            "Which setting to read must be named.".to_owned()
        ))
    );
}

/// Naming one setting is not a way past the withholding.
///
/// The narrowing happens after the display path rather than instead of it, so a
/// credential asked for by name comes back withheld exactly as the listing withholds
/// it. Worth asserting rather than assuming: a read that filtered the file and then
/// displayed what it found would pass every other test on this page and hand over
/// the value.
#[tokio::test]
async fn naming_a_credential_reads_it_withheld_as_the_listing_withholds_it() {
    let seen = asked(
        configured("named-credential", &kept()),
        "/api/config?key=SONARR_API_KEY",
    )
    .await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::OK
            && !body.contains(&a_value())
            && body.contains(&format!(
                r#""key":"SONARR_API_KEY","value":"{REDACTED}","secret":true"#
            ))),
        "a credential named on its own is withheld the way it is in the listing"
    );
}

#[tokio::test]
async fn the_quality_in_force_is_the_envelope_the_command_renders() {
    let expected = as_the_command_renders_it(
        &world(running(), stack()),
        Command::Quality(QualityAction::Show),
    )
    .await;

    assert!(expected.is_some(), "the command answered");
    assert_eq!(
        asked(world(running(), stack()), "/api/quality").await,
        expected.map(|body| (StatusCode::OK, body))
    );
}

#[tokio::test]
async fn the_quality_in_force_is_carried_in_its_own_envelope() {
    let seen = asked(world(running(), stack()), "/api/quality").await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::OK
            && body.starts_with(r#"{"api_version":1,"kind":"quality","data":{"#)),
        "the choice in force is answered in the quality envelope"
    );
}

#[tokio::test]
async fn what_a_word_means_is_the_envelope_the_command_renders() {
    let expected = as_the_command_renders_it(
        &world(running(), stack()),
        Command::Explain {
            word: "indexer".to_owned(),
        },
    )
    .await;

    assert!(expected.is_some(), "the command answered");
    assert_eq!(
        asked(world(running(), stack()), "/api/explain?word=indexer").await,
        expected.map(|body| (StatusCode::OK, body))
    );
}

#[tokio::test]
async fn a_word_is_explained_with_no_stack_and_no_engine_to_read() {
    // The property this read has and no other on this surface does: the words are
    // compiled in, so a browser meeting one in a failure can ask what it means
    // while the thing that failed is still down.
    let seen = asked(
        world(Reporting::absent(), nowhere()),
        "/api/explain?word=hardlink",
    )
    .await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::OK
            && body.starts_with(r#"{"api_version":1,"kind":"word","data":{"word":"hardlink""#)
            && body.contains("Deleting one leaves the other working.")),
        "the entry whole, longer form and all, from a host with nothing running"
    );
}

#[tokio::test]
async fn a_word_a_script_asked_about_is_the_whole_entry() {
    // A caller has no way to ask a second time for the rest, so the longer form and
    // the other services' names — the parts it could not have guessed — arrive in
    // the one document. These are the bytes `lemonfiber explain <word> --json`
    // writes, because both go through the same rendering.
    let seen = asked(world(running(), stack()), "/api/explain?word=indexer").await;
    assert!(
        seen.is_some_and(|(_, body)| body.lines().count() == 1
            && body.contains("Prowlarr")
            && body.contains(r#""also_called":["search provider"]"#)),
        "one document, the longer form and the other names in it"
    );
}

#[tokio::test]
async fn naming_no_word_lists_every_word_there_is_to_ask_about() {
    // One endpoint over two commands, for the reason `/api/forms` is: a caller that
    // has never met this vocabulary cannot name a word out of it, and one that
    // carried its own copy of the table would be explaining words its own way.
    let expected = as_the_command_renders_it(&world(running(), stack()), Command::Glossary).await;

    assert!(expected.is_some(), "the command answered");
    assert_eq!(
        asked(world(running(), stack()), "/api/explain").await,
        expected.map(|body| (StatusCode::OK, body))
    );
}

#[tokio::test]
async fn the_words_this_product_explains_are_carried_in_their_own_envelope() {
    // Written out rather than derived, so a second serialisation could not pass
    // this by agreeing with itself.
    let seen = asked(world(running(), stack()), "/api/explain").await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::OK
            && body.starts_with(
                r#"{"api_version":1,"kind":"glossary","data":{"words":[{"word":"indexer""#
            )
            && body.contains(r#""word":"custom format""#)),
        "the whole table, under the glossary kind"
    );
}

#[tokio::test]
async fn a_word_this_product_does_not_explain_is_refused_rather_than_listed() {
    // A word with no entry is a mistake to correct, and answering it with the
    // catalogue would read as having been answered. The refusal names the words
    // there are, which is the same one the command line reports.
    let seen = asked(world(running(), stack()), "/api/explain?word=kubernetes").await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::NOT_FOUND
            && body.starts_with(r#"{"api_version":1,"kind":"error","data":{"code":"WORD-1""#)
            && body.contains("indexer")),
        "a word with no entry is refused, with what there is instead"
    );
}

#[tokio::test]
async fn a_word_with_no_entry_is_not_said_the_way_a_stack_that_cannot_answer_is() {
    // What this endpoint could not say before: a glossary reading `WORD-1` and an
    // engine that is not there were one status, and a caller holding both had to
    // write a sentence true of either.
    let missing = asked(world(running(), stack()), "/api/explain?word=kubernetes").await;
    let unanswered = asked(world(Reporting::absent(), stack()), "/api/logs").await;

    assert_eq!(
        missing.map(|(status, _)| status),
        Some(StatusCode::NOT_FOUND)
    );
    assert_eq!(
        unanswered.map(|(status, _)| status),
        Some(StatusCode::INTERNAL_SERVER_ERROR)
    );
}

#[tokio::test]
async fn a_word_given_and_left_empty_named_a_word_rather_than_none() {
    // Naming the parameter and leaving it blank is a word this product does not
    // explain, not a request for the list — the same answer `lemonfiber explain ""`
    // gets, and the reason the empty one is not read as having named nothing.
    let seen = asked(world(running(), stack()), "/api/explain?word=").await;
    assert!(
        seen.is_some_and(
            |(status, body)| status == StatusCode::NOT_FOUND && body.contains(r#""code":"WORD-1""#)
        ),
        "an empty word is refused rather than listed"
    );
}

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
