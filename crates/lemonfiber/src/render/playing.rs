//! What the media server is playing now, printed.
//!
//! A line per person watching: who, what, on which device, and whether it is paused.
//! Nobody watching and a media server that would not say are told apart, because a
//! quiet house on the day the server is down is the one reading this must not give.

use lemonfiber_core::model::{Playback, PlayingReport};

use super::Lines;

/// What is playing, a line per session.
pub(super) fn playing(report: &PlayingReport) -> Lines {
    let mut lines = Lines::default();
    lines.put(if report.member.is_empty() {
        "What is playing".to_owned()
    } else {
        format!("{} — what is playing", report.member)
    });

    for session in &report.sessions {
        lines.put(format!("  {}", watched(session)));
    }

    if report.sessions.is_empty() && report.available {
        lines.put("  Nothing is playing — which is the answer, not a gap.");
    } else if report.available {
        let count = report.sessions.len();
        lines.spaced(format!(
            "  {count} {} playing",
            if count == 1 { "session" } else { "sessions" }
        ));
    }

    for finding in &report.findings {
        lines.put(format!("  ! {finding}"));
    }
    lines
}

/// One session, as somebody would say it.
fn watched(session: &Playback) -> String {
    let what = match (&session.series, session.season, session.episode) {
        (Some(series), Some(season), Some(episode)) => {
            format!(
                "{series}, season {season} episode {episode}: {}",
                session.title
            )
        }
        (Some(series), _, _) => format!("{series}: {}", session.title),
        (None, _, _) => session.title.clone(),
    };
    let paused = if session.paused { ", paused" } else { "" };
    format!(
        "{} — {what} (on {}{paused})",
        session.member, session.device
    )
}

#[cfg(test)]
mod tests;
