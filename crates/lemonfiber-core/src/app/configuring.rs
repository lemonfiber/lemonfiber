//! Reading and changing one setting.
//!
//! Its own module rather than part of the lifecycle engine, because this is the
//! one command that *writes* what every other command then reads, and the writing
//! carries a duty the reading does not: a change with a consequence has to say so
//! at the moment it is made, which is the only moment the operator is deciding.

use crate::config::{port_forward_from_env, store};
use crate::error::{Diagnose, Problem};
use crate::model::{ConfigReport, SettingReport};

use super::{Ctx, Outcome};

/// Read or change settings.
///
/// A rehearsal reads and reports what it would have written without writing it,
/// so `--dry-run` means the same thing here as everywhere else.
///
/// The failure is boxed. This is the only fallible path here that is not async,
/// so it is the only one where a large error variant sits in the returned value
/// rather than inside a future — and a problem is a rare, cold thing that is
/// cheaper to move behind a pointer.
pub(super) fn configuration(
    ctx: &Ctx,
    key: Option<&str>,
    value: Option<&str>,
) -> Result<Outcome, Box<Problem>> {
    let Some(path) = ctx.settings.env_file.as_deref() else {
        return Err(Box::new(store::Failure::Nowhere.problem()));
    };

    // Whether this call actually writes. A rehearsal has decided nothing and a read
    // is not a decision, so neither has a consequence to state.
    let written = key.is_some() && value.is_some() && !ctx.dry_run;

    // What was forwarding before, read only where a write is about to change it:
    // a consequence is the difference a change made, and where nothing is being
    // written there is no difference to state. A read that fails says nothing —
    // the write that follows reports the failure itself, in its own words.
    let before = written
        .then(|| store::read(path).ok())
        .flatten()
        .map(|file| port_forward_from_env(&file));

    // Trimmed on the way in, for the same reason setup trims what is pasted into it:
    // a key copied from a dashboard carries a trailing newline, it authenticates
    // nowhere, and the file format has no way to mean the whitespace deliberately. The
    // parser already trims the name; the value was the half still taken literally.
    let value = value.map(str::trim);

    let changed = match (key, value) {
        (Some(key), Some(value)) if !ctx.dry_run => {
            if let Err(err) = store::set(path, key, value) {
                return Err(Box::new(err.problem()));
            }
            true
        }
        (_, value) => value.is_some(),
    };

    let file = match store::read(path) {
        Ok(file) => file,
        Err(err) => return Err(Box::new(err.problem())),
    };
    let consequence = stated(ctx, key, written, before.as_ref(), &file);
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
    }))
}

/// What the change just made costs, where it costs anything.
///
/// One sentence rather than a list, because one call writes one setting: the change
/// either has a cost worth stating or it has none.
///
/// Naming a front door is the one change whose consequence does not depend on what
/// the setting was before. Every other answer this product gives about the door is
/// worked out afresh, so a stack that changes is answered about as it is; a named
/// one is answered about as it was decided, and saying so belongs at the moment it
/// is decided.
///
/// The forwarded port is nothing where the stack does not torrent: a forwarded port
/// buys it nothing, so the sentence would be about a problem this operator cannot
/// have.
fn stated(
    ctx: &Ctx,
    key: Option<&str>,
    written: bool,
    before: Option<&crate::config::PortForward>,
    file: &crate::config::env::EnvFile,
) -> Option<String> {
    if !written {
        return None;
    }
    if key == Some(crate::config::FRONT_DOOR_KEY) {
        return Some(crate::door::KEPT.to_owned());
    }
    // Every answer setup wrote says what changing it affects, and the catalogue is the
    // one place that knows. A surface that writes a setting cannot then state a cost
    // the rest of the product disagrees with, and a decision nobody catalogued says
    // nothing rather than a guess.
    if let Some(entry) = key.and_then(crate::reconfigure::decision) {
        return Some(format!("changing this affects {}", entry.affects));
    }
    before
        .and_then(|before| super::seeding::on_change(before, &port_forward_from_env(file)))
        .filter(|_| ctx.settings.protocols.torrent)
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::configuration;
    use crate::app::Outcome;
    use crate::config::{FRONT_DOOR_KEY, VPN_PORT_FORWARDING_KEY};
    use crate::test_support::a_context;

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
    fn ctx(env_file: std::path::PathBuf) -> crate::app::Ctx {
        a_context()
            .runner(std::sync::Arc::new(crate::test_support::Scripted(Ok(
                crate::test_support::spoke(""),
            ))))
            .settings(crate::config::Settings {
                env_file: Some(env_file),
                protocols: crate::config::Protocols::both(),
                ..crate::config::Settings::default()
            })
            .build()
    }

    /// What a change said it cost, where it said anything.
    fn consequence(outcome: Result<Outcome, Box<crate::error::Problem>>) -> Option<String> {
        outcome.ok().and_then(|outcome| match outcome {
            Outcome::Config(report) => report.consequence,
            other => Some(format!("{other:?} is not a configuration answer")),
        })
    }

    #[test]
    fn turning_port_forwarding_off_says_what_it_costs_there_and_then() {
        // The moment it is decided is the only moment worth saying it: afterwards
        // the check goes quiet, deliberately, because there is nothing to fix.
        let ctx = ctx(env_at("off", "VPN_PORT_FORWARDING=on\n"));
        let said = consequence(configuration(
            &ctx,
            Some(VPN_PORT_FORWARDING_KEY),
            Some("off"),
        ));
        assert_eq!(said.as_deref(), Some(crate::app::seeding::COST));
    }

    #[test]
    fn an_unrelated_setting_says_nothing_about_seeding() {
        // Every setting a stack has passes through here. A sentence about seeding
        // attached to a change that did not touch it reads as a warning nobody
        // caused, which is how operators learn to ignore them.
        let ctx = ctx(env_at("unrelated", "VPN_PORT_FORWARDING=off\n"));
        let said = consequence(configuration(&ctx, Some("LEMONFIBER_USENET"), Some("on")))
            .unwrap_or_default();
        // Turning Usenet on has a consequence of its own — what it opens and what it
        // takes away — and the sentence is that one rather than the seeding sentence
        // sitting next to it.
        assert!(said.contains("whether Usenet runs"), "{said}");
        assert!(!said.contains(crate::app::seeding::COST), "{said}");
    }

    #[test]
    fn moving_the_data_location_says_what_it_affects_as_it_is_written() {
        // The sharpest change in the product: every *arr holds absolute paths to its
        // root folders, and an operator told this afterwards has already lost the
        // library the telling was for.
        let ctx = ctx(env_at("moved", "DATA_ROOT=/srv/old\n"));
        let said = consequence(configuration(
            &ctx,
            Some(crate::config::DATA_ROOT_KEY),
            Some("/srv/new"),
        ))
        .unwrap_or_default();
        assert!(said.contains("points at nothing"), "{said}");
    }

    #[test]
    fn a_setting_setup_never_asked_about_says_nothing_it_cannot_stand_behind() {
        // A cost invented for a setting whose consequences nobody worked out is worse
        // than silence: it teaches the operator to dismiss the ones that mean something.
        let ctx = ctx(env_at("unasked", ""));
        assert_eq!(
            consequence(configuration(
                &ctx,
                Some("LEMONFIBER_EXPLANATIONS"),
                Some("on")
            )),
            None
        );
    }

    #[test]
    fn a_key_pasted_with_a_newline_on_it_is_stored_as_the_key() {
        // The same paste error setup already absorbs, on the other way in. A key set
        // here with a newline still on it authenticates nowhere, while reading back
        // as though it were fine — which is the silent failure, not the loud one.
        let env = env_at("pasted", "");
        let ctx = ctx(env.clone());
        let written = configuration(
            &ctx,
            Some(crate::config::INDEXER_APIKEY_KEY),
            Some("  the-key\n"),
        );
        assert!(written.is_ok());
        let file = crate::config::store::read(&env).unwrap_or_default();
        assert_eq!(file.get(crate::config::INDEXER_APIKEY_KEY), Some("the-key"));
    }

    #[test]
    fn naming_a_front_door_says_what_naming_one_costs() {
        // The operator is choosing to stop lemonfiber keeping this answer right, and
        // the moment they choose it is the only moment they are weighing it.
        let ctx = ctx(env_at("door", ""));
        let said = consequence(configuration(&ctx, Some(FRONT_DOOR_KEY), Some("jellyfin")));
        assert_eq!(said.as_deref(), Some(crate::door::KEPT));
    }

    #[test]
    fn reading_the_named_front_door_back_costs_nothing_to_say() {
        // Reading is not deciding, and a rehearsal has decided nothing either.
        let ctx = ctx(env_at("door-read", "LEMONFIBER_FRONT_DOOR=jellyfin\n"));
        assert_eq!(
            consequence(configuration(&ctx, Some(FRONT_DOOR_KEY), None)),
            None
        );

        let mut rehearsing = ctx;
        rehearsing.dry_run = true;
        assert_eq!(
            consequence(configuration(
                &rehearsing,
                Some(FRONT_DOOR_KEY),
                Some("jellyfin")
            )),
            None
        );
    }

    #[test]
    fn a_rehearsal_decides_nothing_and_so_states_nothing() {
        // It has not changed anything, and a consequence stated for a change that
        // did not happen is the tool reporting a decision the operator never made.
        let mut rehearsing = ctx(env_at("rehearsal", "VPN_PORT_FORWARDING=on\n"));
        rehearsing.dry_run = true;
        let said = consequence(configuration(
            &rehearsing,
            Some(VPN_PORT_FORWARDING_KEY),
            Some("off"),
        ));
        assert_eq!(said, None);
    }

    #[test]
    fn reading_a_setting_is_never_a_decision() {
        let ctx = ctx(env_at("reading", "VPN_PORT_FORWARDING=off\n"));
        assert_eq!(
            consequence(configuration(&ctx, Some(VPN_PORT_FORWARDING_KEY), None)),
            None
        );
    }

    #[test]
    fn nothing_but_a_settings_answer_is_read_for_a_consequence() {
        // The reader above is total, and this is the arm that proves it rather
        // than a fallback nothing ever reaches.
        let other = Outcome::Version(crate::model::VersionReport {
            binary: "0".to_owned(),
            supported_schema: Vec::new(),
            stack: String::new(),
            compose: None,
        });
        assert!(
            consequence(Ok(other)).is_some_and(|said| said.contains("not a configuration answer"))
        );
    }
}
