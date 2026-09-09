//! What lemonfiber has already changed, on a terminal.
//!
//! Newest first, because the change somebody is asking about is nearly always the one
//! they just made. Each line says when, what it did, and — where it is not simply
//! reversible — how far it could be put back and why.
//!
//! Whether a change can be put back is [`lemonfiber_core::rollback`]'s judgement, not
//! this file's. What is here is the wording of it.

use lemonfiber_core::model::{ChangeReport, HistoryReport};

use super::Lines;

/// Everything lemonfiber changed, newest first.
pub(super) fn history(report: &HistoryReport) -> Lines {
    let mut lines = Lines::default();
    if report.changes.is_empty() {
        lines.put("nothing has been changed yet");
        lines.put(String::new());
        lines.put(format!("on record: {}", report.horizon));
        return lines;
    }

    lines.put(format!("{} changes, newest first", report.changes.len()));
    lines.put(String::new());
    for change in &report.changes {
        lines.extend(one(change));
    }
    lines.put(format!("on record: {}", report.horizon));
    lines
}

/// One change, and what putting it back would come to.
fn one(change: &ChangeReport) -> Lines {
    let mut lines = Lines::default();
    lines.put(format!(
        "{} — {} ({})",
        change.at, change.did, change.operation
    ));
    lines.put(format!("  on {}", change.target));
    lines.put(format!("  putting it back: {}", putting(&change.reversal)));
    if change.alongside > 1 {
        // The operation is the unit that goes back, so what else would go with it is
        // said on the line rather than counted off the list.
        lines.put(format!(
            "  part of {} changes made together",
            change.alongside
        ));
    }
    if let Some(because) = &change.because {
        lines.put(format!("  because {because}"));
    }
    if let Some(instead) = &change.instead {
        lines.put(format!("  instead {instead}"));
    }
    lines.put(String::new());
    lines
}

/// How far a change could be put back, in the operator's terms rather than the name.
fn putting(reversal: &str) -> &'static str {
    match reversal {
        "whole" => "in full",
        "partial" => "in part",
        _ => "not by lemonfiber",
    }
}

#[cfg(test)]
mod tests {
    use lemonfiber_core::model::{ChangeReport, HistoryReport};

    use super::history;

    /// One change on the record, with the fields a test is not varying already filled.
    fn change(did: &str, reversal: &str) -> ChangeReport {
        ChangeReport {
            at: "2024-03-01T10:00:00Z".to_owned(),
            operation: "reconfigure".to_owned(),
            target: ".env".to_owned(),
            did: did.to_owned(),
            reversal: reversal.to_owned(),
            because: None,
            instead: None,
            alongside: 1,
        }
    }

    /// The record these changes make up.
    fn record(changes: Vec<ChangeReport>) -> HistoryReport {
        HistoryReport {
            changes,
            horizon: "every change since this machine was set up".to_owned(),
        }
    }

    /// A machine that has changed nothing still says how far back the record goes.
    /// An empty record and a trimmed one are the same list otherwise, and only one
    /// of them means something is missing.
    #[test]
    fn a_machine_that_changed_nothing_says_so_and_still_states_its_horizon() {
        let said = history(&record(Vec::new())).text();
        assert!(said.starts_with("nothing has been changed yet"), "{said}");
        assert!(
            said.contains("on record: every change since this machine was set up"),
            "{said}"
        );
    }

    /// Newest first, because the change somebody is asking about is nearly always the
    /// one they just made — and each line says when it was made, what it did and what
    /// it was made to.
    #[test]
    fn the_record_counts_its_changes_and_says_when_each_was_made_and_to_what() {
        let said = history(&record(vec![
            change("set TZ to UTC", "whole"),
            change("made /srv/media", "whole"),
        ]))
        .text();

        assert!(said.starts_with("2 changes, newest first"), "{said}");
        assert!(
            said.contains("2024-03-01T10:00:00Z — set TZ to UTC (reconfigure)"),
            "{said}"
        );
        assert!(said.contains("  on .env"), "{said}");
        assert!(said.contains("made /srv/media"), "{said}");
        assert!(said.ends_with("on record: every change since this machine was set up"));
    }

    /// The judgement's own names are `whole`, `partial` and `none`, and none of them
    /// tells an operator what putting a change back would actually come to. What is
    /// on the line is the meaning rather than the name.
    #[test]
    fn how_far_a_change_could_be_put_back_is_said_in_what_it_would_mean() {
        let said = history(&record(vec![
            change("set TZ to UTC", "whole"),
            change("changed DATA_ROOT from /srv/old to /srv/new", "partial"),
            change("added a downloadclient", "none"),
        ]))
        .text();

        assert!(said.contains("putting it back: in full"), "{said}");
        assert!(said.contains("putting it back: in part"), "{said}");
        assert!(
            said.contains("putting it back: not by lemonfiber"),
            "{said}"
        );
    }

    /// The operation is the unit that goes back, so what a single line would take with
    /// it is said on the line. A change made on its own says nothing about that rather
    /// than announcing that it took one thing with it.
    #[test]
    fn a_change_made_with_others_says_so_and_one_made_alone_says_nothing_about_it() {
        let together = history(&record(vec![ChangeReport {
            alongside: 4,
            ..change("set PUID to 1000", "whole")
        }]))
        .text();
        assert!(
            together.contains("part of 4 changes made together"),
            "{together}"
        );

        let alone = history(&record(vec![change("set PUID to 1000", "whole")])).text();
        assert!(!alone.contains("made together"), "{alone}");
    }

    /// A refusal without its reason is a dead end. The reason is what tells an operator
    /// whether to reach for a backup, and what to do instead is what they came for.
    #[test]
    fn a_change_that_cannot_go_back_carries_the_reason_and_what_to_do_instead() {
        let said = history(&record(vec![ChangeReport {
            because: Some("TZ now holds America/New_York".to_owned()),
            instead: Some("set it yourself if the older value is the one you want".to_owned()),
            ..change("set TZ to UTC", "none")
        }]))
        .text();

        assert!(
            said.contains("  because TZ now holds America/New_York"),
            "{said}"
        );
        assert!(
            said.contains("  instead set it yourself if the older value is the one you want"),
            "{said}"
        );
    }

    /// Through the dispatcher rather than by calling this module, because what the
    /// terminal draws is what the printer chose for the outcome — an arm nothing
    /// reaches renders nowhere however good the renderer under it is.
    #[test]
    fn the_printer_reaches_this_renderer_for_this_outcome() {
        let report = record(vec![change("added a downloadclient", "whole")]);
        let drawn = crate::render::shaped(&lemonfiber_core::app::Outcome::History(report)).text();
        assert!(drawn.contains("1 changes, newest first"), "{drawn}");
        assert!(drawn.contains("added a downloadclient"), "{drawn}");
        assert!(drawn.contains("putting it back: in full"), "{drawn}");
    }
}
