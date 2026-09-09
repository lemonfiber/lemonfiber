//! What a proposed change comes to on this machine, said before it is written.
//!
//! One of the renderers, its own file so each answer's shape is read on its own.
//! Every one of them builds lines and hands them back; the printer is at the edge.
//!
//! The order is the order somebody weighing the change reads it in: where the
//! library would land, what is still coming down, what was edited underneath, then
//! what the change opens, stops and keeps. The verdict and its refusal are said by
//! the settings renderer above, once, for every stance alike.

use lemonfiber_core::reconfigure::Findings;

use super::Lines;

/// What was found about this change, or nothing where nothing was.
pub(super) fn found(findings: &Findings) -> Lines {
    let mut lines = Lines::default();
    if !findings.any() {
        return lines;
    }
    lines.extend(library(findings));
    lines.extend(active(findings));
    lines.extend(edited(findings));
    lines.extend(opened(findings));
    lines.extend(closed(findings));
    lines
}

/// The library paths a move would land on, and where each of them would land.
fn library(findings: &Findings) -> Lines {
    let mut lines = Lines::default();
    if findings.library.is_empty() {
        return lines;
    }
    lines.spaced("Where the services file now:");
    for path in &findings.library {
        let mark = if path.carried { "kept" } else { "lost" };
        lines.put(format!("  {mark}  {} — {}", path.path, path.because));
    }
    lines
}

/// What is still coming down that this change would interrupt.
fn active(findings: &Findings) -> Lines {
    let mut lines = Lines::default();
    if findings.active.is_empty() {
        return lines;
    }
    lines.spaced("Still coming down:");
    for download in &findings.active {
        lines.put(format!(
            "  {}% {} ({})",
            download.progress, download.name, download.protocol
        ));
    }
    lines
}

/// The hand-edit found under this setting, both sides of it.
fn edited(findings: &Findings) -> Lines {
    let mut lines = Lines::default();
    let Some(edit) = &findings.edited else {
        return lines;
    };
    lines.spaced("Changed outside lemonfiber:");
    lines.put(format!("  lemonfiber wrote  {}", edit.wrote));
    lines.put(format!("  the file holds    {}", edit.found));
    lines
}

/// What this change newly asks the operator for.
fn opened(findings: &Findings) -> Lines {
    let mut lines = Lines::default();
    if findings.opens.is_empty() {
        return lines;
    }
    lines.spaced("This opens:");
    for open in &findings.opens {
        lines.put(format!("  {} — {}", open.what, open.because));
    }
    lines
}

/// What this change stops, and what it leaves exactly as it is.
fn closed(findings: &Findings) -> Lines {
    let mut lines = Lines::default();
    if !findings.stops.is_empty() {
        lines.spaced(format!("This stops: {}", findings.stops.join(", ")));
    }
    if !findings.keeps.is_empty() {
        lines.spaced("This keeps:");
        for kept in &findings.keeps {
            lines.put(format!("  {kept}"));
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use lemonfiber_core::reconfigure::{Active, Edited, Findings, LibraryPath, Opening};

    use super::found;

    #[test]
    fn a_change_that_came_to_nothing_says_nothing() {
        // Not empty headings over empty lists: a report full of those about a
        // timezone teaches the operator to skip the one that matters.
        assert_eq!(found(&Findings::default()).text(), "");
    }

    #[test]
    fn a_move_that_would_lose_the_library_marks_the_path_it_would_lose() {
        let findings = Findings {
            library: vec![LibraryPath {
                service: "Sonarr".to_owned(),
                path: "/data/media/tv".to_owned(),
                host: Some("/srv/new/media/tv".to_owned()),
                carried: false,
                because: "/srv/new/media/tv is not there".to_owned(),
            }],
            ..Findings::default()
        };
        let text = found(&findings).text();
        assert!(text.contains("lost  /data/media/tv"), "{text}");
        assert!(text.contains("is not there"), "{text}");
        assert!(findings.any());
    }

    #[test]
    fn a_move_the_library_survives_says_where_each_folder_lands() {
        let findings = Findings {
            library: vec![LibraryPath {
                service: "Sonarr".to_owned(),
                path: "/data/media/tv".to_owned(),
                host: Some("/srv/new/media/tv".to_owned()),
                carried: true,
                because: "Sonarr keeps filing into /data/media/tv".to_owned(),
            }],
            ..Findings::default()
        };
        assert!(found(&findings).text().contains("kept  /data/media/tv"));
    }

    #[test]
    fn a_reduction_lists_what_is_in_flight_what_stops_and_what_is_kept() {
        let findings = Findings {
            active: vec![Active {
                protocol: "torrent".to_owned(),
                name: "Ubuntu.iso".to_owned(),
                progress: 94,
            }],
            keeps: vec!["everything already downloaded, in /srv/downloads".to_owned()],
            stops: vec!["qbittorrent".to_owned()],
            ..Findings::default()
        };
        let text = found(&findings).text();
        assert!(text.contains("94% Ubuntu.iso (torrent)"), "{text}");
        assert!(text.contains("This stops: qbittorrent"), "{text}");
        assert!(text.contains("/srv/downloads"), "{text}");
    }

    #[test]
    fn a_hand_edit_underneath_is_shown_from_both_sides() {
        let findings = Findings {
            edited: Some(Edited {
                wrote: "/srv/old".to_owned(),
                found: "/mnt/theirs".to_owned(),
                secret: false,
            }),
            ..Findings::default()
        };
        let text = found(&findings).text();
        assert!(text.contains("lemonfiber wrote  /srv/old"), "{text}");
        assert!(text.contains("the file holds    /mnt/theirs"), "{text}");
    }

    #[test]
    fn adding_a_protocol_lists_what_it_asks_for() {
        let findings = Findings {
            opens: vec![Opening {
                what: "Usenet provider".to_owned(),
                because: "it holds the data your downloaders fetch".to_owned(),
                setting: None,
            }],
            ..Findings::default()
        };
        let text = found(&findings).text();
        assert!(text.contains("This opens:"), "{text}");
        assert!(text.contains("Usenet provider"), "{text}");
    }
}
