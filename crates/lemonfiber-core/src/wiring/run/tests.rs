use super::{dispatched, listing, substituting, CANNOT_FILL, CHOICE_UNWRITABLE, NOTHING_ASKS};
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
    let filling = Filling {
        capability: "indexer.search".to_owned(),
        service: "nzbhydra2".to_owned(),
    };

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
    let filling = Filling {
        capability: "indexer.search".to_owned(),
        service: "nzbhydra2".to_owned(),
    };

    let report = substituting(&ctx, &filling).ok();

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
        substituting(
            &ctx,
            &Filling {
                capability: capability.to_owned(),
                service: service.to_owned(),
            },
        )
        .err()
        .map(|problem| problem.code)
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
    let refused = substituting(
        &ctx,
        &Filling {
            capability: "indexer.search".to_owned(),
            service: "nzbhydra2".to_owned(),
        },
    )
    .err()
    .map(|problem| problem.code);
    assert_eq!(refused, Some(CHOICE_UNWRITABLE));
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
        substituting(
            &ctx,
            &Filling {
                capability: "indexer.search".to_owned(),
                service: "nzbhydra2".to_owned(),
            },
        )
        .is_err(),
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

    let answered = substituting(
        &ctx,
        &Filling {
            capability: "indexer.search".to_owned(),
            service: "nzbhydra2".to_owned(),
        },
    )
    .map(|report| report.applied)
    .map_err(|problem| problem.severity);

    assert_eq!(answered, Err(crate::error::Severity::Error));
}

/// The read and the verb arrive as one command and come back as two answers.
#[test]
fn the_read_and_the_verb_answer_as_the_two_outcomes_they_are() {
    let (ctx, _at) = ctx("dispatched");
    let rehearsing = ctx.rehearsing();
    assert!(matches!(
        dispatched(&rehearsing, &Linking::Read),
        Ok(Outcome::Wiring(_))
    ));
    assert!(matches!(
        dispatched(
            &rehearsing,
            &Linking::Fill(Filling {
                capability: "indexer.search".to_owned(),
                service: "nzbhydra2".to_owned(),
            })
        ),
        Ok(Outcome::Substituted(_))
    ));
}
