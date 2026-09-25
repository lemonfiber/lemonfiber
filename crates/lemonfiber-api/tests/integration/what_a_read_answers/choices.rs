//! Settings, quality and the glossary, read.

use super::reading::*;

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
