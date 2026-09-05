//! How the line is shared, on a terminal.
//!
//! Ordered by what somebody came here to find out. Where the line stands is first,
//! because that is the question. What is in force is second, with the measured
//! figure beside every share, since a share without it is a setting nobody can
//! check. What the clients are actually doing is third — a limit nobody can see
//! the effect of is a limit that gets turned off.
//!
//! What is *not* limited is last and is always said. The two things an operator
//! fears from a feature like this are that it will throttle the household's own
//! viewing and that it will meddle with the machine, and a report that leaves both
//! to be inferred is a report that gets read as doing them.

use lemonfiber_core::bandwidth::{Answer, Cap, Capacity, Held, Holding, Metered, Reached, Sharing};
use lemonfiber_core::bytes::{a_second, humanize};

use super::Lines;

/// Where the line stands, and what became of anything asked of it.
pub(crate) fn sharing(report: &Sharing) -> Lines {
    let mut lines = Lines::default();
    lines.put(format!("The line — {}", report.means));
    lines.extend(carrying(report.capacity.as_ref(), &report.cautions));
    lines.extend(in_force(report));
    lines.extend(against_the_cap(report));
    lines.extend(keeping(&report.clients));
    lines.extend(untouched(report));
    lines
}

/// What the line was measured to carry, and what that reading is worth.
fn carrying(capacity: Option<&Capacity>, cautions: &[String]) -> Lines {
    let mut lines = Lines::default();
    let Some(line) = capacity else {
        lines.spaced("Nothing has measured this line yet, so a share of it cannot be");
        lines.put("worked out. Give a rate instead, or say what the line carries.");
        return lines;
    };
    lines.spaced(format!(
        "Measured at {} down and {} up — {}",
        a_second(line.down),
        a_second(line.up),
        line.source.means()
    ));
    for caution in cautions {
        lines.put(format!("  {caution}"));
    }
    lines
}

/// What the stack is being held to, and when.
fn in_force(report: &Sharing) -> Lines {
    let mut lines = Lines::default();
    lines.spaced("What the stack may take:");
    // The limit's own sentence rather than one assembled here, because the rule
    // that a share is never shown without the figure it is a share of belongs in
    // one place rather than in each of three surfaces.
    lines.put(format!("  down  {}", report.down.says));
    lines.put(format!("  up    {}", report.up.says));
    if let Some(rhythm) = report.rhythm.as_ref() {
        lines.put(format!(
            "  when  {} — outside those hours the stack has the line",
            rhythm.says()
        ));
        lines.put(match report.zone.as_deref() {
            Some(zone) => format!("        kept by the clients themselves, on {zone} time"),
            None => "        no zone is set, so the clients keep these hours in UTC".to_owned(),
        });
    }
    if let Some(said) = report.respite_says.as_deref() {
        lines.put(format!("  now   {said}"));
    }
    if let Some(ratio) = report.ratio {
        lines.spaced(ratio.to_owned());
    }
    lines
}

/// Where the month stands against a declared cap.
fn against_the_cap(report: &Sharing) -> Lines {
    let mut lines = Lines::default();
    let Some(cap) = report.cap.as_ref() else {
        return lines;
    };
    lines.spaced(format!(
        "A cap of {} a month, and at it: {}",
        humanize(cap.monthly),
        cap.exceeded.means()
    ));
    let Some(month) = report.metered.as_ref() else {
        lines.put("  Nothing could say what has been moved this month, so nothing is");
        lines.put("  counted against it. A cap nothing counts against is not a cap.");
        return lines;
    };
    lines.extend(spent(cap, month, report.reached));
    if let Some(acting) = report.acting {
        lines.put(format!("  {acting}"));
    }
    lines
}

/// What the month has spent, and what that count leaves out.
fn spent(cap: &Cap, month: &Metered, reached: Option<Reached>) -> Lines {
    let mut lines = Lines::default();
    lines.put(format!(
        "  {} moved in {} — {} down, {} up",
        humanize(month.moved()),
        month.month,
        humanize(month.down),
        humanize(month.up)
    ));
    lines.put(match reached {
        Some(Reached::Exceeded) => "  The cap is spent.".to_owned(),
        Some(Reached::Within | Reached::Warning) | None => {
            format!("  {} of it left.", humanize(cap.left(month.moved())))
        }
    });
    for missing in &month.incomplete {
        lines.put(format!("  {missing}"));
    }
    lines.put(format!("  {}", month.excludes));
    lines
}

/// What each client was asked and what it is doing about it.
fn keeping(clients: &[Holding]) -> Lines {
    let mut lines = Lines::default();
    if clients.is_empty() {
        return lines;
    }
    lines.spaced("What the clients say:");
    for client in clients {
        match &client.answer {
            Answer::Silent { said } => {
                lines.put(format!("  {} — would not answer: {said}", client.client));
                lines.put("    So what it is limited to is unknown, not unlimited.");
            }
            Answer::Held { down, up, period } => {
                lines.put(format!(
                    "  {}{}",
                    client.client,
                    period.map_or_else(String::new, |side| format!(" — {}", side.means()))
                ));
                lines.extend(direction("down", down));
                lines.extend(direction("up", up));
            }
        }
        // Beneath the limits rather than beside the name, because it is the
        // louder fact: a client that is not fetching at all is one whose limits
        // are being kept by having nothing to keep them on.
        if let Some(pulling) = client.pulling {
            lines.put(format!("    fetching  {}", pulling.means()));
        }
    }
    lines
}

/// One direction on one client: what it took, what it is moving, and the verdict
/// where there is one worth reading.
fn direction(way: &str, held: &Held) -> Lines {
    let mut lines = Lines::default();
    lines.put(format!(
        "    {way:5} held to {}, moving {}",
        held.accepted.map_or_else(|| "nothing".to_owned(), a_second),
        held.moving.map_or_else(|| "unknown".to_owned(), a_second)
    ));
    if held.verdict.worth_saying() {
        lines.put(format!("      {}", held.verdict.means()));
    }
    lines
}

/// What is outside every limit here, always said.
fn untouched(report: &Sharing) -> Lines {
    let mut lines = Lines::default();
    lines.spaced("Not limited, and never will be:");
    for line in &report.untouched {
        lines.put(format!("  {line}"));
    }
    if report.applied {
        lines.spaced("These limits are now in the clients themselves.");
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::sharing;
    use lemonfiber_core::bandwidth::{
        weigh, Answer, Cap, Capacity, Declared, Held, Holding, Limit, Measured, Metered, Period,
        Pulling, Respite, Rhythm, WhenExceeded,
    };

    /// A moment every case here reads against.
    const NOW: u64 = 1_790_812_800;

    /// A household sharing a measured line, on a schedule, against a cap.
    fn a_household() -> Measured {
        Measured {
            declared: Declared {
                down: Some(Limit::Share(50)),
                up: Some(Limit::Share(25)),
                rhythm: Rhythm::read("07:00-23:00"),
                cap: Some(Cap {
                    monthly: 100 * 1024 * 1024 * 1024,
                    exceeded: WhenExceeded::Throttle,
                }),
                capacity: Some(Capacity {
                    down: 10 * 1024 * 1024,
                    up: 1024 * 1024,
                    source: lemonfiber_core::bandwidth::capacity::Source::Observed,
                    taken: NOW,
                    through_tunnel: true,
                }),
                respite: None,
                stopped: false,
            },
            now: NOW,
            zone: Some("Europe/Amsterdam".to_owned()),
            clients: vec![Holding {
                client: "qbittorrent".to_owned(),
                answer: Answer::Held {
                    down: Held::of(Some(5_242_880), Some(1_000), Some(900), true),
                    up: Held::of(Some(262_144), Some(262_144), Some(100), true),
                    period: Some(Period::Active),
                },
                pulling: None,
            }],
            metered: Some(Metered::of(
                "2026-09",
                50 * 1024 * 1024 * 1024,
                1024 * 1024 * 1024,
                vec!["qbittorrent counts only since it last started.".to_owned()],
            )),
            applied: true,
        }
    }

    /// The same household with its month spent and its clients stopped for it.
    fn a_spent_month() -> Measured {
        let mut spent = a_household();
        spent.declared.cap = Some(Cap {
            monthly: 100,
            exceeded: WhenExceeded::Pause,
        });
        spent.metered = Some(Metered::of("2026-09", 100, 0, Vec::new()));
        for client in &mut spent.clients {
            client.pulling = Some(Pulling::Stopped);
        }
        spent
    }

    /// The whole report as one string, for reading claims out of.
    fn shown(measured: &Measured) -> String {
        sharing(&weigh(measured)).text()
    }

    #[test]
    fn a_share_is_never_shown_without_the_line_it_is_a_share_of() {
        let said = shown(&a_household());
        assert!(said.contains("50%"), "{said}");
        assert!(said.contains("10.0 MiB/s"), "the measured line: {said}");
        assert!(said.contains("5.0 MiB/s"), "and what it comes to: {said}");
    }

    #[test]
    fn what_the_upload_limit_costs_the_ratio_is_said_beside_the_limit() {
        let said = shown(&a_household());
        assert!(said.contains("ratio you are earning"), "{said}");
    }

    #[test]
    fn a_client_that_did_not_take_the_limit_is_named_with_what_to_do_about_it() {
        let said = shown(&a_household());
        assert!(said.contains("own configuration"), "{said}");
    }

    #[test]
    fn a_client_that_would_not_answer_is_unknown_rather_than_unlimited() {
        let mut silent = a_household();
        silent.clients = vec![Holding {
            client: "qbittorrent".to_owned(),
            answer: Answer::Silent {
                said: "connection refused".to_owned(),
            },
            pulling: None,
        }];
        let said = shown(&silent);
        assert!(
            said.contains("would not answer: connection refused"),
            "{said}"
        );
        assert!(said.contains("unknown, not unlimited"), "{said}");
    }

    #[test]
    fn a_client_reporting_nothing_in_force_and_no_rate_is_not_rendered_as_a_figure() {
        // Two absences, and neither of them is a zero. A client reporting no limit
        // has not taken the setting, and one whose throughput read failed has not
        // been measured at all — rendering either as a rate would invent the one
        // number the operator opened this report to check.
        let mut unsaid = a_household();
        unsaid.clients = vec![Holding {
            client: "qbittorrent".to_owned(),
            answer: Answer::Held {
                down: Held::of(Some(5_242_880), None, None, true),
                up: Held::of(Some(262_144), None, None, true),
                period: None,
            },
            pulling: None,
        }];
        let said = shown(&unsaid);
        assert!(said.contains("held to nothing, moving unknown"), "{said}");
        assert!(
            said.contains("own configuration"),
            "and a figure it did not take is still a setting that did not take: {said}"
        );
    }

    #[test]
    fn what_is_never_limited_is_said_on_every_report() {
        for measured in [a_household(), Measured::default()] {
            let said = shown(&measured);
            assert!(said.contains("watching from your own library"), "{said}");
            assert!(said.contains("shape the machine's traffic"), "{said}");
        }
    }

    #[test]
    fn a_month_says_what_it_does_not_count_beside_what_it_does() {
        let said = shown(&a_household());
        assert!(said.contains("moved in 2026-09"), "{said}");
        assert!(said.contains("is not counted here"), "{said}");
        assert!(said.contains("since it last started"), "{said}");
    }

    #[test]
    fn a_cap_with_nothing_counting_against_it_says_so_rather_than_reading_as_empty() {
        let mut uncounted = a_household();
        uncounted.metered = None;
        let said = shown(&uncounted);
        assert!(said.contains("nothing is"), "{said}");
        assert!(
            said.contains("A cap nothing counts against is not a cap"),
            "{said}"
        );
    }

    #[test]
    fn a_spent_cap_says_it_is_spent_rather_than_offering_nothing_left() {
        let mut spent = a_household();
        spent.metered = Some(Metered::of(
            "2026-09",
            200 * 1024 * 1024 * 1024,
            0,
            Vec::new(),
        ));
        assert!(shown(&spent).contains("The cap is spent."));
    }

    #[test]
    fn a_line_nobody_measured_says_what_to_do_about_it() {
        let said = shown(&Measured::default());
        assert!(
            said.contains("Nothing has measured this line yet"),
            "{said}"
        );
        assert!(said.contains("say what the line carries"), "{said}");
    }

    #[test]
    fn the_zone_the_hours_are_kept_in_is_named_and_its_absence_is_said() {
        assert!(shown(&a_household()).contains("on Europe/Amsterdam time"));
        let mut zoneless = a_household();
        zoneless.zone = None;
        assert!(shown(&zoneless).contains("keep these hours in UTC"));
    }

    #[test]
    fn an_override_that_is_running_says_when_it_ends_itself() {
        let mut lifted = a_household();
        lifted.declared.respite = Some(Respite {
            until: NOW + 45 * 60,
        });
        let said = shown(&lifted);
        assert!(said.contains("45 minutes"), "{said}");
        assert!(said.contains("come back on their own"), "{said}");
    }

    #[test]
    fn a_spent_cap_says_what_was_done_about_it_and_which_clients_it_was_done_to() {
        // Both, because neither is the other: the choice says what should be
        // happening and the clients say what is. A report with only the first is
        // one an operator has to take on faith.
        let said = shown(&a_spent_month());
        assert!(said.contains("The cap is spent."), "{said}");
        assert!(
            said.contains("the download clients are stopped"),
            "the choice is not only named, it is said to be in force: {said}"
        );
        assert!(said.contains("stopped, and taking nothing new"), "{said}");
    }

    #[test]
    fn a_month_that_is_not_over_says_nothing_about_stopping_anything() {
        let said = shown(&a_household());
        assert!(!said.contains("The cap is spent"), "{said}");
        assert!(!said.contains("nothing new is fetched"), "{said}");
        assert!(!said.contains("stopped, and taking nothing new"), "{said}");
    }
}
