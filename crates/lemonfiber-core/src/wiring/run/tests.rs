use super::{listing, substituting, wiring, CANNOT_FILL, CHOICE_UNWRITABLE, NOTHING_ASKS};
use crate::app::{Filling, Linking, Outcome};
use crate::wiring::{Reaches, FILLS_KEY};

/// A scratch layout with somewhere to keep a setting and somewhere to keep a
/// journal, which is what a substitution needs and a listing does not.
fn dir(named: &str) -> std::path::PathBuf {
    let at = lemonfiber_fixtures::scratch::Scratch::named(&format!("wiring-{named}")).kept();
    let _ = std::fs::remove_dir_all(&at);
    let _ = std::fs::create_dir_all(at.join("config"));
    let _ = std::fs::create_dir_all(at.join("data"));
    at
}

/// A context over the stack this build pins, keeping its settings in `named`.
fn ctx(named: &str) -> (crate::app::Ctx, std::path::PathBuf) {
    let at = dir(named);
    let settings = crate::config::Settings {
        env_file: Some(at.join("config").join(".env")),
        stack_dir: Some(at.join("data").join("stack")),
        ..crate::config::Settings::default()
    };
    (
        crate::test_support::a_context().settings(settings).build(),
        at,
    )
}

/// A choice of `service` for `capability`, with nothing said and no offer answered.
fn fill(capability: &str, service: &str) -> Filling {
    Filling {
        capability: capability.to_owned(),
        service: service.to_owned(),
        reason: None,
        agreement: None,
    }
}

/// The choice read, and then answered with the name its reading printed.
fn agreed(
    ctx: &crate::app::Ctx,
    filling: Filling,
) -> Result<crate::model::SubstitutionReport, Box<crate::error::Problem>> {
    let reading = substituting(ctx, &filling)?;
    substituting(
        ctx,
        &Filling {
            agreement: Some(reading.agreement),
            ..filling
        },
    )
}

/// What the settings file says the operator chose.
fn recorded(at: &std::path::Path) -> Option<String> {
    crate::config::store::read(&at.join("config").join(".env"))
        .ok()
        .and_then(|held| held.get(FILLS_KEY).map(str::to_owned))
}

/// What one asked-for capability reaches, as the listing answers it.
fn asked(report: &crate::model::WiringReport, by: &str) -> Option<Vec<String>> {
    report.wired.iter().find_map(|link| match &link.reaches {
        Reaches::Asked { services, .. } if link.by == by => Some(services.clone()),
        _ => None,
    })
}

/// The listing answers over the stack this build pins, with nothing chosen.
#[test]
fn the_listing_answers_every_link_the_pinned_stack_declares() {
    let (ctx, _at) = ctx("listing");
    let report = listing(&ctx).ok();
    assert_eq!(
        report.as_ref().map(|report| report.wired.len() > 10),
        Some(true),
        "{report:?}"
    );
    assert_eq!(
        report.as_ref().map(|report| report.unfilled.clone()),
        Some(Vec::new())
    );
    assert_eq!(
        report.as_ref().and_then(|report| asked(report, "seerr")),
        Some(vec!["jellyfin".to_owned()])
    );
}

/// A rehearsal works the whole thing out and writes nothing, which is the answer
/// somebody wants before agreeing to it.
#[test]
fn a_rehearsed_substitution_answers_in_full_and_writes_nothing() {
    let (ctx, at) = ctx("rehearsed");
    let rehearsing = ctx.rehearsing();
    let filling = fill("indexer.search", "nzbhydra2");

    let report = substituting(&rehearsing, &filling).ok();

    assert_eq!(report.as_ref().map(|report| report.applied), Some(false));
    assert_eq!(
        report
            .as_ref()
            .map(|report| report.substitution.was.clone()),
        Some(Some("prowlarr".to_owned()))
    );
    assert_eq!(
        report
            .as_ref()
            .map(|report| report.substitution.asked_by.clone()),
        Some(vec!["bindery".to_owned()])
    );
    assert_eq!(recorded(&at), None, "a rehearsal recorded a choice");
}

/// A real run records the choice, and the next listing reads it back.
#[test]
fn a_substitution_is_recorded_and_read_back() {
    let (ctx, at) = ctx("recorded");
    let filling = fill("indexer.search", "nzbhydra2");

    let report = agreed(&ctx, filling).ok();

    assert_eq!(report.map(|report| report.applied), Some(true));
    assert_eq!(recorded(&at).as_deref(), Some("indexer.search=nzbhydra2"));

    let after = listing(&ctx).ok();
    assert_eq!(
        after.and_then(|after| asked(&after, "bindery")),
        Some(vec!["nzbhydra2".to_owned()])
    );
}

/// Each refusal is its own mistake with its own next step, so each carries its
/// own code rather than one that did not work.
#[test]
fn each_refusal_carries_the_code_that_says_which_mistake_it_was() {
    let (ctx, _at) = ctx("refused");
    let refused = |capability: &str, service: &str| {
        let problem = substituting(&ctx, &fill(capability, service)).err();
        // Answered at the status the published list gives its code, so a client
        // reading the list before it meets one reads the status it will meet.
        if let Some(problem) = &problem {
            assert_eq!(
                crate::wiring::REFUSED
                    .iter()
                    .find(|(code, _)| *code == problem.code)
                    .map(|(_, amiss)| *amiss),
                Some(problem.amiss),
                "{}",
                problem.code
            );
        }
        problem.map(|problem| problem.code)
    };

    assert_eq!(refused("indexer.search", "sabnzbd"), Some(CANNOT_FILL));
    assert_eq!(refused("media.serve", "jellyfin"), Some(NOTHING_ASKS));
    assert_eq!(refused("identity.source", "jellyfin"), Some(CANNOT_FILL));
    assert_eq!(
        refused("indexer.search", "plex").map(lemonfiber_error::Code::as_str),
        Some("WIRE-1")
    );
}

/// Which service fills a capability is a setting, so a run with nowhere to keep
/// one cannot make the choice — and says so rather than reporting it made.
#[test]
fn a_run_with_nowhere_to_record_the_choice_refuses_rather_than_pretending() {
    let ctx = crate::test_support::a_context().build();
    let refused = agreed(&ctx, fill("indexer.search", "nzbhydra2"))
        .err()
        .map(|problem| (problem.code, problem.amiss));
    assert_eq!(
        refused,
        Some((CHOICE_UNWRITABLE, crate::error::Amiss::Answering))
    );
}

/// A stack nothing can read answers neither question. Both halves read the
/// manifest first and neither can say anything useful without it — a listing of
/// nothing would read as a stack that wires nothing, and a substitution worked
/// out against nothing would be a choice recorded for a capability no wiring asks
/// for.
#[test]
fn a_stack_that_cannot_be_read_refuses_both_the_listing_and_the_change() {
    let ctx = crate::test_support::a_context()
        .over(crate::test_support::nowhere())
        .build();

    assert!(
        listing(&ctx).is_err(),
        "a stack nothing could read was listed"
    );
    assert!(
        substituting(&ctx, &fill("indexer.search", "nzbhydra2"),).is_err(),
        "a choice was worked out against a stack nothing could read"
    );
}

/// A settings file that cannot be written refuses, rather than reporting a change
/// it did not make.
///
/// The journal entry goes down first on purpose, so a run stopped between the two
/// leaves an entry for a setting that still holds its old value — which unwinds to
/// the value it already has. What must not happen is the opposite: the answer
/// saying the choice was recorded when the file says otherwise.
#[test]
fn a_choice_the_settings_file_will_not_take_is_refused_rather_than_reported_made() {
    let at = dir("unwritable");
    // A directory where the settings file goes: the path exists, so there is
    // somewhere to record the choice, and writing to it cannot succeed.
    let _ = std::fs::create_dir_all(at.join("config").join(".env"));
    let settings = crate::config::Settings {
        env_file: Some(at.join("config").join(".env")),
        stack_dir: Some(at.join("data").join("stack")),
        ..crate::config::Settings::default()
    };
    let ctx = crate::test_support::a_context().settings(settings).build();

    let answered = agreed(&ctx, fill("indexer.search", "nzbhydra2"))
        .map(|report| report.applied)
        .map_err(|problem| problem.severity);

    assert_eq!(answered, Err(crate::error::Severity::Error));
}

/// The read and the verb arrive as one command and come back as two answers.
#[test]
fn the_read_and_the_verb_answer_as_the_two_outcomes_they_are() {
    let (ctx, _at) = ctx("wiring");
    let rehearsing = ctx.rehearsing();
    assert!(matches!(
        wiring(&rehearsing, &Linking::Read),
        Ok(Outcome::Wiring(_))
    ));
    assert!(matches!(
        wiring(
            &rehearsing,
            &Linking::Fill(fill("indexer.search", "nzbhydra2"))
        ),
        Ok(Outcome::Substitution(_))
    ));
}

/// A choice whose record cannot be written is not made: the journal goes down first,
/// and a journal that will not take the entry stops the change it would have recorded.
#[test]
fn a_choice_the_journal_will_not_take_is_not_made() {
    let (ctx, at) = ctx("unjournalled");
    assert!(
        std::fs::create_dir_all(at.join("config").join("journal.jsonl.writing").join("held"))
            .is_ok()
    );

    let answered = agreed(&ctx, fill("indexer.search", "nzbhydra2"));

    assert!(answered.is_err(), "a choice was made with no record of it");
    assert_eq!(recorded(&at), None, "and the settings file holds it anyway");
}

/// What the settings file holds under `key`.
fn held(at: &std::path::Path, key: &str) -> Option<String> {
    crate::config::store::read(&at.join("config").join(".env"))
        .ok()
        .and_then(|file| file.get(key).map(str::to_owned))
}

/// How the listing says one asked-for capability was settled.
fn settled(report: &crate::model::WiringReport, by: &str) -> Option<crate::wiring::Settled> {
    report.wired.iter().find_map(|link| match &link.reaches {
        Reaches::Asked { settled, .. } if link.by == by => Some(settled.clone()),
        _ => None,
    })
}

/// Unanswered, a choice is the reading and nothing more: what it would come to and the
/// name it goes by, with nothing written.
#[test]
fn an_unanswered_choice_is_its_reading_and_writes_nothing() {
    let (ctx, at) = ctx("unanswered");
    let report = substituting(&ctx, &fill("indexer.search", "nzbhydra2")).ok();
    assert_eq!(report.as_ref().map(|report| report.applied), Some(false));
    assert_eq!(
        report.map(|report| report.agreement.split('-').count()),
        Some(4),
        "named part by part"
    );
    assert_eq!(recorded(&at), None);
}

/// A rehearsal answering the offer checks it and still writes nothing.
#[test]
fn a_rehearsal_answering_the_offer_writes_nothing() {
    let (ctx, at) = ctx("rehearsed-answer");
    let report = agreed(&ctx.rehearsing(), fill("indexer.search", "nzbhydra2")).ok();
    assert_eq!(report.map(|report| report.applied), Some(false));
    assert_eq!(recorded(&at), None);
}

/// An answer naming a reading that has since moved is refused, naming what moved, and
/// changes nothing.
#[test]
fn an_answer_to_a_moved_reading_is_refused_naming_what_moved() {
    let (ctx, at) = ctx("moved");
    let reading = substituting(&ctx, &fill("indexer.search", "nzbhydra2")).ok();
    let mut stale: Vec<String> = reading
        .map(|report| report.agreement.split('-').map(str::to_owned).collect())
        .unwrap_or_default();
    if let Some(second) = stale.get_mut(1) {
        *second = "00000000".to_owned();
    }
    let refused = substituting(
        &ctx,
        &Filling {
            agreement: Some(stale.join("-")),
            ..fill("indexer.search", "nzbhydra2")
        },
    )
    .err();
    assert_eq!(
        refused.as_ref().map(|problem| problem.code),
        Some(super::WIRING_MOVED)
    );
    assert_eq!(
        refused.as_ref().map(|problem| problem.amiss),
        Some(crate::agreement::MOVED_AMISS)
    );
    assert!(
        refused.is_some_and(|problem| problem.meaning.contains("what fills it now")
            && !problem.meaning.contains("what asks for it")
            && !problem.meaning.contains("the choice itself")),
        "only the part that moved is named"
    );
    assert_eq!(recorded(&at), None);
}

/// A reason given is recorded with the choice and read back as the choice's own.
#[test]
fn a_reason_is_recorded_with_the_choice_and_read_back_beside_it() {
    let (ctx, at) = ctx("reasoned");
    let said = "hydra searches the private trackers prowlarr cannot";
    let report = agreed(
        &ctx,
        Filling {
            reason: Some(said.to_owned()),
            ..fill("indexer.search", "nzbhydra2")
        },
    )
    .ok();
    assert_eq!(
        report.map(|report| report.substitution.why),
        Some(Some(said.to_owned()))
    );
    assert!(held(&at, crate::wiring::FILLS_WHY_KEY).is_some());
    let listed = listing(&ctx).ok();
    assert!(
        matches!(
            listed.as_ref().and_then(|report| settled(report, "bindery")),
            Some(crate::wiring::Settled::Chosen { whose: crate::wiring::Whose::Operator, why: Some(why), .. })
                if why == said
        ),
        "{listed:?}"
    );
}

/// A choice made with nothing said, over choices that had nothing said either, writes
/// no reasons setting at all.
#[test]
fn a_choice_with_nothing_said_writes_no_reasons() {
    let (ctx, at) = ctx("unreasoned");
    assert!(agreed(&ctx, fill("indexer.search", "nzbhydra2")).is_ok());
    assert_eq!(held(&at, crate::wiring::FILLS_WHY_KEY), None);
}

/// A reason too long to read beside the choice, or running over more than one line,
/// is refused before anything is read or written.
#[test]
fn a_reason_that_cannot_be_recorded_is_refused() {
    let (ctx, at) = ctx("unreasonable");
    for said in [
        "x".repeat(crate::wiring::REASON_MOST + 1),
        "one line\nand another".to_owned(),
        "a \u{1b}[2Jcleared screen".to_owned(),
    ] {
        let refused = substituting(
            &ctx,
            &Filling {
                reason: Some(said),
                ..fill("indexer.search", "nzbhydra2")
            },
        )
        .err();
        assert_eq!(
            refused.as_ref().map(|problem| problem.code),
            Some(super::UNREASONABLE)
        );
        assert_eq!(
            refused.map(|problem| problem.amiss),
            Some(crate::error::Amiss::Asking)
        );
    }
    let longest = "x".repeat(crate::wiring::REASON_MOST);
    assert!(substituting(
        &ctx,
        &Filling {
            reason: Some(longest),
            ..fill("indexer.search", "nzbhydra2")
        },
    )
    .is_ok());
    assert_eq!(recorded(&at), None);
}

/// An answer carried over to another service is not an answer to this reading.
#[test]
fn an_answer_carried_over_to_another_service_names_the_choice_itself() {
    let one = crate::wiring::Substitution {
        capability: "indexer.search".to_owned(),
        was: Some("prowlarr".to_owned()),
        now: "nzbhydra2".to_owned(),
        asked_by: vec!["bindery".to_owned()],
        leaves_unfilled: Vec::new(),
        setting: "indexer.search=nzbhydra2".to_owned(),
        why: None,
    };
    let other = crate::wiring::Substitution {
        now: "jackett".to_owned(),
        ..one.clone()
    };
    assert_eq!(
        crate::agreement::differs(&super::offer(&one), &super::offer(&other), &super::OFFERED),
        vec!["the choice itself"]
    );
}

/// A record of what is installed that cannot be read refuses the choice, because a
/// plugin's service may be the very claimant being chosen between.
#[test]
fn a_choice_over_a_record_that_will_not_read_is_refused() {
    let (ctx, at) = ctx("record-unread");
    assert!(std::fs::write(at.join("config").join("plugins.json"), "{ not json").is_ok());
    let refused = substituting(&ctx, &fill("indexer.search", "nzbhydra2"))
        .err()
        .map(|problem| problem.code.as_str().to_owned());
    assert_eq!(refused.as_deref(), Some("PLUGIN-4"));
}

/// A media server a plugin brought, written into the layout at `at` as an install leaves
/// it: recorded, and its document written, on no network of the stack's.
fn a_plugin_media_server(ctx: &crate::app::Ctx, at: &std::path::Path) -> std::path::PathBuf {
    let api = lemonfiber_manifest::Api {
        kind: lemonfiber_manifest::ApiKind::Jellyfin,
        key_source: lemonfiber_manifest::KeySource::ConfigXml,
        path: Some("/config/config.xml".to_owned()),
        version: None,
    };
    let server =
        crate::test_support::a_placed("server", &["identity.source"], Some(api), Some(8097));
    let installed = crate::test_support::an_installed("serving", vec![server]);
    let mut register = crate::plugin::Register::empty();
    assert!(register.record(installed.clone()).is_ok());
    assert!(
        crate::app::record::keep(crate::app::plugins::kept_at(ctx).as_deref(), &register).is_ok()
    );
    let document = crate::plugin::overlay(&at.join("data").join("stack"), "serving");
    assert!(crate::config::store::write(&document, &crate::plugin::written(&installed)).is_ok());
    document
}

/// What the record says the plugin's media server joins.
fn joined(ctx: &crate::app::Ctx) -> Option<Vec<String>> {
    crate::app::plugins::read(ctx)
        .ok()?
        .holds("serving")?
        .services
        .first()
        .map(|placed| placed.networks.clone())
}

/// Choosing a plugin's media server for the identity the request service asks for
/// rewrites its document and its record onto the networks the stack's is reached over,
/// in the journalled change the choice is; choosing the stack's back takes them off.
#[test]
fn a_choice_moves_the_networks_of_the_plugin_it_settles_in() {
    let (ctx, at) = ctx("rejoined");
    let document = a_plugin_media_server(&ctx, &at);
    let before = std::fs::read_to_string(&document).unwrap_or_default();

    let chosen = agreed(&ctx, fill("identity.source", "server")).ok();
    let written = std::fs::read_to_string(&document).unwrap_or_default();
    let journal = crate::app::targets::layout(&ctx)
        .and_then(|paths| crate::app::recover::journal_at(&paths.journal()).ok())
        .map(|journal| journal.changes().to_vec())
        .unwrap_or_default();

    assert_eq!(chosen.map(|report| report.applied), Some(true));
    assert_eq!(
        joined(&ctx),
        Some(vec!["default".to_owned(), "gate-upstream".to_owned()])
    );
    assert!(written.contains("- gate-upstream"), "{written}");
    assert!(!written.contains("decline-upstream"), "{written}");
    assert!(journal.iter().any(|change| matches!(&change.kind,
        crate::journal::Kind::Rewritten { path, previous, .. }
            if *path == document.display().to_string() && *previous == before)));

    let back = agreed(&ctx, fill("identity.source", "jellyfin")).ok();

    assert_eq!(back.map(|report| report.applied), Some(true));
    assert_eq!(joined(&ctx), Some(Vec::new()));
    assert_eq!(std::fs::read_to_string(&document).ok(), Some(before));
}

/// A choice that moves no plugin's networks writes over nothing but the setting.
#[test]
fn a_choice_that_moves_no_networks_writes_over_nothing() {
    let (ctx, at) = ctx("unmoved");
    let document = a_plugin_media_server(&ctx, &at);
    let before = std::fs::read_to_string(&document).unwrap_or_default();

    let chosen = agreed(&ctx, fill("indexer.search", "nzbhydra2")).ok();
    let journal = crate::app::targets::layout(&ctx)
        .and_then(|paths| crate::app::recover::journal_at(&paths.journal()).ok())
        .map(|journal| journal.changes().to_vec())
        .unwrap_or_default();

    assert_eq!(chosen.map(|report| report.applied), Some(true));
    assert_eq!(std::fs::read_to_string(&document).ok(), Some(before));
    assert!(journal
        .iter()
        .all(|change| !matches!(change.kind, crate::journal::Kind::Rewritten { .. })));
}

/// With no stack directory there is no plugin document to write over, so a choice that
/// moves a plugin's networks writes over nothing but the record of what is installed.
#[test]
fn without_a_stack_directory_no_plugin_document_is_written_over() {
    let (mut ctx, at) = ctx("rejoined-nowhere");
    let document = a_plugin_media_server(&ctx, &at);
    ctx.settings.stack_dir = None;
    let manifest = ctx.stack.checked_manifest(ctx.today()).ok();
    let read = crate::app::plugins::read(&ctx).ok();

    let overwrites = manifest
        .as_ref()
        .zip(read.as_ref())
        .map(|(manifest, register)| {
            crate::app::plugins::writing::rejoined(
                &ctx,
                manifest,
                register,
                &crate::wiring::Chosen::read(Some("identity.source=server")),
            )
        })
        .unwrap_or_default();

    assert!(
        overwrites
            .iter()
            .all(|overwrite| overwrite.path != document),
        "{:?}",
        overwrites.iter().map(|one| &one.path).collect::<Vec<_>>()
    );
}

/// What is installed changing between the moment a choice reads it and the moment it
/// writes is refused, with nothing written: a document or a record written from what was
/// read would put back a plugin that has since gone, or drop one installed since.
#[test]
fn a_choice_over_what_was_installed_since_it_read_writes_nothing() {
    let (ctx, at) = ctx("raced");
    let document = a_plugin_media_server(&ctx, &at);
    let manifest = ctx.stack.checked_manifest(ctx.today()).ok();
    let read = crate::app::plugins::read(&ctx).ok();
    let overwrites = manifest
        .as_ref()
        .zip(read.as_ref())
        .map(|(manifest, register)| {
            crate::app::plugins::writing::rejoined(
                &ctx,
                manifest,
                register,
                &crate::wiring::Chosen::read(Some("identity.source=server")),
            )
        })
        .unwrap_or_default();
    let kept = crate::app::plugins::kept_at(&ctx).unwrap_or_default();
    assert!(std::fs::write(&kept, "{\"installed\":[]}\n").is_ok());
    assert!(std::fs::remove_file(&document).is_ok());

    let written = crate::app::plugins::writing::overwritten(&overwrites);

    assert!(!overwrites.is_empty());
    assert!(written.is_err());
    assert_eq!(
        std::fs::read_to_string(&kept).ok().as_deref(),
        Some("{\"installed\":[]}\n")
    );
    assert!(!document.exists());
}

/// A file the machine will not let a choice write over is refused as unwritten, naming
/// it, rather than reported written.
#[test]
fn a_file_that_will_not_be_written_over_is_said_unwritten() {
    use std::os::unix::fs::PermissionsExt as _;

    let at = dir("unwritable-over");
    let holding = at.join("holding");
    let file = holding.join("serving.yml");
    assert!(std::fs::create_dir_all(&holding).is_ok());
    assert!(std::fs::write(&file, "before\n").is_ok());
    let overwrite = crate::app::plugins::writing::Overwrite {
        path: file.clone(),
        previous: "before\n".to_owned(),
        text: "after\n".to_owned(),
    };
    let locked = std::fs::set_permissions(&holding, std::fs::Permissions::from_mode(0o500))
        .and_then(|()| std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o400)));

    let written = crate::app::plugins::writing::overwritten(&[overwrite]);

    let _ = std::fs::set_permissions(&holding, std::fs::Permissions::from_mode(0o700));
    let _ = std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600));
    assert!(locked.is_ok());
    assert_eq!(
        written
            .err()
            .map(|problem| problem.code.as_str().to_owned())
            .as_deref(),
        Some("WIRE-4")
    );
    assert_eq!(
        std::fs::read_to_string(&file).ok().as_deref(),
        Some("before\n")
    );
}
