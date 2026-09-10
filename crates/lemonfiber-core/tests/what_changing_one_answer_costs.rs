//! What a proposed change comes to on the machine it is made on.
//!
//! The diff beside this — what the setting holds and what it would hold — is settled
//! without asking anybody. Everything here needs somebody outside lemonfiber to have
//! been asked: the \*arrs for where they actually file, the download clients for what
//! is still coming down. So both sides are faked — a filesystem handing back each
//! \*arr's key, and a transport answering as the services would — and the command is
//! driven the way a surface drives it.
//!
//! From here rather than a `#[cfg(test)]` module, as the wiring and credentials tests
//! are: the app layer is compiled twice, and a branch driven only from the in-crate
//! tests is counted as never run in the copy these binaries link.

mod common;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use lemonfiber_core::app::{dispatch, Command, Ctx, Outcome, Setting, Waiting};
use lemonfiber_core::config::{Protocols, Settings};
use lemonfiber_core::platform::Environment;
use lemonfiber_core::ports::http::Http;
use lemonfiber_core::reconfigure::{Review, Stance};
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::downloads::{
    downloads, QBIT_FINISHED, QBIT_TORRENTS, SAB_EMPTY, SAB_KEY_INI, SAB_QUEUE,
};
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::ports::Stopped;
use lemonfiber_fixtures::support::{seeding_routes, spoke, Reporting, Scripted, SeedFs};
use lemonfiber_ports::docker::{Health, Lifecycle};

/// A Servarr config carrying a key, as one reads from disk.
const CONFIG: &str = "<Config><ApiKey>a1b2c3d4e5</ApiKey></Config>";

/// The settings these name, spelled once.
const DATA_ROOT: &str = lemonfiber_core::config::DATA_ROOT_KEY;
const TORRENT: &str = lemonfiber_core::config::TORRENT_KEY;
const USENET: &str = lemonfiber_core::config::USENET_KEY;

/// What the operator said about a change beyond what the change is.
#[derive(Clone, Copy)]
struct Said {
    /// Whether they agreed to what it costs.
    confirmed: bool,
    /// Whether they asked for what is still coming down to finish first.
    waiting: Waiting,
}

/// Nothing said: the plain run, which stages a consequential change.
const UNSAID: Said = Said {
    confirmed: false,
    waiting: Waiting::Never,
};

/// The same, having agreed to what it costs.
const AGREED: Said = Said {
    confirmed: true,
    waiting: Waiting::Never,
};

/// The same, having asked to wait for what is still coming down.
const WAITING: Said = Said {
    confirmed: false,
    waiting: Waiting::ForTheDownloads,
};

/// A scratch environment file, holding the data location a move starts from and the
/// download client's recorded password — without which nothing can ask qBittorrent
/// what is still coming down.
fn env_at(name: &str, from: &Path) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("lemonfiber-change-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let env = dir.join(".env");
    let _ = lemonfiber_core::config::store::set(&env, DATA_ROOT, &from.display().to_string());
    let _ = lemonfiber_core::config::store::set(
        &env,
        lemonfiber_core::config::QBITTORRENT_PASSWORD_KEY,
        &["minted", "-earlier"].concat(),
    );
    env
}

/// What lemonfiber last wrote, put where a change reads it back — so the change under
/// test is not the first this machine has seen and has a record to judge against.
fn remembering(env: &Path, key: &str, value: &str) {
    let record = format!(
        r#"{{"services":{{"lemonfiber:settings":{{"{key}":{{"value":"{value}","at":"1"}}}}}}}}"#
    );
    let _ = std::fs::write(env.with_file_name("baseline.json"), record);
}

/// A context over the stack this repository carries, reaching \*arrs that answer.
///
/// `missing` names path fragments the filesystem will not resolve, which is how a host
/// directory that is not there is driven: the \*arrs hold `/data/media/<type>` whatever
/// happens, and what decides a move is whether the directory behind each one exists at
/// the new location.
fn reaching(env: PathBuf, from: &Path, protocols: Protocols, missing: Vec<&'static str>) -> Ctx {
    over(
        env,
        from,
        protocols,
        Fake::by_path(seeding_routes()),
        missing,
    )
}

/// The same, over a transport a test supplies.
fn over(
    env: PathBuf,
    from: &Path,
    protocols: Protocols,
    http: Arc<Fake>,
    missing: Vec<&'static str>,
) -> Ctx {
    stood_up(
        Settings {
            env_file: Some(env),
            data_root: Some(from.to_path_buf()),
            protocols,
            ..Settings::default()
        },
        Source::External(common::stack::project()),
        http,
        missing,
    )
}

/// A context over whatever settings, stack and transport a test hands it.
fn stood_up(settings: Settings, stack: Source, http: Arc<Fake>, missing: Vec<&'static str>) -> Ctx {
    Ctx::new(
        Arc::new(Scripted(Ok(spoke("")))),
        Arc::new(Reporting::holding(
            &["sonarr"],
            Lifecycle::Running,
            Health::Healthy,
        )),
        Stopped::today(),
        lemonfiber_ports::seams::Seams {
            filesystem: Arc::new(SeedFs::keyed(Some(CONFIG), Some(SAB_KEY_INI)).missing(missing)),
            ..lemonfiber_adapters::live()
        },
        stack,
        settings,
        Environment::MacOs,
    )
    .with_http(http as Arc<dyn Http>)
}

/// The proposal, or nothing where the answer was not a settings one.
async fn changing(ctx: &Ctx, key: &str, value: &str, said: Said) -> Option<Review> {
    match dispatch(
        Command::ConfigSet(
            Setting::to(key, value)
                .agreed(said.confirmed)
                .waiting(said.waiting),
        ),
        ctx,
    )
    .await
    {
        Ok(Outcome::Config(report)) => report.review,
        _ => None,
    }
}

/// Where the proposal stands.
fn stance(review: Option<&Review>) -> Option<Stance> {
    review.map(|review| review.stance)
}

/// Why nothing was written, where nothing was.
fn refusal(review: Option<&Review>) -> Option<String> {
    review.and_then(|review| review.refusal.clone())
}

/// What the environment file holds for a setting now.
fn held(env: &Path, key: &str) -> Option<String> {
    lemonfiber_core::config::store::read(env)
        .ok()
        .and_then(|file| file.get(key).map(str::to_owned))
}

// ── Moving the data location ─────────────────────────────────────────────────

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
    let dir = std::env::temp_dir().join(format!("lemonfiber-change-{}-first", std::process::id()));
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

// ── Adding and dropping a way of downloading ─────────────────────────────────

#[tokio::test]
async fn dropping_torrents_while_one_is_coming_down_reports_it_and_offers_to_wait() {
    // Ninety-four per cent of the way down, and no tracker resumes that everywhere.
    // The operator almost never knows — they are thinking about the setting they
    // typed, not about what is inside the client it takes away.
    let from = PathBuf::from("/srv/media");
    let env = env_at("in-flight", &from);
    let ctx = over(
        env.clone(),
        &from,
        Protocols::both(),
        downloads(QBIT_TORRENTS, SAB_EMPTY),
        Vec::new(),
    );

    let review = changing(&ctx, TORRENT, "off", UNSAID).await;

    let named: Vec<String> = review
        .as_ref()
        .map(|review| {
            review
                .findings
                .active
                .iter()
                .map(|download| download.name.clone())
                .collect()
        })
        .unwrap_or_default();
    assert!(named.contains(&"Ubuntu.iso".to_owned()), "{named:?}");
    assert_eq!(stance(review.as_ref()), Some(Stance::Blocked));
    let refusal = refusal(review.as_ref()).unwrap_or_default();
    assert!(refusal.contains("--wait"), "{refusal}");
    assert!(refusal.contains("--confirm"), "{refusal}");
    // What it keeps is stated even while it is refusing, because that is the half the
    // operator is actually weighing.
    let kept = review
        .map(|review| review.findings.keeps.join(" | "))
        .unwrap_or_default();
    assert!(kept.contains("/srv/media/downloads"), "{kept}");
    assert_eq!(held(&env, TORRENT), None, "a refused change writes nothing");
}

#[tokio::test]
async fn dropping_usenet_names_what_is_coming_down_over_usenet_and_not_the_torrents() {
    // Narrowed to the protocol being dropped: a torrent is in no danger from switching
    // Usenet off, and naming it would be reporting work nothing is about to interrupt.
    let from = PathBuf::from("/srv/media");
    let env = env_at("usenet-in-flight", &from);
    let ctx = over(
        env.clone(),
        &from,
        Protocols::both(),
        downloads(QBIT_TORRENTS, SAB_QUEUE),
        Vec::new(),
    );

    let review = changing(&ctx, USENET, "off", UNSAID).await;

    let named: Vec<(String, String)> = review
        .as_ref()
        .map(|review| {
            review
                .findings
                .active
                .iter()
                .map(|download| (download.protocol.clone(), download.name.clone()))
                .collect()
        })
        .unwrap_or_default();
    assert_eq!(
        named,
        vec![("usenet".to_owned(), "Linux.nzb".to_owned())],
        "only what the dropped protocol is carrying"
    );
    assert_eq!(stance(review.as_ref()), Some(Stance::Blocked));
    assert_eq!(held(&env, USENET), None);
}

#[tokio::test]
async fn the_offer_to_wait_taken_up_lets_the_downloads_finish_and_then_makes_the_change() {
    // The clients hold nothing, so the wait is over as soon as it begins — which is
    // the case worth driving: what is proven here is that the change goes through the
    // wait rather than around it, and a fixture that never drained would only prove a
    // test can hang.
    let from = PathBuf::from("/srv/media");
    let env = env_at("waited", &from);
    let ctx = over(
        env.clone(),
        &from,
        Protocols::both(),
        downloads(QBIT_FINISHED, SAB_EMPTY),
        Vec::new(),
    );

    let review = changing(
        &ctx,
        TORRENT,
        "off",
        Said {
            confirmed: true,
            ..WAITING
        },
    )
    .await;

    assert_eq!(stance(review.as_ref()), Some(Stance::Applied));
    assert!(review.is_some_and(|review| review.findings.active.is_empty()));
    assert_eq!(held(&env, TORRENT).as_deref(), Some("off"));
}

#[tokio::test]
async fn asking_a_change_that_takes_nothing_away_to_wait_waits_for_nothing() {
    // A wait is the offer a *reduction* makes. Asked of a change that opens something,
    // it must not sit down in front of every download on the machine.
    let from = PathBuf::from("/srv/media");
    let env = env_at("not-a-reduction", &from);
    let fake = downloads(QBIT_TORRENTS, SAB_EMPTY);
    let ctx = over(env, &from, Protocols::none(), Arc::clone(&fake), Vec::new());

    let review = changing(&ctx, USENET, "on", WAITING).await;

    assert_eq!(stance(review.as_ref()), Some(Stance::Pending));
    assert!(
        fake.requests().is_empty(),
        "a change that takes nothing away asked the clients anyway: {:?}",
        fake.requests()
    );
}

#[tokio::test]
async fn adding_usenet_asks_for_its_own_login_and_never_for_a_tunnel() {
    let from = PathBuf::from("/srv/media");
    let env = env_at("adding", &from);
    let ctx = reaching(env.clone(), &from, Protocols::none(), Vec::new());

    let review = changing(&ctx, USENET, "on", AGREED).await;

    let opened: Vec<String> = review
        .as_ref()
        .map(|review| {
            review
                .findings
                .opens
                .iter()
                .filter_map(|open| open.setting.clone())
                .collect()
        })
        .unwrap_or_default();
    assert!(opened.contains(&"USENET_HOST".to_owned()), "{opened:?}");
    assert!(!opened.contains(&"VPN_PROVIDER".to_owned()), "{opened:?}");
    assert_eq!(stance(review.as_ref()), Some(Stance::Applied));
    assert_eq!(held(&env, USENET).as_deref(), Some("on"));
}

#[tokio::test]
async fn switching_on_what_is_already_on_opens_nothing_and_keeps_nothing() {
    // A change that leaves the protocols where they are says nothing about them: a
    // report of what a protocol opens, attached to a change that opened nothing, is a
    // warning nobody caused.
    let from = PathBuf::from("/srv/media");
    let env = env_at("already-on", &from);
    let ctx = reaching(
        env,
        &from,
        Protocols {
            usenet: true,
            torrent: false,
        },
        Vec::new(),
    );

    let review = changing(&ctx, USENET, "on", AGREED).await;

    assert!(review.is_some_and(|review| !review.findings.any()));
}

// ── What the change cannot get past ──────────────────────────────────────────

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

// ── An edit made outside lemonfiber ──────────────────────────────────────────

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
