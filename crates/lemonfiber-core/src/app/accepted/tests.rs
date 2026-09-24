use super::{load, save};
use crate::doctor::acknowledged::Accepted;
use crate::test_support::a_context;

/// Where a test's scratch record lives. Naming it does not touch it.
fn scratch(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("lemonfiber-accepted-{}-{name}", std::process::id()))
}

/// A context whose environment file is in an emptied scratch directory.
fn ctx_at(name: &str) -> crate::app::Ctx {
    let dir = scratch(name);
    let _ = std::fs::remove_dir_all(&dir);
    ctx_with(Some(dir.join(".env")))
}

/// A context with the given environment file, or none at all.
fn ctx_with(env_file: Option<std::path::PathBuf>) -> crate::app::Ctx {
    a_context()
        .runner(std::sync::Arc::new(crate::test_support::Scripted(Ok(
            crate::test_support::spoke(""),
        ))))
        .settings(crate::config::Settings {
            env_file,
            ..crate::config::Settings::default()
        })
        .build()
}

#[test]
fn a_choice_answered_once_stays_answered() {
    // The whole point: a settled question is not put again next run.
    let ctx = ctx_at("round-trip");
    let mut accepted = Accepted::new();
    accepted.accept("vpn.tunnel");
    assert!(save(&ctx, &accepted).is_ok());

    let read_back = load(&ctx);
    assert!(read_back.has("vpn.tunnel"));
    assert!(!read_back.has("vpn.port-forward"), "and only that one");
}

#[test]
fn a_machine_where_nothing_was_answered_starts_with_nothing() {
    assert_eq!(load(&ctx_at("fresh")), Accepted::new());
}

#[test]
fn a_record_that_will_not_parse_puts_the_question_again() {
    // Tiresome, and the safe direction: the alternative is treating an
    // unreadable file as blanket consent.
    let ctx = ctx_at("corrupt");
    let mut accepted = Accepted::new();
    accepted.accept("vpn.tunnel");
    assert!(save(&ctx, &accepted).is_ok());
    let written = scratch("corrupt").join("accepted.json");
    assert!(
        written.exists(),
        "the record was written in the first place"
    );
    assert!(
        crate::config::store::write(&written, "not json at all").is_ok(),
        "and is then replaced with something unparsable"
    );
    assert_eq!(load(&ctx), Accepted::new());
}

#[test]
fn a_record_that_cannot_be_written_is_reported_rather_than_swallowed() {
    // Somewhere to keep it and still no way to write it: a directory sits
    // where the file must go. Telling the operator a choice was settled when
    // it was not is the failure worth avoiding here.
    let ctx = ctx_at("blocked");
    let blocked = scratch("blocked").join("accepted.json");
    assert!(
        std::fs::create_dir_all(&blocked).is_ok(),
        "the blocking directory"
    );
    assert!(save(&ctx, &Accepted::new()).is_err());
}

#[test]
fn a_record_with_nowhere_to_go_is_reported_rather_than_swallowed() {
    let ctx = ctx_with(None);
    assert!(save(&ctx, &Accepted::new()).is_err());
    assert_eq!(load(&ctx), Accepted::new());
}

/// A report carrying one finding.
fn reporting(finding: crate::doctor::Finding) -> crate::model::DoctorReport {
    crate::model::DoctorReport {
        overall: crate::doctor::Overall::Degraded,
        findings: vec![finding],
    }
}

/// A warning about a choice, on the given check.
fn warning(check: &str) -> crate::doctor::Finding {
    crate::doctor::Finding::in_category(
        crate::doctor::Category::Vpn,
        check,
        "Torrent traffic is contained",
        crate::doctor::Verdict::Warn(crate::error::Problem::new(
            crate::error::codes::vpn::NO_TUNNEL,
            crate::error::Severity::Warning,
            "Torrent traffic is not contained by a VPN",
            "It leaves under this connection's own address.",
            crate::error::Remedy::new("Put the client behind a VPN container"),
        )),
    )
}

#[test]
fn answering_a_warning_settles_it_in_this_run_and_the_next() {
    let ctx = ctx_at("acknowledge");
    let mut report = reporting(warning("vpn.unprotected"));
    // A passing check alongside it, because an answer to one finding must not
    // reach into the rest of the run.
    report.findings.push(crate::doctor::Finding::in_category(
        crate::doctor::Category::Vpn,
        "vpn.egress-match",
        "The tunnel",
        crate::doctor::Verdict::Pass { note: None },
    ));
    let answered = super::acknowledge(&ctx, Some("vpn.unprotected"), report);
    let states: Vec<crate::error::State> = answered
        .as_ref()
        .map(|report| {
            report
                .findings
                .iter()
                .filter_map(|finding| match &finding.verdict {
                    crate::doctor::Verdict::Warn(problem)
                    | crate::doctor::Verdict::Fail(problem) => Some(problem.state),
                    crate::doctor::Verdict::Pass { .. }
                    | crate::doctor::Verdict::Unverified { .. }
                    | crate::doctor::Verdict::Skipped { .. } => None,
                })
                .collect()
        })
        .unwrap_or_default();
    // Settled in the run that answered it, rather than only from the next one:
    // an operator who answers and sees the warning again assumes it did nothing.
    assert_eq!(states, vec![crate::error::State::Suppressed]);
    assert!(load(&ctx).has("vpn.unprotected"), "and it is written down");
}

#[test]
fn answering_something_nothing_warns_about_is_refused_rather_than_recorded() {
    // A typo recorded silently leaves the operator believing a question is
    // settled while the tool goes on asking it.
    //
    // Built from the correct name with its last character dropped rather than
    // written out: a misspelling in a literal is one the spell-check gate
    // reads as a mistake in the source, which it cannot tell from a real one.
    let ctx = ctx_at("typo");
    let mistyped = "vpn.unprotected".trim_end_matches('d');
    let refused = super::acknowledge(&ctx, Some(mistyped), reporting(warning("vpn.unprotected")));
    assert!(refused.is_err());
    assert!(!load(&ctx).has(mistyped), "nothing was written");
    let detail = refused.err().and_then(|problem| {
        problem
            .remedies
            .first()
            .and_then(|remedy| remedy.detail.clone())
    });
    assert_eq!(
        detail,
        Some("lemonfiber doctor --accept vpn.unprotected".to_owned()),
        "and it names what could be answered instead"
    );
}

#[test]
fn a_failure_cannot_be_answered_at_all() {
    // The most damaging thing this could do, and the reason the check is on the
    // verdict rather than on the check name: a fault is not a choice.
    let ctx = ctx_at("failure");
    let failing = crate::doctor::Finding::in_category(
        crate::doctor::Category::Vpn,
        "vpn.egress-match",
        "The tunnel",
        crate::doctor::Verdict::Fail(crate::error::Problem::new(
            crate::error::codes::vpn::LEAKING,
            crate::error::Severity::Critical,
            "Traffic is leaving outside the tunnel",
            "Every torrent is visible under this machine's own address.",
            crate::error::Remedy::new("Stop the download client"),
        )),
    );
    assert!(super::acknowledge(&ctx, Some("vpn.egress-match"), reporting(failing)).is_err());
    assert!(!load(&ctx).has("vpn.egress-match"));
}

#[test]
fn a_rehearsed_answer_settles_the_warning_in_the_report_and_nowhere_else() {
    // The two halves together: the operator sees exactly the report a real answer
    // would give them, and the next run still puts the question.
    let ctx = ctx_at("rehearsed").rehearsing();
    let mut report = reporting(warning("vpn.unprotected"));
    // A passing check alongside it, for the reason the answered case carries one:
    // an answer to one finding must not reach into the rest of the run, and a
    // rehearsal that quietened a check nobody asked about would be describing a
    // report the real answer does not produce.
    report.findings.push(crate::doctor::Finding::in_category(
        crate::doctor::Category::Vpn,
        "vpn.egress-match",
        "The tunnel",
        crate::doctor::Verdict::Pass { note: None },
    ));
    let answered = super::acknowledge(&ctx, Some("vpn.unprotected"), report);
    let states: Vec<crate::error::State> = answered
        .as_ref()
        .map(|report| {
            report
                .findings
                .iter()
                .filter_map(|finding| match &finding.verdict {
                    crate::doctor::Verdict::Warn(problem)
                    | crate::doctor::Verdict::Fail(problem) => Some(problem.state),
                    crate::doctor::Verdict::Pass { .. }
                    | crate::doctor::Verdict::Unverified { .. }
                    | crate::doctor::Verdict::Skipped { .. } => None,
                })
                .collect()
        })
        .unwrap_or_default();
    assert_eq!(states, vec![crate::error::State::Suppressed]);
    assert!(
        !load(&ctx).has("vpn.unprotected"),
        "a rehearsal that wrote the answer down would have answered for them"
    );
}

#[test]
fn a_run_that_answers_nothing_is_left_exactly_as_it_came() {
    let ctx = ctx_at("untouched");
    let report = reporting(warning("vpn.unprotected"));
    let same = super::acknowledge(&ctx, None, report.clone());
    assert_eq!(same.ok(), Some(report));
}

#[test]
fn a_refusal_with_nothing_answerable_says_so_rather_than_offering_an_empty_list() {
    let ctx = ctx_at("nothing-answerable");
    let quiet = crate::model::DoctorReport {
        overall: crate::doctor::Overall::Healthy,
        findings: Vec::new(),
    };
    let refused = super::acknowledge(&ctx, Some("vpn.unprotected"), quiet);
    assert_eq!(
        refused.err().and_then(|problem| problem
            .remedies
            .first()
            .and_then(|remedy| remedy.detail.clone())),
        None,
        "an empty list would read as though nothing could ever be answered"
    );
}
