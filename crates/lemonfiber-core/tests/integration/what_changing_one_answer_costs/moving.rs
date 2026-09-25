//! Moving the data location, and what the library survives.

use super::{
    changing, common, env_at, held, over, reaching, refusal, remembering, stance, stood_up, AGREED,
    DATA_ROOT, TORRENT, UNSAID,
};
use lemonfiber_core::config::{Protocols, Settings};
use lemonfiber_core::reconfigure::Stance;
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::downloads::SAB_KEY_INI;
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::support::{seeding_routes, SeedFs};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[tokio::test]
async fn a_move_the_library_survives_says_where_each_folder_lands_and_is_made_once_agreed() {
    // The \*arrs hold `/data/media/{tv,movies,music}`, and the new location holds the
    // directories behind all three. The paths do not change — they are inside the
    // container — but what they resolve to does, and that is what has to be said.
    let from = PathBuf::from("/srv/old");
    let env = env_at("carried", &from);
    // A record kept for some other setting: lemonfiber has written here before and
    // still has nothing recorded about the one this change names.
    remembering(&env, TORRENT, "on");
    let ctx = reaching(env.clone(), &from, Protocols::none(), Vec::new());

    let staged = changing(&ctx, DATA_ROOT, "/srv/new", UNSAID).await;
    let landing: Vec<String> = staged
        .as_ref()
        .map(|review| review.findings.library.clone())
        .unwrap_or_default()
        .into_iter()
        .filter(|path| path.carried)
        .filter_map(|path| path.host)
        .collect();
    assert!(
        landing.contains(&"/srv/new/media/tv".to_owned()),
        "{landing:?}"
    );
    // Where each folder lands is said while the change is still staged, which is the
    // only moment it is worth having.
    assert_eq!(stance(staged.as_ref()), Some(Stance::Pending));
    assert_eq!(held(&env, DATA_ROOT).as_deref(), Some("/srv/old"));

    let applied = changing(&ctx, DATA_ROOT, "/srv/new", AGREED).await;
    assert_eq!(stance(applied.as_ref()), Some(Stance::Applied));
    assert_eq!(held(&env, DATA_ROOT).as_deref(), Some("/srv/new"));
}

#[tokio::test]
async fn a_move_that_would_leave_the_library_pointing_at_nothing_cannot_be_forced() {
    // The new location has no `media/movies` behind it. Applying anyway would leave
    // Radarr importing into a void and the operator finding out weeks later, so there
    // is no version of this a confirmation gets past.
    let from = PathBuf::from("/srv/old");
    let env = env_at("refused", &from);
    let ctx = reaching(
        env.clone(),
        &from,
        Protocols::none(),
        vec!["/srv/new/media/movies"],
    );

    let review = changing(&ctx, DATA_ROOT, "/srv/new", AGREED).await;

    let refusal = refusal(review.as_ref());
    assert!(
        refusal
            .as_deref()
            .is_some_and(|said| said.contains("/data/media/movies")),
        "{refusal:?}"
    );
    assert!(
        refusal
            .as_deref()
            .is_some_and(|said| !said.contains("--confirm")),
        "a break is not the operator's to force: {refusal:?}"
    );
    assert_eq!(stance(review.as_ref()), Some(Stance::Blocked));
    assert_eq!(
        held(&env, DATA_ROOT).as_deref(),
        Some("/srv/old"),
        "a refused change leaves the file exactly as it was"
    );
}

#[tokio::test]
async fn a_move_is_refused_where_the_services_have_not_written_their_keys_yet() {
    // A service still starting has written no key, so there is nothing to open it
    // with. That is not the same as a service that says it holds no library, and only
    // one of the two makes a move safe.
    let from = PathBuf::from("/srv/old");
    let env = env_at("keyless", &from);
    let ctx = stood_up(
        Settings {
            env_file: Some(env.clone()),
            data_root: Some(from),
            ..Settings::default()
        },
        Source::External(common::stack::project()),
        Fake::by_path(seeding_routes()),
        Vec::new(),
    )
    .with_filesystem(Arc::new(SeedFs::keyed(None, Some(SAB_KEY_INI))));

    let review = changing(&ctx, DATA_ROOT, "/srv/new", AGREED).await;

    assert_eq!(stance(review.as_ref()), Some(Stance::Blocked));
    assert_eq!(held(&env, DATA_ROOT).as_deref(), Some("/srv/old"));
}

#[tokio::test]
async fn a_move_no_service_will_answer_about_is_refused_where_there_is_a_library_to_lose() {
    // Not one \*arr answers, so nothing will say where the library files. Silence is
    // not an empty library, and reading it as one is how a move comes to be applied
    // over a library it breaks.
    let from = PathBuf::from("/srv/old");
    let env = env_at("silent", &from);
    let ctx = over(
        env.clone(),
        &from,
        Protocols::none(),
        Fake::by_path(vec![("", Answer::reply(500, "no"))]),
        Vec::new(),
    );

    let review = changing(&ctx, DATA_ROOT, "/srv/new", AGREED).await;

    let refusal = refusal(review.as_ref());
    assert!(
        refusal
            .as_deref()
            .is_some_and(|said| said.contains("/srv/old/media")),
        "{refusal:?}"
    );
    assert_eq!(stance(review.as_ref()), Some(Stance::Blocked));
    assert_eq!(held(&env, DATA_ROOT).as_deref(), Some("/srv/old"));
}

#[tokio::test]
async fn a_folder_no_move_can_repoint_is_refused_by_name() {
    // A folder outside `/data` — an adopted stack's own, or one set by hand. No write
    // to the data location reaches it, so the move is refused rather than silently
    // leaving one \\*arr filing where it filed before.
    let from = PathBuf::from("/srv/old");
    let env = env_at("outside", &from);
    let mut routes = vec![(
        "/rootfolder",
        Answer::reply(200, r#"[{"id":1,"path":"/mnt/old/tv"}]"#),
    )];
    routes.extend(seeding_routes());
    let ctx = over(
        env.clone(),
        &from,
        Protocols::none(),
        Fake::by_path(routes),
        Vec::new(),
    );

    let review = changing(&ctx, DATA_ROOT, "/srv/new", AGREED).await;

    let refusal = refusal(review.as_ref());
    assert!(
        refusal
            .as_deref()
            .is_some_and(|said| said.contains("/mnt/old/tv")),
        "{refusal:?}"
    );
    assert_eq!(held(&env, DATA_ROOT).as_deref(), Some("/srv/old"));
}

#[tokio::test]
async fn one_arr_that_will_not_list_is_passed_over_rather_than_read_as_a_break() {
    // A service that will not list has not said it holds nothing, and a later run
    // completes it. The \\*arrs that did answer are enough to weigh the move.
    let from = PathBuf::from("/srv/old");
    let env = env_at("half-answering", &from);
    let mut routes = vec![("127.0.0.1:7878", Answer::reply(500, "no"))];
    routes.extend(seeding_routes());
    let ctx = over(
        env.clone(),
        &from,
        Protocols::none(),
        Fake::by_path(routes),
        Vec::new(),
    );

    let review = changing(&ctx, DATA_ROOT, "/srv/new", AGREED).await;

    let named: Vec<String> = review
        .as_ref()
        .map(|review| review.findings.library.clone())
        .unwrap_or_default()
        .into_iter()
        .map(|path| path.service)
        .collect();
    assert!(!named.is_empty(), "the services that answered were read");
    assert_eq!(stance(review.as_ref()), Some(Stance::Applied));
    assert_eq!(held(&env, DATA_ROOT).as_deref(), Some("/srv/new"));
}

#[tokio::test]
async fn a_move_with_no_library_behind_the_old_location_is_simply_made() {
    // A stack that never ran has nothing to invalidate, and refusing it would block
    // the commonest reason anybody moves the data location at all.
    let from = PathBuf::from("/srv/old");
    let env = env_at("empty", &from);
    // A record that matches the file: lemonfiber wrote this and nobody has touched it
    // since, so there is no edit under the change however hard it is looked for.
    remembering(&env, DATA_ROOT, "/srv/old");
    let ctx = over(
        env.clone(),
        &from,
        Protocols::none(),
        Fake::by_path(vec![("", Answer::reply(500, "no"))]),
        vec!["/srv/old/media"],
    );

    let review = changing(&ctx, DATA_ROOT, "/srv/new", AGREED).await;

    assert_eq!(stance(review.as_ref()), Some(Stance::Applied));
    assert_eq!(held(&env, DATA_ROOT).as_deref(), Some("/srv/new"));
}

#[tokio::test]
async fn setting_the_data_location_to_where_it_already_is_asks_no_service_anything() {
    // Not a move. Reading the \*arrs about it would be reaching the network to work
    // out that nothing changes.
    let from = PathBuf::from("/srv/media");
    let env = env_at("unmoved", &from);
    let fake = Fake::by_path(seeding_routes());
    let ctx = over(env, &from, Protocols::none(), Arc::clone(&fake), Vec::new());

    let review = changing(&ctx, DATA_ROOT, "/srv/media", AGREED).await;

    assert_eq!(stance(review.as_ref()), Some(Stance::Unchanged));
    assert!(fake.requests().is_empty(), "{:?}", fake.requests());
}

#[tokio::test]
async fn choosing_a_data_location_where_none_was_chosen_is_setups_answer_not_a_move() {
    // Nothing has been chosen, so there is no library at a previous location this
    // could invalidate — and refusing here would block the first location anybody
    // records.
    let dir = lemonfiber_fixtures::scratch::Scratch::named("change-first");
    let _ = std::fs::remove_dir_all(&dir);
    let env = dir.join(".env");
    let ctx = stood_up(
        Settings {
            env_file: Some(env.clone()),
            ..Settings::default()
        },
        Source::External(common::stack::project()),
        Fake::by_path(seeding_routes()),
        Vec::new(),
    );

    let review = changing(&ctx, DATA_ROOT, "/srv/first", AGREED).await;

    assert_eq!(stance(review.as_ref()), Some(Stance::Applied));
    assert_eq!(held(&env, DATA_ROOT).as_deref(), Some("/srv/first"));
}

#[tokio::test]
async fn a_stack_that_cannot_be_read_names_no_service_and_still_refuses_a_blind_move() {
    // Without a manifest there is nothing to resolve the \*arrs from, so nothing can
    // say where the library files — and a stack this build cannot read is exactly the
    // one where a silent move would go worst.
    let from = PathBuf::from("/srv/old");
    let env = env_at("no-stack", &from);
    let ctx = stood_up(
        Settings {
            env_file: Some(env),
            data_root: Some(from),
            protocols: Protocols::both(),
            ..Settings::default()
        },
        Source::External(Path::new("/lemonfiber-not-a-real-stack")),
        Fake::by_path(seeding_routes()),
        Vec::new(),
    );

    let moved = changing(&ctx, DATA_ROOT, "/srv/new", AGREED).await;
    assert_eq!(stance(moved.as_ref()), Some(Stance::Blocked));

    // The same stack, asked to drop a protocol: nothing can be named as stopping,
    // which is silence about the services rather than a claim that none stop.
    let dropped = changing(&ctx, TORRENT, "off", AGREED).await;
    let findings = dropped.map(|review| review.findings);
    assert!(findings.is_some_and(|found| found.stops.is_empty() && !found.keeps.is_empty()));
}
