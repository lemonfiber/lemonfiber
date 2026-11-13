//! A title, what a member was part-way through, a grant and a player's progress, printed.
//!
//! Each says where it is served at the guarded front door, because at a terminal that is
//! what an operator checks: whether a title a member cannot play is unlocated, and why.

use lemonfiber_core::model::{
    ChannelsReport, GrantReport, Located, PartWayReport, PlayReport, TitleReport, TunedReport,
    WatchedReport,
};

use super::held::titled;
use super::news::moment;
use super::{qr, Lines};

/// One title, its details and where it is served.
pub(super) fn title(report: &TitleReport) -> Lines {
    let mut lines = Lines::default();
    let Some(title) = &report.title else {
        lines.put("No such title on this shelf.");
        return lines;
    };
    lines.put(titled(&title.held));
    if let Some(overview) = &title.overview {
        lines.put(format!("  {overview}"));
    }
    let facts: Vec<String> = [
        title.minutes.map(|minutes| format!("{minutes} minutes")),
        title.certificate.clone(),
        title.released.clone(),
        (!title.genres.is_empty()).then(|| title.genres.join(", ")),
    ]
    .into_iter()
    .flatten()
    .collect();
    if !facts.is_empty() {
        lines.put(format!("  {}", facts.join(" · ")));
    }
    served(&mut lines, &title.held.at);
    for season in &title.seasons {
        lines.spaced(format!("  {}", season.name));
        for episode in &season.episodes {
            let number = episode
                .number
                .map_or_else(String::new, |number| format!("{number}. "));
            lines.put(format!("    {number}{}", episode.held.title));
        }
    }
    lines
}

/// What a member was part-way through, a line each.
pub(super) fn part_way(report: &PartWayReport) -> Lines {
    let mut lines = Lines::default();
    lines.put(format!("{} — part-way through", report.member));
    for one in &report.part_way {
        let of = one
            .length
            .map_or_else(String::new, |length| format!(" of {}", clock(length)));
        lines.put(format!(
            "  {} — at {}{of}",
            titled(&one.held),
            clock(one.position)
        ));
    }
    if report.part_way.is_empty() && report.available {
        lines.put("  Nothing part-way through — which is the answer, not a gap.");
    }
    for finding in &report.findings {
        lines.put(format!("  ! {finding}"));
    }
    lines
}

/// A grant, and how long it lasts.
///
/// Never the token: what is printed for a person stays on a screen and in a scrollback,
/// so the token goes to a caller that asked for `--json` and to nobody else.
pub(super) fn grant(report: &GrantReport) -> Lines {
    let mut lines = Lines::default();
    lines.put(if report.granted {
        format!(
            "The device now plays as {}, until {} unless it is used before then. Its token \
             is answered under --json alone, and nothing keeps it.",
            report.member, report.lasts_until
        )
    } else {
        format!(
            "A device would play as {}, until {} unless it is used before then.",
            report.member, report.lasts_until
        )
    });
    lines
}

/// How far a member got, as the media server now holds it.
pub(super) fn watched(report: &WatchedReport) -> Lines {
    let mut lines = Lines::default();
    lines.put(if report.ended {
        "Recorded as finished.".to_owned()
    } else {
        format!("Recorded at {}.", clock(report.position))
    });
    lines
}

/// The channels a member may watch, a line each with what is on now.
pub(super) fn channels(report: &ChannelsReport) -> Lines {
    let mut lines = Lines::default();
    lines.put(format!("{} — live TV", report.member));
    for channel in &report.channels {
        let number = channel
            .number
            .as_ref()
            .map_or_else(String::new, |number| format!("{number} "));
        let now = channel.now.as_ref().map_or_else(String::new, |now| {
            let certificate = now
                .certificate
                .as_ref()
                .map_or_else(String::new, |certificate| format!(" ({certificate})"));
            format!(" — {}{certificate}", now.title)
        });
        lines.put(format!("  {number}{}{now}  [{}]", channel.name, channel.id));
    }
    if report.channels.is_empty() {
        lines.put("  No channel this member may watch now — which is the answer, not a gap.");
    }
    lines
}

/// A live stream opened, kept or closed, and where it is served, each also as a code.
pub(super) fn tuned(report: &TunedReport) -> Lines {
    let mut lines = Lines::default();
    if let Some(why) = &report.unlocated {
        lines.put(format!("! {why}"));
        return lines;
    }
    let (Some(stream), Some(lapses)) = (&report.stream, report.lapses) else {
        lines.put(if report.rehearsed {
            format!(
                "Channel {} for {}: rehearsed, so nothing was opened, kept or closed.",
                report.channel, report.member
            )
        } else {
            format!(
                "Channel {} is closed for {}.",
                report.channel, report.member
            )
        });
        return lines;
    };
    lines.put(format!(
        "Channel {} is open for {} as {stream}, on passes lapsing at {}.",
        report.channel,
        report.member,
        moment(lapses)
    ));
    if let Some(fingerprint) = &report.fingerprint {
        lines.put(format!("  the door presents {fingerprint}"));
    }
    for (label, at) in [
        ("As the tuner sends it:", &report.direct),
        ("Transcoded:", &report.transcoded),
    ] {
        if let Some(at) = at {
            lines.spaced(label);
            lines.put(format!("  {at}"));
            qr::labelled(&mut lines, "The same, for a camera:", at);
        }
    }
    lines
}

/// Where a title plays in a browser, also as a code.
pub(super) fn play(report: &PlayReport) -> Lines {
    let mut lines = Lines::default();
    if let Some(why) = &report.unlocated {
        lines.put(format!("! {why}"));
        return lines;
    }
    let (Some(at), Some(lapses)) = (&report.stream_from, report.lapses) else {
        lines.put(format!(
            "{} would play in a browser as {}.",
            report.id, report.member
        ));
        return lines;
    };
    lines.put(format!(
        "{} plays in a browser as {}, on a pass lapsing at {}:",
        report.id,
        report.member,
        moment(lapses)
    ));
    lines.put(format!("  {at}"));
    qr::labelled(&mut lines, "The same, for a camera:", at);
    lines
}

/// Where a title is served, or why it is not.
fn served(lines: &mut Lines, at: &Located) {
    if let Some(why) = &at.unlocated {
        lines.put(format!("  ! {why}"));
        return;
    }
    if let Some(stream) = &at.stream_from {
        lines.put(format!("  plays from {stream}"));
    }
    if let Some(door) = &at.door {
        lines.put(format!("  the door presents {}", door.fingerprint));
    }
}

/// Seconds as hours, minutes and seconds.
fn clock(seconds: u64) -> String {
    let (hours, minutes, seconds) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

#[cfg(test)]
mod tests;
