use std::sync::Arc;

use super::{configuration, reading, Setting};
use crate::app::{Ctx, Outcome, Waiting};
use crate::config::{
    store, EXPLANATIONS_KEY, FRONT_DOOR_KEY, INDEXER_APIKEY_KEY, INDEXER_URL_KEY,
    PROVIDER_PORT_KEY, PROVIDER_TLS_KEY, VPN_PORT_FORWARDING_KEY,
};
use crate::error::Diagnose;
use crate::reconfigure::{Review, Stance};
use crate::test_support::a_context;
use lemonfiber_fixtures::http::{Answer, Fake};

/// A search a Torznab indexer answers with, which proves a key.
const ANSWERED: &str = "<rss><channel><item/></channel></rss>";

/// A scratch environment file holding the given settings.
fn env_at(name: &str, contents: &str) -> std::path::PathBuf {
    let dir = lemonfiber_fixtures::scratch::Scratch::named(&format!("configuring-{name}")).kept();
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.join(".env");
    let _ = crate::config::store::write(&path, contents);
    path
}

/// A context over that file, for a stack that torrents.
fn ctx(env_file: std::path::PathBuf) -> Ctx {
    a_context()
        .runner(Arc::new(crate::test_support::Scripted(Ok(
            crate::test_support::spoke(""),
        ))))
        .settings(crate::config::Settings {
            env_file: Some(env_file),
            protocols: crate::config::Protocols::both(),
            ..crate::config::Settings::default()
        })
        .build()
}

/// The same, reaching every service through a transport that answers `answer`.
///
/// Replacing the transport replaces the validator with one that proves credentials
/// over it, so what an indexer says about a replacement key is the test's to say.
fn reaching(env_file: std::path::PathBuf, answer: Answer) -> Ctx {
    ctx(env_file).with_http(Fake::always(answer))
}

/// A file holding a complete Usenet login over TLS.
const A_LOGIN: &str = "USENET_HOST=news.example.net\nUSENET_PORT=563\n\
                       USENET_USER=someone\nUSENET_PASS=old-pass\nUSENET_TLS=on\n";

/// What a change said it cost, where it said anything.
fn consequence(outcome: &Result<Outcome, Box<crate::error::Problem>>) -> Option<String> {
    match outcome {
        Ok(Outcome::Config(report)) => report.consequence.clone(),
        Ok(other) => Some(format!("{other:?} is not a configuration answer")),
        Err(_) => None,
    }
}

/// The review a change came back with, where it came back with one.
fn reviewed(outcome: &Result<Outcome, Box<crate::error::Problem>>) -> Option<Review> {
    match outcome {
        Ok(Outcome::Config(report)) => report.review.clone(),
        _ => None,
    }
}

/// Where a change stands, where it proposed one.
fn stance(outcome: &Result<Outcome, Box<crate::error::Problem>>) -> Option<Stance> {
    reviewed(outcome).map(|review| review.stance)
}

/// What a setting holds on disk now.
fn on_disk(path: &std::path::Path, key: &str) -> Option<String> {
    store::read(path)
        .ok()
        .and_then(|file| file.get(key).map(str::to_owned))
}

/// A setting the operator declared unmanaged is turned away, in their own words,
/// and the file is left exactly as it was.
#[tokio::test]
async fn a_setting_declared_unmanaged_is_refused_with_the_reason_they_gave() {
    let path = env_at("unmanaged", "PUID=1000\n");
    let context = a_context()
        .settings(crate::config::Settings {
            env_file: Some(path.clone()),
            unmanaged: vec![(
                "PUID".to_owned(),
                "I set the ids here by hand to match my NAS".to_owned(),
            )],
            ..crate::config::Settings::default()
        })
        .build();

    let outcome = configuration(&context, Setting::to("PUID", "1001")).await;

    assert_eq!(stance(&outcome), Some(Stance::Blocked));
    let why = reviewed(&outcome).and_then(|review| review.refusal);
    assert!(
        why.is_some_and(|why| why.contains("match my NAS")),
        "the refusal does not carry the reason they gave"
    );
    assert_eq!(
        on_disk(&path, "PUID"),
        Some("1000".to_owned()),
        "the file was written to anyway"
    );
}

/// And a change that would do nothing is not turned away: saying "you told me to
/// leave this alone" about a value already where they want it reads as an obstacle
/// where there is none.
#[tokio::test]
async fn a_declared_setting_already_holding_what_was_asked_for_is_not_a_refusal() {
    let path = env_at("unmanaged-same", "PUID=1000\n");
    let context = a_context()
        .settings(crate::config::Settings {
            env_file: Some(path),
            unmanaged: vec![(
                "PUID".to_owned(),
                "I set the ids here by hand to match my NAS".to_owned(),
            )],
            ..crate::config::Settings::default()
        })
        .build();

    let outcome = configuration(&context, Setting::to("PUID", "1000")).await;

    assert_eq!(stance(&outcome), Some(Stance::Unchanged));
}

#[tokio::test]
async fn turning_port_forwarding_off_says_what_it_costs_there_and_then() {
    // The moment it is decided is the only moment worth saying it: afterwards
    // the check goes quiet, deliberately, because there is nothing to fix.
    let ctx = ctx(env_at("off", "VPN_PORT_FORWARDING=on\n"));
    let said = consequence(&configuration(&ctx, Setting::to(VPN_PORT_FORWARDING_KEY, "off")).await);
    assert_eq!(said.as_deref(), Some(crate::app::unforwarded::COST));
}

#[tokio::test]
async fn an_unrelated_setting_says_nothing_about_seeding() {
    // Every setting a stack has passes through here. A sentence about seeding
    // attached to a change that did not touch it reads as a warning nobody
    // caused, which is how operators learn to ignore them.
    let ctx = ctx(env_at("unrelated", "VPN_PORT_FORWARDING=off\n"));
    let said = consequence(&configuration(&ctx, Setting::to("LEMONFIBER_USENET", "on")).await)
        .unwrap_or_default();
    // Turning Usenet on has a consequence of its own — what it opens and what it
    // takes away — and the sentence is that one rather than the seeding sentence
    // sitting next to it.
    assert!(said.contains("whether Usenet runs"), "{said}");
    assert!(!said.contains(crate::app::unforwarded::COST), "{said}");
}

#[tokio::test]
async fn moving_the_data_location_says_what_it_affects_before_anything_moves() {
    // The sharpest change in the product: every *arr holds absolute paths to its
    // root folders, and an operator told this afterwards has already lost the
    // library the telling was for. So the sentence arrives while the old location
    // is still the one on disk.
    let path = env_at("moved", "DATA_ROOT=/srv/old\n");
    let ctx = ctx(path.clone());
    let staged = configuration(&ctx, Setting::to(crate::config::DATA_ROOT_KEY, "/srv/new")).await;

    let said = consequence(&staged).unwrap_or_default();
    assert!(said.contains("points at nothing"), "{said}");
    assert_eq!(stance(&staged), Some(Stance::Pending));
    assert_eq!(
        on_disk(&path, crate::config::DATA_ROOT_KEY).as_deref(),
        Some("/srv/old"),
        "a change nobody agreed to reached the file"
    );
}

#[tokio::test]
async fn the_same_move_confirmed_is_the_one_that_lands() {
    let path = env_at("moved-agreed", "DATA_ROOT=/srv/old\n");
    let ctx = ctx(path.clone());
    let applied = configuration(
        &ctx,
        Setting::to(crate::config::DATA_ROOT_KEY, "/srv/new").agreed(true),
    )
    .await;

    assert_eq!(stance(&applied), Some(Stance::Applied));
    assert_eq!(
        on_disk(&path, crate::config::DATA_ROOT_KEY).as_deref(),
        Some("/srv/new")
    );
}

#[tokio::test]
async fn a_change_asked_to_wait_lets_what_is_coming_down_finish_before_it_weighs_anything() {
    // The clients cannot be reached here, so the wait is over as soon as it begins.
    // What is proven is that a reduction asked to wait goes *through* the wait
    // rather than around it — a fixture that never drained would only prove that a
    // test can hang.
    let path = env_at("waited", "LEMONFIBER_TORRENT=on\n");
    let ctx = ctx(path.clone());
    let outcome = configuration(
        &ctx,
        Setting::to(crate::config::TORRENT_KEY, "off")
            .agreed(true)
            .waiting(Waiting::ForTheDownloads),
    )
    .await;

    assert_eq!(stance(&outcome), Some(Stance::Applied));
    assert_eq!(
        on_disk(&path, crate::config::TORRENT_KEY).as_deref(),
        Some("off")
    );
}

#[tokio::test]
async fn a_setting_already_holding_what_was_asked_for_has_nothing_to_do() {
    // Writing it again would move the file's own timestamp, which afterwards
    // reads as an edit somebody made outside lemonfiber.
    let path = env_at("same", "DATA_ROOT=/srv/media\n");
    let ctx = ctx(path);
    let outcome = configuration(
        &ctx,
        Setting::to(crate::config::DATA_ROOT_KEY, "/srv/media"),
    )
    .await;

    assert_eq!(stance(&outcome), Some(Stance::Unchanged));
    assert_eq!(consequence(&outcome), None);
    assert!(
        reviewed(&outcome).is_some_and(|review| !review.differs()),
        "a change that changes nothing reported one"
    );
}

#[tokio::test]
async fn a_setting_setup_never_asked_about_says_nothing_it_cannot_stand_behind() {
    // A cost invented for a setting whose consequences nobody worked out is worse
    // than silence: it teaches the operator to dismiss the ones that mean something.
    let ctx = ctx(env_at("unasked", ""));
    assert_eq!(
        consequence(&configuration(&ctx, Setting::to("LEMONFIBER_EXPLANATIONS", "on")).await),
        None
    );
}

#[tokio::test]
async fn a_key_pasted_with_a_newline_on_it_is_stored_as_the_key() {
    // The same paste error setup already absorbs, on the other way in. A key set
    // here with a newline still on it authenticates nowhere, while reading back
    // as though it were fine — which is the silent failure, not the loud one.
    let env = env_at("pasted", "");
    let ctx = ctx(env.clone());
    let written = configuration(&ctx, Setting::to(INDEXER_APIKEY_KEY, "  the-key\n")).await;
    assert!(written.is_ok());
    assert_eq!(
        on_disk(&env, INDEXER_APIKEY_KEY).as_deref(),
        Some("the-key")
    );
}

#[tokio::test]
async fn naming_a_front_door_says_what_naming_one_costs() {
    // The operator is choosing to stop lemonfiber keeping this answer right, and
    // the moment they choose it is the only moment they are weighing it.
    let ctx = ctx(env_at("door", ""));
    let said = consequence(&configuration(&ctx, Setting::to(FRONT_DOOR_KEY, "jellyfin")).await);
    assert_eq!(said.as_deref(), Some(crate::door::KEPT));
}

/// Where a read said one setting came from, or nothing where it answered
/// something else entirely.
fn origin_of(
    outcome: &Result<Outcome, Box<crate::error::Problem>>,
    key: &str,
) -> Option<crate::origin::Origin> {
    match outcome {
        Ok(Outcome::Config(report)) => report
            .settings
            .iter()
            .find(|setting| setting.key == key)
            .map(|setting| setting.origin.clone()),
        Ok(_) | Err(_) => None,
    }
}

#[tokio::test]
async fn a_setting_written_through_lemonfiber_is_read_back_as_the_operators() {
    let env = env_at("origin-written", "");
    let ctx = ctx(env.clone());
    let written = configuration(&ctx, Setting::to(EXPLANATIONS_KEY, "on")).await;
    assert!(written.is_ok());
    assert_eq!(on_disk(&env, EXPLANATIONS_KEY).as_deref(), Some("on"));

    let read = reading(&ctx, Some(EXPLANATIONS_KEY)).await;
    assert_eq!(
        origin_of(&read, EXPLANATIONS_KEY),
        Some(crate::origin::Origin::Operator)
    );
}

/// Put what a plugin install and one of its setting changes would write into this
/// machine's journal, by hand — nothing in this build writes a setting under a
/// plugin's name until recipes apply, so this is the shape they will write.
fn a_plugin_set(ctx: &Ctx, plugin: &str, key: &str, previous: Option<&str>, current: &str) {
    let changes = [
        crate::journal::Change {
            at: "1".to_owned(),
            operation: plugin.to_owned(),
            target: "document".to_owned(),
            kind: crate::journal::Kind::Made {
                path: format!("/stack/compose/plugins/{plugin}.yml"),
            },
        },
        crate::journal::Change {
            at: "1".to_owned(),
            operation: plugin.to_owned(),
            target: ".env".to_owned(),
            kind: crate::journal::Kind::Set {
                key: key.to_owned(),
                previous: previous.map(str::to_owned),
                current: current.to_owned(),
            },
        },
    ];
    let _ = crate::app::targets::layout(ctx).map(|paths| {
        crate::app::recover::journalled(&paths.journal(), &changes, ctx.random.as_ref());
    });
}

/// A context over that file with a stack directory beside it, which is what places
/// the journal: a machine set up has both.
fn set_up(env_file: &std::path::Path) -> Ctx {
    let mut ctx = ctx(env_file.to_path_buf());
    ctx.settings.stack_dir = env_file.parent().map(|dir| dir.join("data").join("stack"));
    ctx
}

/// Record `plugins` as installed, beside the settings file, or write a record that
/// will not read where there are none to name and `readable` is false.
fn registered(env: &std::path::Path, plugins: &[&str], readable: bool) {
    let entries: Vec<String> = plugins
        .iter()
        .map(|one| format!(r#"{{"plugin":"{one}","version":"1.0.0","services":[]}}"#))
        .collect();
    let text = if readable {
        format!(r#"{{"installed":[{}]}}"#, entries.join(","))
    } else {
        "{ not a register".to_owned()
    };
    let _ = env
        .parent()
        .map(|dir| std::fs::write(dir.join(crate::config::paths::PLUGINS), text));
}

/// A value an installed plugin set over the operator's is read as overridden, with
/// the value it replaced and whose that was; one a plugin left behind when it went
/// is read as orphaned; and where the record of what is installed will not read,
/// neither is claimed.
#[tokio::test]
async fn a_value_a_plugin_set_is_overridden_while_it_is_installed_and_orphaned_after() {
    let env = env_at("origin-overridden", "LEMONFIBER_EXPLANATIONS=off\n");
    let ctx = set_up(&env);
    a_plugin_set(&ctx, "komga", EXPLANATIONS_KEY, Some("on"), "off");

    registered(&env, &["komga"], true);
    let held = origin_of(
        &reading(&ctx, Some(EXPLANATIONS_KEY)).await,
        EXPLANATIONS_KEY,
    );
    assert!(
        matches!(&held, Some(crate::origin::Origin::Overridden { named, replaced })
            if named == "komga" && replaced.value.as_deref() == Some("on") && !replaced.from.is_settled()),
        "{held:?}"
    );

    registered(&env, &[], true);
    let held = origin_of(
        &reading(&ctx, Some(EXPLANATIONS_KEY)).await,
        EXPLANATIONS_KEY,
    );
    assert_eq!(
        held,
        Some(crate::origin::Origin::Orphaned {
            named: "komga".to_owned()
        })
    );

    registered(&env, &[], false);
    let held = origin_of(
        &reading(&ctx, Some(EXPLANATIONS_KEY)).await,
        EXPLANATIONS_KEY,
    );
    assert!(
        held.as_ref()
            .and_then(crate::origin::Origin::why)
            .is_some_and(|why| why.contains("will not read")),
        "{held:?}"
    );
}

/// A credential a plugin replaced is read out of the sealed journal to be judged,
/// and is never put on the listing.
#[tokio::test]
async fn a_credential_a_plugin_replaced_is_never_shown() {
    let env = env_at("origin-credential-replaced", "INDEXER_APIKEY=the-new-one\n");
    let ctx = set_up(&env);
    a_plugin_set(
        &ctx,
        "komga",
        INDEXER_APIKEY_KEY,
        Some("the-old-one"),
        "the-new-one",
    );
    registered(&env, &["komga"], true);

    let read = reading(&ctx, Some(INDEXER_APIKEY_KEY)).await;

    let held = origin_of(&read, INDEXER_APIKEY_KEY);
    assert!(
        matches!(&held, Some(crate::origin::Origin::Overridden { replaced, .. })
            if replaced.value.is_none() && replaced.withheld),
        "{held:?}"
    );
    let json = read
        .ok()
        .and_then(|outcome| serde_json::to_string(&outcome).ok())
        .unwrap_or_default();
    assert!(!json.is_empty() && !json.contains("the-old-one") && !json.contains("the-new-one"));
}

#[tokio::test]
async fn a_setting_lemonfiber_never_wrote_says_unknown_rather_than_bundled() {
    // The state a machine is in before its first change through lemonfiber, and
    // the one where a guess would be wrong most usefully: what the file holds
    // cannot be told from what setup left, so the read says so.
    let ctx = ctx(env_at("origin-unrecorded", "LEMONFIBER_EXPLANATIONS=on\n"));
    let held = origin_of(
        &reading(&ctx, Some(EXPLANATIONS_KEY)).await,
        EXPLANATIONS_KEY,
    );
    assert_ne!(held, Some(crate::origin::Origin::Bundled));
    assert!(
        held.as_ref()
            .and_then(crate::origin::Origin::why)
            .is_some_and(|why| why.contains("no record of writing here")),
        "{held:?}"
    );
}

#[tokio::test]
async fn a_credential_says_it_is_unknown_because_it_is_never_recorded() {
    // Not the same silence as the setting above. Keeping a record of a credential
    // would mean holding the value twice, so this one is unknown because a rule
    // is working rather than because something was lost — and it says which.
    let ctx = ctx(env_at("origin-credential", "INDEXER_APIKEY=abc123\n"));
    let held = origin_of(
        &reading(&ctx, Some(INDEXER_APIKEY_KEY)).await,
        INDEXER_APIKEY_KEY,
    );
    assert!(
        held.as_ref()
            .and_then(crate::origin::Origin::why)
            .is_some_and(|why| why.contains("keeps no record of a credential")),
        "{held:?}"
    );
}

#[tokio::test]
async fn reading_a_setting_is_never_a_decision() {
    let ctx = ctx(env_at("reading", "VPN_PORT_FORWARDING=off\n"));
    let read = reading(&ctx, Some(VPN_PORT_FORWARDING_KEY)).await;
    assert_eq!(consequence(&read), None);
    assert_eq!(reviewed(&read), None);
}

#[tokio::test]
async fn a_rehearsal_says_what_it_would_cost_and_writes_nothing() {
    // The review step and the rehearsal are the same thing said two ways, so the
    // one that changes nothing is the one most owed an account of what it would.
    let path = env_at("rehearsal", "VPN_PORT_FORWARDING=on\n");
    let mut rehearsing = ctx(path.clone());
    rehearsing.dry_run = true;
    let said = configuration(&rehearsing, Setting::to(VPN_PORT_FORWARDING_KEY, "off")).await;

    assert_eq!(
        consequence(&said).as_deref(),
        Some(crate::app::unforwarded::COST)
    );
    assert_eq!(stance(&said), Some(Stance::Pending));
    assert_eq!(
        on_disk(&path, VPN_PORT_FORWARDING_KEY).as_deref(),
        Some("on")
    );
}

#[tokio::test]
async fn nothing_but_a_settings_answer_is_read_for_a_consequence() {
    // The four readers above are total, and this is the arm that proves each of
    // them rather than a fallback nothing ever reaches.
    let other: Result<Outcome, Box<crate::error::Problem>> =
        Ok(Outcome::Version(crate::model::VersionReport {
            binary: "0".to_owned(),
            supported_schema: Vec::new(),
            stack: String::new(),
            compose: None,
            changelog: crate::changelog::Notes::unread(),
        }));
    let said = consequence(&other);
    assert!(said.is_some_and(|said| said.contains("not a configuration answer")));
    assert_eq!(origin_of(&other, EXPLANATIONS_KEY), None);

    let refused = Err(Box::new(store::Failure::Nowhere.problem()));
    assert_eq!(consequence(&refused), None);
    assert_eq!(reviewed(&refused), None);
    assert_eq!(stance(&refused), None);
    assert_eq!(origin_of(&refused, EXPLANATIONS_KEY), None);
}

// ── A replacement credential is proven before the one it replaces is dropped ──

#[tokio::test]
async fn a_replacement_key_the_indexer_accepts_is_the_one_that_lands() {
    let path = env_at(
        "key-good",
        "INDEXER_URL=https://indexer.example/api\nINDEXER_APIKEY=old-key\n",
    );
    let ctx = reaching(path.clone(), Answer::reply(200, ANSWERED));
    let outcome = configuration(&ctx, Setting::to(INDEXER_APIKEY_KEY, "new-key")).await;

    assert_eq!(stance(&outcome), Some(Stance::Applied));
    assert_eq!(
        on_disk(&path, INDEXER_APIKEY_KEY).as_deref(),
        Some("new-key")
    );
    let proven = reviewed(&outcome).and_then(|review| review.proof);
    assert!(
        matches!(&proven, Some(crate::validate::Validation::Valid { observed })
            if observed.contains("answered a search")),
        "{proven:?}"
    );
}

#[tokio::test]
async fn a_replacement_key_the_indexer_refuses_never_reaches_the_file() {
    // The whole point of proving first: a bad paste must not cost the operator the
    // key that works.
    let path = env_at(
        "key-bad",
        "INDEXER_URL=https://indexer.example/api\nINDEXER_APIKEY=old-key\n",
    );
    let ctx = reaching(path.clone(), Answer::reply(401, ""));
    let outcome = configuration(&ctx, Setting::to(INDEXER_APIKEY_KEY, "mistyped")).await;

    assert_eq!(stance(&outcome), Some(Stance::Blocked));
    assert_eq!(
        on_disk(&path, INDEXER_APIKEY_KEY).as_deref(),
        Some("old-key")
    );
    let why = reviewed(&outcome).and_then(|review| review.refusal);
    assert!(
        why.is_some_and(|why| why.contains("the one in force was kept")),
        "the refusal says nothing about what was kept"
    );
}

/// A blanket yes is exactly what a bad paste would be waved through by, so the
/// one answer that means *this credential is wrong* is not confirmable.
#[tokio::test]
async fn a_refusal_is_not_something_a_confirmation_gets_past() {
    let path = env_at(
        "key-bad-confirmed",
        "INDEXER_URL=https://indexer.example/api\nINDEXER_APIKEY=old-key\n",
    );
    let ctx = reaching(path.clone(), Answer::reply(401, ""));
    let outcome = configuration(
        &ctx,
        Setting::to(INDEXER_APIKEY_KEY, "mistyped").agreed(true),
    )
    .await;

    assert_eq!(stance(&outcome), Some(Stance::Blocked));
    assert_eq!(
        on_disk(&path, INDEXER_APIKEY_KEY).as_deref(),
        Some("old-key")
    );
}

/// A key the indexer authenticated and then rate-limited is the right key. What
/// is wrong is the account behind it, and keeping the old one does not fix that.
#[tokio::test]
async fn a_key_that_authenticated_but_is_limited_is_still_the_right_key() {
    let path = env_at(
        "key-limited",
        "INDEXER_URL=https://indexer.example/api\nINDEXER_APIKEY=old-key\n",
    );
    let limited = r#"<error code="500" description="Request limit reached"/>"#;
    let ctx = reaching(path.clone(), Answer::reply(200, limited));
    let outcome = configuration(&ctx, Setting::to(INDEXER_APIKEY_KEY, "new-key")).await;

    assert_eq!(stance(&outcome), Some(Stance::Applied));
    assert_eq!(
        on_disk(&path, INDEXER_APIKEY_KEY).as_deref(),
        Some("new-key")
    );
}

#[tokio::test]
async fn an_address_given_before_a_key_is_not_put_to_a_service_at_all() {
    // Half a credential proven against an indexer would be refused on the half
    // nobody has given yet, which would make the first of two changes impossible.
    let path = env_at("half", "");
    let ctx = ctx(path.clone());
    let outcome = configuration(
        &ctx,
        Setting::to(INDEXER_URL_KEY, "https://indexer.example/api"),
    )
    .await;

    assert_eq!(stance(&outcome), Some(Stance::Applied));
    assert_eq!(reviewed(&outcome).and_then(|review| review.proof), None);
}

/// Nothing answering is not the same as a refusal: an operator working offline may
/// know the credential is right, and refusing them forever would make the setting
/// unchangeable — which is the trap reconfiguration exists to close.
#[tokio::test]
async fn a_replacement_nothing_could_prove_is_held_until_it_is_confirmed() {
    let path = env_at("tls-off", A_LOGIN);
    let ctx = ctx(path.clone());
    let held = configuration(&ctx, Setting::to(PROVIDER_TLS_KEY, "off")).await;

    assert_eq!(stance(&held), Some(Stance::Blocked));
    assert_eq!(on_disk(&path, PROVIDER_TLS_KEY).as_deref(), Some("on"));
    let why = reviewed(&held).and_then(|review| review.refusal);
    assert!(
        why.is_some_and(|why| why.contains("Confirm the change to store it unproven")),
        "the refusal says nothing about the way past it"
    );

    let confirmed = configuration(&ctx, Setting::to(PROVIDER_TLS_KEY, "off").agreed(true)).await;
    assert_eq!(stance(&confirmed), Some(Stance::Applied));
    assert_eq!(on_disk(&path, PROVIDER_TLS_KEY).as_deref(), Some("off"));
}

/// A port that is not a port number cannot be dialled and cannot be corrected by
/// the provider, so it never reaches the file at all.
#[tokio::test]
async fn a_replacement_the_product_cannot_read_leaves_the_file_alone() {
    let path = env_at("port-bad", A_LOGIN);
    let ctx = ctx(path.clone());
    let outcome = configuration(
        &ctx,
        Setting::to(PROVIDER_PORT_KEY, "five-six-three").agreed(true),
    )
    .await;

    assert_eq!(stance(&outcome), Some(Stance::Blocked));
    assert_eq!(on_disk(&path, PROVIDER_PORT_KEY).as_deref(), Some("563"));
    let why = reviewed(&outcome).and_then(|review| review.refusal);
    assert!(
        why.is_some_and(|why| why.contains("1 to 65535")),
        "the refusal says nothing a port could be corrected to"
    );
}

#[tokio::test]
async fn a_change_to_a_credential_is_withheld_on_both_sides_of_the_difference() {
    let path = env_at(
        "withheld",
        "INDEXER_URL=https://indexer.example/api\nINDEXER_APIKEY=old-key\n",
    );
    let ctx = reaching(path, Answer::reply(200, ANSWERED));
    let outcome = configuration(&ctx, Setting::to(INDEXER_APIKEY_KEY, "new-key")).await;

    let change = reviewed(&outcome).map(|review| review.change);
    assert_eq!(
        change.map(|change| (change.from, change.to)),
        Some((Some(store::REDACTED.to_owned()), store::REDACTED.to_owned()))
    );
}
