//! Where content is — for whoever runs the stack, and for whoever asked.
//!
//! One of the renderers, its own file so each answer's shape is read on its own.
//! Every one of them builds lines and hands them back; the printer is at the edge.

use lemonfiber_core::model::{
    HouseholdMember, HouseholdReport, MemberRequest, Restriction, StuckReport, TraceReport,
    UnsupportedReport,
};
use lemonfiber_core::ports::service::Unrated;
use lemonfiber_core::trace::{Confidence, Coverage, Outcome as TraceOutcome, HISTORY_HORIZON};
use lemonfiber_core::PRODUCT;

use super::Lines;

/// The exact command that traces one item, printed beneath the line that names it.
///
/// Shared by every surface that leads to a trace so the two cannot drift apart: the term
/// is what the trace searches by, and a link that no longer matches how the trace matches
/// would send an operator to a search that finds nothing.
pub(super) fn trace_link(title: &str) -> String {
    format!("      → {PRODUCT} trace {}", one_argument(title))
}

/// A title written so a shell hands the whole of it to the trace as one argument.
///
/// This line is not a label, it is a command an operator copies and runs, and the title
/// in it came from an indexer or an \*arr. In double quotes a title carrying one closes
/// the quote early, and everything after it is read as further arguments — or, where the
/// title carries `$` or a backtick, as something to expand.
///
/// Single quotes rather than double because inside them a shell reads nothing at all: an
/// apostrophe is the only character with any meaning left, and `'\''` closes the quoting,
/// writes a literal apostrophe and opens it again. Double quotes would still leave `$`,
/// a backtick and — in an interactive shell, which is where this line is pasted — the `!`
/// of `Airplane!` to be dealt with one at a time.
pub(super) fn one_argument(title: &str) -> String {
    format!("'{}'", title.replace('\'', r"'\''"))
}

/// What the household asked for, grouped by whoever asked.
///
/// A member's own words rather than the services': where a request stands, and — for one
/// that has a name to search by — the trace that says why in the services' terms. The
/// deep answer stays where it already lives; this is the way in to it.
pub(super) fn household(report: &HouseholdReport) -> Lines {
    let mut lines = Lines::default();
    for member in &report.members {
        lines.put(format!("{} — {}", member.name, standing(member)));
        for request in &member.requests {
            lines.put(asked_for(request));
            // Only a named request can be traced: the trace searches by title, so a link
            // for one with no name would lead to a search that finds nothing.
            if let Some(title) = &request.title {
                lines.put(trace_link(title));
            }
        }
    }

    if report.members.is_empty() && report.available {
        lines.put("The media server holds no accounts yet.");
    } else if !report.members.is_empty() {
        lines.spaced(counted(report));
    }
    // What happens to what anybody asks for, said once under the list rather than on
    // every line: it is one arrangement for the house, and repeating it per member
    // would read as a per-member setting.
    if let Some(policy) = report.policy {
        lines.put(format!("  {}", allowed(policy, report.allows.as_deref())));
    }
    // What the limits above are, and what they are not — said where anybody carries
    // one, because the reader who most needs it is the parent who set it.
    if let Some(filtering) = &report.filtering {
        lines.put(format!("  {filtering}"));
    }
    // What could not be read, said rather than left to look like an empty household.
    for finding in &report.findings {
        lines.put(format!("  ! {finding}"));
    }
    // Last, because it is the one part of this screen meant to leave it: everything
    // above is about the list, and a block to copy out reads worst with the list's own
    // notes after it.
    for line in to_hand_over(report) {
        lines.put(line);
    }
    lines
}

/// The answer written to one member, where the list is about one member.
///
/// **Nobody in the house has a way to read any of this.** They ask at the request
/// service, which shows no cost, names neither the limit nor the reset when it refuses,
/// and sends a decline with nothing beside it — so the figures that would answer them are
/// gathered here, in front of the one person who is not asking. This is the block that
/// closes that gap the only way this program can: words ready to send, rather than four
/// numbers an operator has to compose into a message.
///
/// Only where the list holds exactly one member. On a whole household it would be the
/// same block per person and the list itself would be lost between them — and a list
/// narrowed to one person is precisely when somebody is about to answer them.
fn to_hand_over(report: &HouseholdReport) -> Vec<String> {
    let [member] = report.members.as_slice() else {
        return Vec::new();
    };
    if member.to_hand_over.is_empty() {
        return Vec::new();
    }
    let mut said = vec![
        String::new(),
        format!(
            "  To hand to {} — none of this is visible where they ask:",
            member.name
        ),
    ];
    said.extend(member.to_hand_over.iter().map(|line| format!("    {line}")));
    said
}

/// One request, as its own line: what it is called, and where it stands.
///
/// A request no service holds yet has no title to print. Naming it by *what it is*
/// keeps the line honest rather than inventing something to call it, and a service
/// reporting a state this build does not know says so rather than guessing at the
/// nearest word.
fn asked_for(request: &MemberRequest) -> String {
    let name = request.title.clone().unwrap_or_else(|| {
        request
            .media
            .clone()
            .map_or_else(|| "something".to_owned(), |media| format!("a {media}"))
    });
    let said = match request.state {
        Some(state) => state.phrase().to_owned(),
        None => "the request service reports a state this build does not know".to_owned(),
    };
    format!("  #{} {name}   {said}{}", request.id, pending(request))
}

/// What is worth saying about a request nobody has ruled on yet, and nothing about
/// one that has been.
///
/// How long it has waited and about what it would take, which are the two things
/// somebody deciding needs and nobody deciding wants. On a request already answered
/// they are noise: the wait is over and the cost is already being paid.
fn pending(request: &MemberRequest) -> String {
    let mut said = Vec::new();
    if let Some(days) = request.waiting_days {
        said.push(format!("waiting {days} day(s)"));
    }
    // Only where it is still a decision. The word is carried on the figure rather than
    // added here, so every surface hedges it the same way.
    if request.waiting_days.is_some() {
        if let Some(estimate) = request.estimate {
            said.push(estimate.reading());
        }
    }
    if said.is_empty() {
        String::new()
    } else {
        format!("   ({})", said.join(", "))
    }
}

/// The line under the list: how many people, how much they asked for, and how many
/// invitations are still out.
///
/// Invitations are counted apart because one nobody has taken up is the entry here an
/// operator might want to do something about today; the rest is context.
fn counted(report: &HouseholdReport) -> String {
    let requests: usize = report
        .members
        .iter()
        .map(|member| member.requests.len())
        .sum();
    let waiting = report
        .members
        .iter()
        .filter(|member| !member.claimed)
        .count();
    let invitations = if waiting > 0 {
        format!(", {waiting} invitation(s) not taken up")
    } else {
        String::new()
    };
    format!(
        "{} member(s), {requests} request(s){invitations}.",
        report.members.len()
    )
}

/// What happens to what the household asks for, and how much of it it may ask.
///
/// The limit is said beside the policy rather than under it, because the two are one
/// arrangement: "everything arrives" and "within five a week" are different promises,
/// and a reader shown only the first has been told the wrong one.
fn allowed(policy: lemonfiber_core::asking::Policy, allows: Option<&str>) -> String {
    match allows {
        Some(allows) => format!("Requests: {} — {allows}.", policy.means()),
        None => format!("Requests: {}.", policy.means()),
    }
}

/// What one member's account says about them, on the line beside their name.
///
/// Access first and activity last, because access is what an operator is deciding about
/// and activity is what tells them whether the decision matters.
///
/// **An unclaimed invitation says so rather than saying "never signed in".** Never being
/// seen is what an unclaimed invitation *is*, so the second reading is true and useless:
/// the operator's next move is to re-send a message, not to wonder why somebody has
/// stopped watching.
fn standing(member: &HouseholdMember) -> String {
    let mut said = Vec::new();
    if member.access.disabled {
        said.push("switched off".to_owned());
    }
    if member.access.administrator {
        said.push("runs the server".to_owned());
    }
    said.push(if member.access.every_library {
        "can watch everything".to_owned()
    } else if member.access.libraries.is_empty() {
        // Not "everything minus nothing": the server was asked for the libraries this
        // account may open and named none of them.
        "can watch nothing".to_owned()
    } else {
        format!("can watch {}", member.access.libraries.join(", "))
    });
    // Said in the words the limit is chosen in, from the place those words live,
    // because a household list naming it differently from the invitation that set it
    // is two surfaces disagreeing about one setting. The media server keeps this as a
    // number, and what is said is a reading of that number — with the certificates
    // this household's own server names on either side of it, since a number alone
    // says nothing about what it actually holds back here.
    if let Some(limit) = member.access.age_limit {
        said.push(member.access.rated.as_ref().map_or_else(
            || lemonfiber_core::age_limit::reading(Some(limit)),
            |rated| lemonfiber_core::rating::said(limit, rated),
        ));
    }
    // Said either way on somebody narrowed, because an unexplained absence is what
    // this answers: a member missing half the library is either this or a defect, and
    // silence does not tell an operator which. Nobody unnarrowed is missing anything,
    // so the line stays off theirs.
    if member.access.restriction != Restriction::Unrestricted {
        said.push(match member.access.unrated {
            Unrated::HeldBack => "nothing unrated".to_owned(),
            Unrated::LetThrough => "including what has no rating".to_owned(),
        });
    }
    // The gap the setting exists to close, on the member's own line rather than only
    // in a finding at the foot: half a limit looks exactly like a whole one.
    if member.access.restriction.disagrees() {
        said.push(member.access.restriction.phrase().to_owned());
    }
    // What they may *ask for*, beside what they may watch, because the two are one
    // question about the same person and read apart they look like two people. Absent
    // where the request service holds no account for them, which is an invitation
    // nobody has used rather than somebody nothing limits.
    if let Some(asking) = &member.asking {
        said.push(asking.standing.phrase().to_owned());
    }
    said.push(if member.claimed {
        member
            .last_seen
            .as_deref()
            .and_then(|at| at.split('T').next())
            .map_or_else(
                // Not "never signed in": the account has been claimed, which on this
                // media server is done *by* signing in. A date missing here is the
                // server not reporting one, not somebody who never arrived.
                || "no sign-in recorded".to_owned(),
                |day| format!("last seen {day}"),
            )
    } else {
        "invited, nobody has set a password yet".to_owned()
    });
    said.join(" · ")
}

/// How much of a series is actually here, season by season — the answer the single
/// furthest stage cannot give, since a show is "imported" the moment one episode lands.
///
/// A complete season is one line; an incomplete one names each episode still outstanding
/// and what it is waiting on, because that is the part an operator can act on. Episodes
/// nobody asked for are counted apart from the totals and said so plainly, so a season of
/// specials never reads as a fault to go and chase.
pub(super) fn seasons(coverage: &Coverage) -> Lines {
    let mut lines = Lines::default();
    // Nothing asked for is not "none of nothing here" — with no denominator the counts
    // say nothing, and the honest reading is that no episode is being maintained.
    if coverage.wanted == 0 {
        lines.put(format!(
            "  no episode(s) asked for — {} not monitored, none on disk",
            coverage.unmonitored
        ));
        return lines;
    }
    lines.put(format!(
        "  {} of {} episode(s) here",
        coverage.have, coverage.wanted
    ));
    for season in &coverage.seasons {
        // Season zero is where a service files specials, which is not a season anyone
        // names that way.
        let name = if season.season == 0 {
            "specials".to_owned()
        } else {
            format!("season {}", season.season)
        };
        if season.wanted == 0 {
            lines.put(format!(
                "      {name}   {} not asked for",
                season.unmonitored
            ));
            continue;
        }
        let complete = if season.complete() { "   complete" } else { "" };
        lines.put(format!(
            "      {name}   {} of {}{complete}",
            season.have, season.wanted
        ));
        if season.unmonitored > 0 {
            lines.put(format!(
                "          ({} more not asked for)",
                season.unmonitored
            ));
        }
        for part in &season.outstanding {
            let waiting = part
                .stage
                .stall()
                .map_or_else(|| part.stage.label().to_owned(), str::to_owned);
            lines.put(format!(
                "          S{:02}E{:02}   {waiting}",
                part.season, part.number
            ));
        }
    }
    lines
}

/// Where one item is in the pipeline: the item, each stage it reached with the service
/// and time, and — where it plainly stopped — why.
pub(super) fn trace(report: &TraceReport) -> Lines {
    let mut lines = Lines::default();
    lines.put(report.item.clone());
    if !report.matched {
        // No monitored item matched — nobody asked for it.
        if let Some(reason) = &report.stall {
            lines.put(format!("  {reason}"));
        }
        return lines;
    }
    for stage in &report.stages {
        let label = stage.stage.label();
        match &stage.at {
            Some(at) => lines.put(format!("  ✓ {label}   {}   {at}", stage.service)),
            None => lines.put(format!("  ✓ {label}   {}", stage.service)),
        }
    }
    if let Some(reason) = &report.stall {
        lines.put(format!("  ✗ stopped: {reason}"));
    }
    // The history of what was tried, shown when it reveals a pattern the linear stages
    // cannot: a download that failed, a file removed, or the same release grabbed more
    // than once. A single clean grab-and-import is already told by the stages above, so it
    // is not repeated here.
    let grabs = report
        .history
        .iter()
        .filter(|moment| moment.outcome == TraceOutcome::Grabbed)
        .count();
    let troubled = report.history.iter().any(|moment| {
        matches!(
            moment.outcome,
            TraceOutcome::DownloadFailed | TraceOutcome::Removed
        )
    });
    if grabs > 1 || troubled {
        lines.put("  history:");
        for moment in &report.history {
            lines.put(format!("      {}   {}", moment.outcome.phrase(), moment.at));
        }
    }
    if let Some(coverage) = &report.coverage {
        lines.extend(seasons(coverage));
    }
    // Things worth the operator's attention that are not a point on the pipeline — a
    // service disagreement, or a detail that could not be read and so is reported as
    // unavailable rather than inferred. Each finding's own words say which.
    for finding in &report.findings {
        lines.put(format!("  ! {finding}"));
    }
    // A trace joined to the library by title alone may not be the item asked for; saying
    // so is the honest thing — better a marked guess than one presented as fact.
    if report.confidence == Confidence::Uncertain {
        lines.put("  ~ matched to the library by title — this may not be the item you meant");
    }
    // The history read is bounded; stating the horizon keeps "nothing earlier" honest —
    // an event older than this window is not read, not proof that nothing happened.
    lines.put(format!(
        "  · reflects the most recent {HISTORY_HORIZON} history events per service"
    ));
    lines
}

/// The items whose downloads are stuck, each named so it links straight to its own
/// trace — the landing point for "N stuck", turning a count into a list the operator can
/// act on one item at a time.
pub(super) fn stuck(report: &StuckReport) -> Lines {
    let mut lines = Lines::default();
    if report.items.is_empty() {
        lines.put("Nothing is stuck — every download is progressing.");
    } else {
        lines.put(format!(
            "{} item(s) stuck — trace any one to see why:",
            report.items.len()
        ));
        for item in &report.items {
            lines.put(format!(
                "  ✗ {}   {}   stuck at {}",
                item.title,
                item.service,
                item.stage.label()
            ));
            lines.put(trace_link(&item.title));
        }
    }
    // A queue that could not be read leaves the list possibly short; saying so keeps it
    // from being read as "nothing else is stuck", the same honesty a trace keeps.
    if report.incomplete {
        lines.spaced("An *arr's queue could not be read, so this list may be incomplete.");
    }
    lines.extend(unreadable(&report.unsupported));
    lines
}

/// The queues that were never asked, under the ones that were.
///
/// A different sentence from the unreadable-queue one above and deliberately so: that
/// is a service that answered badly, and this is a service lemonfiber cannot speak to
/// at all. An operator told only the first would go looking for a service that was
/// down, and find it running.
fn unreadable(unsupported: &[UnsupportedReport]) -> Lines {
    let mut lines = Lines::default();
    if unsupported.is_empty() {
        return lines;
    }
    lines.spaced(
        "These hold queues lemonfiber cannot read at all, so nothing of theirs is in the \
         list above:",
    );
    for one in unsupported {
        lines.put(format!("  {} — {}", one.what, one.because));
    }
    lines
}

#[cfg(test)]
mod tests;
