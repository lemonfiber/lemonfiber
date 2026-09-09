//! Reading and changing one setting.
//!
//! Its own module rather than part of the lifecycle engine, because this is the
//! one command that *writes* what every other command then reads, and the writing
//! carries a duty the reading does not: a change with a consequence has to say so
//! at the moment it is made, which is the only moment the operator is deciding.
//!
//! So a change is weighed before it is written, never after. What the setting holds
//! now and what it would hold are put side by side; a change setup catalogued as
//! consequential is staged until somebody says yes to it; and a replacement for one
//! half of a credential is proven against the live service while the credential it
//! replaces is still the one on disk. A proposal that does not clear all of that
//! leaves the file exactly as it was and says which of them it failed.

mod proving;

use crate::config::{env::EnvFile, port_forward_from_env, store};
use crate::error::{Diagnose, Problem};
use crate::model::{ConfigReport, SettingReport};
use crate::reconfigure::{Consent, Review, Stance};
use crate::validate::{Credential, Validation};

use proving::Proving;

use super::{Ctx, Outcome, Setting, Waiting};

/// What the operator said about this change beyond what the change is.
///
/// Two words a surface already has — a `--confirm` on a command line and a `confirm`
/// in a request body are one word, and so are the two `wait`s — carried together
/// because they are answers to the same question: something stands between this
/// change and the file, and here is what to do about it. One says go ahead anyway;
/// the other says let what is in flight finish first, which is the offer a reduction
/// makes rather than a way past it.
#[derive(Clone, Copy)]
struct Asked {
    /// Whether the operator has agreed to what the change costs.
    confirmed: bool,
    /// Whether they asked for what is still coming down to finish first.
    waiting: Waiting,
}

impl Asked {
    /// What the request said, taken off it.
    const fn of(change: &Setting) -> Self {
        Self {
            confirmed: change.confirmed,
            waiting: change.waiting,
        }
    }
}

/// Read or change settings.
///
/// A rehearsal reads and reports what it would have written without writing it,
/// so `--dry-run` means the same thing here as everywhere else.
///
/// The failure is boxed because a problem is a rare, cold thing that is cheaper to
/// move behind a pointer than to carry in every returned value.
///
/// # Errors
///
/// Returns the [`Problem`] for a machine with nowhere to keep settings, or for a
/// settings file that could not be read or written.
pub(super) async fn configuration(ctx: &Ctx, change: Setting) -> Result<Outcome, Box<Problem>> {
    // Trimmed on the way in, for the same reason setup trims what is pasted into it:
    // a key copied from a dashboard carries a trailing newline, it authenticates
    // nowhere, and the file format has no way to mean the whitespace deliberately. The
    // parser already trims the name; the value was the half still taken literally.
    let change = Setting {
        value: change.value.trim().to_owned(),
        ..change
    };
    settings(ctx, Some(&change.key), Some(&change)).await
}

/// Read one setting, or all of them.
///
/// A read decides nothing, so there is no proposal to weigh and nothing to write.
///
/// # Errors
///
/// Returns the [`Problem`] for a machine with nowhere to keep settings, or for a
/// settings file that could not be read.
pub(super) async fn reading(ctx: &Ctx, key: Option<&str>) -> Result<Outcome, Box<Problem>> {
    settings(ctx, key, None).await
}

/// Both halves: the settings as they stand, and what a change to one comes to.
async fn settings(
    ctx: &Ctx,
    key: Option<&str>,
    change: Option<&Setting>,
) -> Result<Outcome, Box<Problem>> {
    let Some(path) = ctx.settings.env_file.as_deref() else {
        return Err(Box::new(store::Failure::Nowhere.problem()));
    };

    // Read before anything is decided rather than after the write, because the diff an
    // operator is shown is the difference between this file and the one proposed, and a
    // file read afterwards is already the answer to the question.
    let held = match store::read(path) {
        Ok(file) => file,
        Err(err) => return Err(Box::new(err.problem())),
    };

    let (file, changed, consequence, review) = match change {
        Some(change) => {
            let asked = Asked::of(change);
            let proposal = applying(ctx, path, held, (&change.key, &change.value), asked).await?;
            (
                proposal.file,
                proposal.review.differs(),
                proposal.consequence,
                Some(proposal.review),
            )
        }
        None => (held, false, None, None),
    };
    let settings = store::shown(&file)
        .into_iter()
        .filter(|setting| key.is_none_or(|wanted| setting.key == wanted))
        .map(SettingReport::from)
        .collect();

    Ok(Outcome::Config(ConfigReport {
        settings,
        changed,
        rehearsed: ctx.dry_run,
        consequence,
        review,
    }))
}

/// A proposed change, the sentence saying what making it costs, and the settings as
/// they stand once it has been dealt with.
struct Proposal {
    /// The difference, and where it stands.
    review: Review,
    /// What making it decided, where it decided something worth stating.
    consequence: Option<String>,
    /// The file as it now is — changed where the proposal reached it, and exactly as
    /// it was where it did not.
    ///
    /// Carried rather than read back off the disk, because a second read would be a
    /// second failure to report for one command, and the copy that failed would be
    /// the one no test could reach.
    file: EnvFile,
}

/// The change weighed, proven where a service can prove it, and written where nothing
/// stands in the way.
///
/// # Errors
///
/// Returns the [`Problem`] for a settings file that could not be written.
async fn applying(
    ctx: &Ctx,
    path: &std::path::Path,
    held: EnvFile,
    change: (&str, &str),
    asked: Asked,
) -> Result<Proposal, Box<Problem>> {
    let (key, value) = change;
    // The offer to wait, taken up. It runs before anything is weighed, so what the
    // proposal then finds in flight is what is still in flight after the wait — and
    // only for a change that takes something away, since a wait asked of one that
    // does not would sit in front of every download on the machine for nothing.
    if asked.waiting == Waiting::ForTheDownloads
        && !ctx.dry_run
        && super::reconfiguring::waits_for_downloads(ctx, key, value)
    {
        super::engine::drained(ctx, &[]).await;
    }

    let review = weighed(ctx, &held, key, value, asked.confirmed).await;
    let consequence = stated(ctx, &review, &held, key, value);
    let mut file = held;
    if review.writes() {
        if let Err(err) = store::set(path, key, value) {
            return Err(Box::new(err.problem()));
        }
        file.set(key, value);
        // Recorded as what lemonfiber last wrote here, so the next change can tell an
        // operator's edit from lemonfiber's own value rather than overwriting one
        // without saying so.
        super::reconfiguring::record(ctx, key, value);
    }
    Ok(Proposal {
        review,
        consequence,
        file,
    })
}

/// Where the proposal stands once everything that could stop it has been asked.
///
/// The classification comes first and the service second, deliberately: a change
/// nobody has agreed to is not going to happen, and reaching a live indexer to
/// prove a key for it would be spending somebody's rate limit on a decision that
/// has not been taken.
async fn weighed(ctx: &Ctx, held: &EnvFile, key: &str, value: &str, confirmed: bool) -> Review {
    let review = Review::proposed(
        key,
        held.get(key),
        value,
        &Consent {
            settled: confirmed,
            rehearsing: ctx.dry_run,
        },
    );
    if !review.differs() {
        return review;
    }
    // What the change comes to on this machine — where the library would land, what
    // is still coming down, what was edited underneath, what it opens and keeps —
    // worked out for a staged proposal as well as one about to land. A review that
    // withheld this until after the yes was given would be asking for a yes to
    // something unstated.
    let review = super::reconfiguring::assessed(ctx, review, held, (key, value), confirmed).await;
    if !review.writes() {
        return review;
    }
    match proving::wanted(held, key, value) {
        Proving::Nothing | Proving::Incomplete => review,
        Proving::Unreadable(why) => review.blocked(why),
        Proving::Replacement(replacement) => answered(ctx, review, &replacement, confirmed).await,
    }
}

/// The proposal once the live service has answered about the replacement.
///
/// A service that answered and *refused* is the one answer no confirmation gets past.
/// The whole point of proving a replacement first is that a bad paste must not cost the
/// operator the credential that works, and a blanket yes is exactly what a bad paste
/// would be waved through by.
///
/// Nothing answering at all is a different thing, and it is confirmable: an operator
/// working offline, or reaching a provider this machine cannot see, may know the
/// credential is right, and refusing them forever would make the setting unchangeable —
/// which is the trap reconfiguration exists to close. It is stored unproven and said to
/// be unproven.
///
/// A credential that authenticated but cannot do its job is stored. It is the right
/// credential; what is wrong is the account behind it, and that is not fixed by keeping
/// the old one.
async fn answered(ctx: &Ctx, review: Review, replacement: &Credential, confirmed: bool) -> Review {
    let proof = ctx.validator.validate(replacement).await.withheld();
    let refusal = match &proof {
        Validation::Rejected { detail } => Some(format!(
            "the service refused the replacement, so the one in force was kept: {detail}"
        )),
        Validation::Unreachable { detail } if !confirmed => Some(format!(
            "the replacement could not be proven, so the one in force was kept: {detail}. \
             Confirm the change to store it unproven"
        )),
        Validation::Valid { .. } | Validation::Degraded { .. } | Validation::Unreachable { .. } => {
            None
        }
    };
    let review = review.proven(proof);
    match refusal {
        Some(why) => review.blocked(why),
        None => review,
    }
}

/// What the proposed change costs, where it costs anything.
///
/// One sentence rather than a list, because one call changes one setting: the change
/// either has a cost worth stating or it has none. Stated for a change that is only
/// staged as well as for one that landed — a review step that withheld the cost until
/// after the write would be a review step in name only.
///
/// Naming a front door is the one change whose consequence does not depend on what
/// the setting was before. Every other answer this product gives about the door is
/// worked out afresh, so a stack that changes is answered about as it is; a named
/// one is answered about as it was decided, and saying so belongs at the moment it
/// is decided.
///
/// The forwarded port is nothing where the stack does not torrent: a forwarded port
/// buys it nothing, so the sentence would be about a problem this operator cannot
/// have. It is worked out from the difference rather than from the file on disk, so a
/// rehearsal is told what it would cost as plainly as a write is told what it did.
fn stated(ctx: &Ctx, review: &Review, held: &EnvFile, key: &str, value: &str) -> Option<String> {
    if review.stance == Stance::Unchanged {
        return None;
    }
    if key == crate::config::FRONT_DOOR_KEY {
        return Some(crate::door::KEPT.to_owned());
    }
    // Every answer setup wrote says what changing it affects, and the catalogue is the
    // one place that knows. A surface that writes a setting cannot then state a cost
    // the rest of the product disagrees with, and a decision nobody catalogued says
    // nothing rather than a guess.
    if let Some(entry) = crate::reconfigure::decision(key) {
        return Some(format!("changing this affects {}", entry.affects));
    }
    if !ctx.settings.protocols.torrent {
        return None;
    }
    let mut proposed = held.clone();
    proposed.set(key, value);
    super::seeding::on_change(
        &port_forward_from_env(held),
        &port_forward_from_env(&proposed),
    )
    .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{configuration, reading, Setting};
    use crate::app::{Ctx, Outcome, Waiting};
    use crate::config::{
        store, FRONT_DOOR_KEY, INDEXER_APIKEY_KEY, INDEXER_URL_KEY, PROVIDER_PORT_KEY,
        PROVIDER_TLS_KEY, VPN_PORT_FORWARDING_KEY,
    };
    use crate::error::Diagnose;
    use crate::reconfigure::{Review, Stance};
    use crate::test_support::a_context;
    use lemonfiber_fixtures::http::{Answer, Fake};

    /// A search a Torznab indexer answers with, which proves a key.
    const ANSWERED: &str = "<rss><channel><item/></channel></rss>";

    /// A scratch environment file holding the given settings.
    fn env_at(name: &str, contents: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "lemonfiber-configuring-{}-{name}",
            std::process::id()
        ));
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

    #[tokio::test]
    async fn turning_port_forwarding_off_says_what_it_costs_there_and_then() {
        // The moment it is decided is the only moment worth saying it: afterwards
        // the check goes quiet, deliberately, because there is nothing to fix.
        let ctx = ctx(env_at("off", "VPN_PORT_FORWARDING=on\n"));
        let said =
            consequence(&configuration(&ctx, Setting::to(VPN_PORT_FORWARDING_KEY, "off")).await);
        assert_eq!(said.as_deref(), Some(crate::app::seeding::COST));
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
        assert!(!said.contains(crate::app::seeding::COST), "{said}");
    }

    #[tokio::test]
    async fn moving_the_data_location_says_what_it_affects_before_anything_moves() {
        // The sharpest change in the product: every *arr holds absolute paths to its
        // root folders, and an operator told this afterwards has already lost the
        // library the telling was for. So the sentence arrives while the old location
        // is still the one on disk.
        let path = env_at("moved", "DATA_ROOT=/srv/old\n");
        let ctx = ctx(path.clone());
        let staged =
            configuration(&ctx, Setting::to(crate::config::DATA_ROOT_KEY, "/srv/new")).await;

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
            Some(crate::app::seeding::COST)
        );
        assert_eq!(stance(&said), Some(Stance::Pending));
        assert_eq!(
            on_disk(&path, VPN_PORT_FORWARDING_KEY).as_deref(),
            Some("on")
        );
    }

    #[tokio::test]
    async fn nothing_but_a_settings_answer_is_read_for_a_consequence() {
        // The three readers above are total, and this is the arm that proves each of
        // them rather than a fallback nothing ever reaches.
        let other = Outcome::Version(crate::model::VersionReport {
            binary: "0".to_owned(),
            supported_schema: Vec::new(),
            stack: String::new(),
            compose: None,
        });
        let said = consequence(&Ok(other));
        assert!(said.is_some_and(|said| said.contains("not a configuration answer")));

        let refused = Err(Box::new(store::Failure::Nowhere.problem()));
        assert_eq!(consequence(&refused), None);
        assert_eq!(reviewed(&refused), None);
        assert_eq!(stance(&refused), None);
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

        let confirmed =
            configuration(&ctx, Setting::to(PROVIDER_TLS_KEY, "off").agreed(true)).await;
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
}
