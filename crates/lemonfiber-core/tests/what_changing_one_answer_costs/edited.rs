//! A setting edited by hand, shown before anything is written.

use crate::{
    changing, common, env_at, held, over, reaching, refusal, remembering, stance, stood_up, AGREED,
    DATA_ROOT, UNSAID, USENET,
};
use lemonfiber_core::app::{dispatch, Command, Setting};
use lemonfiber_core::config::{Protocols, Settings};
use lemonfiber_core::reconfigure::Stance;
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::support::seeding_routes;
use std::path::PathBuf;

#[cfg(unix)]
#[tokio::test]
async fn a_change_that_cannot_reach_the_file_reports_that_rather_than_a_proposal() {
    // Everything in front of the write cleared, and the write itself failed. What
    // comes back is the file store's own words about a directory it cannot write in,
    // not a proposal that quietly did nothing.
    use std::os::unix::fs::PermissionsExt as _;

    let dir = std::env::temp_dir().join(format!("lemonfiber-change-{}-ro", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o500));
    let from = PathBuf::from("/srv/media");
    let ctx = stood_up(
        Settings {
            env_file: Some(dir.join(".env")),
            data_root: Some(from),
            ..Settings::default()
        },
        Source::External(common::stack::project()),
        Fake::by_path(seeding_routes()),
        Vec::new(),
    );

    let refused = dispatch(
        Command::ConfigSet(Setting::to("LEMONFIBER_EXPLANATIONS", "off")),
        &ctx,
    )
    .await
    .err()
    .map(|problem| problem.code);
    assert_eq!(
        refused,
        Some(lemonfiber_core::config::store::CONFIG_NOT_WRITTEN)
    );

    let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn a_record_that_cannot_be_read_leaves_the_change_free_to_be_made() {
    // A record that is there but unreadable cannot tell an edit from lemonfiber's own
    // value, so nothing is judged against it and nothing is written over it — the
    // change goes ahead and the record is left for the operator to re-form.
    let from = PathBuf::from("/srv/media");
    let env = env_at("record-lost", &from);
    let baseline = env.with_file_name("baseline.json");
    let _ = std::fs::write(&baseline, "not json at all");
    let ctx = reaching(env.clone(), &from, Protocols::none(), Vec::new());

    let review = changing(&ctx, USENET, "on", AGREED).await;

    assert_eq!(stance(review.as_ref()), Some(Stance::Applied));
    assert!(review.is_some_and(|review| review.findings.edited.is_none()));
    assert_eq!(held(&env, USENET).as_deref(), Some("on"));
    assert_eq!(
        std::fs::read_to_string(&baseline).unwrap_or_default(),
        "not json at all",
        "left exactly as it was"
    );
}

#[tokio::test]
async fn a_setting_edited_by_hand_is_shown_from_both_sides_before_anything_is_written() {
    // The record says lemonfiber wrote `/srv/old`; the file says somewhere else. The
    // difference is the operator's, and a change that took it silently is exactly the
    // failure that teaches people not to use the tool for their own configuration.
    let from = PathBuf::from("/mnt/theirs");
    let env = env_at("edited", &from);
    remembering(&env, DATA_ROOT, "/srv/old");
    let ctx = over(
        env.clone(),
        &from,
        Protocols::none(),
        Fake::by_path(vec![("", Answer::reply(500, "no"))]),
        vec!["/mnt/theirs/media"],
    );

    let review = changing(&ctx, DATA_ROOT, "/srv/new", UNSAID).await;

    let both = review
        .as_ref()
        .and_then(|review| review.findings.edited.clone())
        .map(|edit| (edit.wrote, edit.found));
    assert_eq!(
        both,
        Some(("/srv/old".to_owned(), "/mnt/theirs".to_owned())),
        "both sides are shown so the operator chooses between them"
    );
    assert_eq!(stance(review.as_ref()), Some(Stance::Blocked));
    assert!(refusal(review.as_ref()).is_some_and(|said| said.contains("--confirm")));
    assert_eq!(held(&env, DATA_ROOT).as_deref(), Some("/mnt/theirs"));

    // Confirmed, it goes ahead — the operator has been shown both sides and chosen,
    // which is the whole of what the refusal was for.
    let confirmed = changing(&ctx, DATA_ROOT, "/srv/new", AGREED).await;
    assert_eq!(stance(confirmed.as_ref()), Some(Stance::Applied));
    assert_eq!(held(&env, DATA_ROOT).as_deref(), Some("/srv/new"));
}

#[tokio::test]
async fn a_credential_edited_by_hand_is_reported_without_being_printed() {
    // The report is one a script can log. Saying *that* the provider password was
    // changed underneath is the whole of what the operator needs; printing either
    // value would make this the one place the password leaves the file.
    let from = PathBuf::from("/srv/media");
    let env = env_at("secret", &from);
    let _ = lemonfiber_core::config::store::set(
        &env,
        lemonfiber_core::config::PROVIDER_PASS_KEY,
        "the-one-they-set",
    );
    remembering(&env, "USENET_PASS", "the-one-we-wrote");
    let ctx = reaching(env, &from, Protocols::none(), Vec::new());

    let review = changing(
        &ctx,
        lemonfiber_core::config::PROVIDER_PASS_KEY,
        "another",
        UNSAID,
    )
    .await;

    let edit = review
        .as_ref()
        .and_then(|review| review.findings.edited.clone());
    assert_eq!(
        edit.map(|edit| (edit.wrote, edit.found, edit.secret)),
        Some((
            lemonfiber_core::config::store::REDACTED.to_owned(),
            lemonfiber_core::config::store::REDACTED.to_owned(),
            true
        ))
    );
    assert_eq!(stance(review.as_ref()), Some(Stance::Blocked));
}

#[tokio::test]
async fn a_line_taken_out_of_the_file_by_hand_is_an_edit_too() {
    // Somebody deleted the line. That is a change made outside lemonfiber exactly as
    // much as an altered value is, and the report has to say what it found rather than
    // an empty string that reads like a value.
    let from = PathBuf::from("/srv/media");
    let env = env_at("removed", &from);
    remembering(&env, USENET, "on");
    let ctx = reaching(env, &from, Protocols::none(), Vec::new());

    let review = changing(&ctx, USENET, "off", UNSAID).await;

    let found = review.as_ref().and_then(|review| {
        review
            .findings
            .edited
            .as_ref()
            .map(|edit| edit.found.clone())
    });
    assert_eq!(found.as_deref(), Some("nothing — the line is gone"));
}
