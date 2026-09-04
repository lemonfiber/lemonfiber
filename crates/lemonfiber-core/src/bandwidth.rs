//! What the household's line carries, and how much of it the stack may take.
//!
//! A media stack saturates whatever connection it is given, and the people who
//! notice are the ones who did not install it: the call stutters, the game lags,
//! the film buffers. That is a social failure rather than a technical one, and it
//! is usually settled by turning the stack off during the day — so the limits here
//! exist to keep it running rather than to make it slower.
//!
//! Four decisions shape everything here.
//!
//! **A limit is expressible as a share of the line.** Almost nobody knows their
//! connection in bytes a second, and everybody knows they want the stack to have
//! about half of it in the evening. A share always travels with the figure it is a
//! share of, so a setting can be checked rather than believed.
//!
//! **The schedule is written into the clients, not acted on from here.** The
//! household's day is declared once and handed to each download client's own
//! scheduler, which runs on the container's clock — the clock the stack sets from
//! `TZ`. That is what makes the boundary land on the household's own wall clock,
//! and what makes a daylight-saving transition somebody else's problem: this
//! product holds instants and calendar days with no time of day between them, and
//! a schedule it computed itself would be a schedule in the wrong hour twice a
//! year. Which side of the boundary the stack is on is therefore *read back* from
//! the client rather than worked out, which also makes it a measurement.
//!
//! **A limit is verified, not assumed.** A client that accepts a setting and does
//! not apply it looks exactly like one that did, so every limit is read back and
//! the throughput is read beside it. A cap whose effect the operator cannot see is
//! a cap they turn off.
//!
//! **Only the stack is limited.** Not the machine, not the household. lemonfiber
//! sets rate limits on its own download clients through their own APIs. It does
//! not shape the host's traffic, which would want privileges it should not hold
//! and would reach applications that are none of its business — and it never
//! touches anybody watching from the library, because that traffic does not go out
//! over the line at all.

use serde::{Deserialize, Serialize};

pub mod cap;
pub mod capacity;
pub mod holding;
pub mod limit;
pub mod respite;
pub mod rhythm;

pub use cap::{Cap, Metered, Reached, WhenExceeded};
pub use capacity::Capacity;
pub use holding::{Answer, Held, Holding, Verdict};
pub use limit::{Limit, Resolved};
pub use respite::Respite;
pub use rhythm::{Period, Rhythm, Wall};

/// Raised when a limit is expressed as a share of a line nothing has measured.
pub const NOTHING_MEASURED: crate::error::Code = crate::error::Code::new("RATE-1");

/// Raised when a schedule is asked for and nothing says which zone the clients
/// would read it in.
pub const NO_ZONE: crate::error::Code = crate::error::Code::new("RATE-2");

/// Raised when what was asked for could not be read as a limit, a window or a cap.
pub const UNREADABLE: crate::error::Code = crate::error::Code::new("RATE-3");

/// Raised when there is no download client to limit.
pub const NOTHING_TO_LIMIT: crate::error::Code = crate::error::Code::new("RATE-4");

/// What throttling the upload costs, said the same way wherever it is said.
///
/// Stated in what it does rather than in what it is. Seeding is an obligation on a
/// private tracker rather than a courtesy, so a limit is offered in preference to
/// stopping — but a slower upload earns ratio more slowly, and an operator is owed
/// that before the limit rather than after the warning from the tracker.
///
/// Its own sentence rather than [`crate::space::RATIO_CONSEQUENCE`], because the two
/// are different costs and an operator acts differently on each: that one is what
/// *letting a download go* takes away, which is a ratio already earned, and this is
/// what a limit does to the ratio still being earned. Saying either in the other's
/// place would be telling somebody they were about to lose something they were not.
pub const SLOWED_SEEDING: &str =
    "A limit on the upload slows what you give back. On a private tracker the ratio \
     you are earning is what your account is kept on, so a limit that runs for weeks \
     can cost standing you cannot buy back. Throttling is offered rather than \
     stopping for exactly this reason: a slow seed still counts and a stopped one \
     does not.";

/// What is deliberately outside every limit here, always said.
///
/// Always, and not only where somebody might wonder. The two things an operator
/// most fears from a bandwidth feature are that it will throttle the household's
/// own viewing and that it will meddle with the machine, and a report that leaves
/// both to be inferred is a report that gets read as doing them.
pub const UNTOUCHED: [&str; 2] = [
    "Anybody watching from your own library, over your own network. That traffic \
     never goes out over the line, so nothing here can slow it down and nothing \
     here tries.",
    "Everything else this machine does. lemonfiber sets limits inside its own \
     download clients and nowhere else — it does not shape the machine's traffic, \
     which would want privileges it should not hold and would reach applications \
     that are none of its business.",
];

/// Where the line stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Restraint {
    /// Nothing is configured.
    Unlimited,
    /// Limits are in force, with no schedule to switch them.
    Limited,
    /// Inside the household's active hours, so the limits apply.
    ScheduledActive,
    /// Outside them, so the line is the stack's.
    ScheduledQuiet,
    /// A temporary override is lifting the limits, and will expire.
    Overridden,
    /// The month is close enough to a declared cap to say so.
    CapWarning,
    /// The cap has been reached and the declared behaviour applies.
    CapExceeded,
}

impl Restraint {
    /// The one word that describes the line, given everything known about it.
    ///
    /// The order is the point. A cap that has been reached is the loudest thing
    /// true of a metered line and outranks everything, because it is the one with
    /// a bill behind it; a cap being approached comes next for the same reason. An
    /// override outranks the schedule because it is what is actually happening to
    /// the limits right now, and the schedule outranks a plain limit because
    /// "limited" is true of both and says less. Everything this hides is carried
    /// in the report beside it, so the headline is a summary rather than the whole
    /// answer.
    #[must_use]
    pub fn reached(
        limited: bool,
        period: Option<Period>,
        lifted: bool,
        cap: Option<Reached>,
    ) -> Self {
        match cap {
            Some(Reached::Exceeded) => return Self::CapExceeded,
            Some(Reached::Warning) => return Self::CapWarning,
            Some(Reached::Within) | None => {}
        }
        if lifted {
            return Self::Overridden;
        }
        match period {
            Some(Period::Active) => Self::ScheduledActive,
            Some(Period::Quiet) => Self::ScheduledQuiet,
            None if limited => Self::Limited,
            None => Self::Unlimited,
        }
    }

    /// What this means for the household, in the words it is shown in.
    #[must_use]
    pub const fn means(self) -> &'static str {
        match self {
            Self::Unlimited => {
                "nothing holds the stack back, so it will take whatever the line has"
            }
            Self::Limited => "the stack is held to its limits around the clock",
            Self::ScheduledActive => "people are up, so the stack is held to its limits",
            Self::ScheduledQuiet => "the house is asleep, so the stack has the line",
            Self::Overridden => {
                "you lifted the limits for a while, and they come back on their own"
            }
            Self::CapWarning => "the month is nearly spent against the cap you declared",
            Self::CapExceeded => "the cap you declared is spent, and what you chose for it applies",
        }
    }

    /// Whether this is worth putting in front of an operator who asked about
    /// something else.
    #[must_use]
    pub const fn worth_saying(self) -> bool {
        matches!(
            self,
            Self::CapWarning | Self::CapExceeded | Self::Overridden
        )
    }
}

/// Everything the operator has declared about the line, kept between runs.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Declared {
    /// The download limit, where one was declared.
    #[serde(default)]
    pub down: Option<Limit>,
    /// The upload limit, declared apart from the download one.
    #[serde(default)]
    pub up: Option<Limit>,
    /// The household's hours, declared once for every client.
    #[serde(default)]
    pub rhythm: Option<Rhythm>,
    /// The monthly cap and what to do at it.
    #[serde(default)]
    pub cap: Option<Cap>,
    /// What the line was measured to carry.
    #[serde(default)]
    pub capacity: Option<Capacity>,
    /// A temporary override, where one is outstanding.
    #[serde(default)]
    pub respite: Option<Respite>,
}

impl Declared {
    /// Whether anything at all holds the stack back.
    #[must_use]
    pub fn limited(&self) -> bool {
        [self.down, self.up]
            .into_iter()
            .flatten()
            .any(|limit| limit != Limit::Unlimited)
    }

    /// The limit for one direction, or no limit where none was declared.
    #[must_use]
    pub fn or_unlimited(limit: Option<Limit>) -> Limit {
        limit.unwrap_or(Limit::Unlimited)
    }
}

/// One direction's limit, as declared and as it comes to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Reading {
    /// The limit as it was expressed.
    pub limit: Limit,
    /// What it comes to against the measured line.
    pub resolved: Resolved,
    /// The limit and the line it was measured against, in one sentence.
    ///
    /// Carried rather than left to each surface, so the rule that a share is never
    /// shown without the figure it is a share of is kept in one place instead of
    /// three.
    pub says: String,
}

impl Reading {
    /// One direction weighed against what the line was measured to carry.
    #[must_use]
    pub fn of(limit: Limit, capacity: Option<u64>) -> Self {
        Self {
            limit,
            resolved: limit.against(capacity),
            says: limit.says(capacity),
        }
    }

    /// The figure a client would be given, where there is one to give.
    #[must_use]
    pub const fn bytes(&self) -> Option<u64> {
        match self.resolved {
            Resolved::At(bytes) => Some(bytes),
            // A share of a line nobody measured holds nothing back, which is the
            // one case a refusal is raised for rather than quietly applied.
            Resolved::Unlimited | Resolved::Unmeasured => None,
        }
    }
}

/// Everything one report is made from, gathered before any of it is judged.
#[derive(Debug, Default)]
pub struct Measured {
    /// What the operator declared.
    pub declared: Declared,
    /// The moment this reading was taken, in seconds since the epoch.
    pub now: u64,
    /// The zone the stack tells its containers to read a clock in, where it says.
    pub zone: Option<String>,
    /// What each download client said about the limits on it.
    pub clients: Vec<Holding>,
    /// What the stack itself moved this month, where anything could count it.
    pub metered: Option<Metered>,
    /// Whether this run wrote the limits to the clients or only read them.
    pub applied: bool,
}

/// How the line is shared, and what that costs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Sharing {
    /// Where the line stands.
    pub restraint: Restraint,
    /// What that means for the household.
    pub means: String,
    /// What the line was measured to carry.
    pub capacity: Option<Capacity>,
    /// What is worth knowing about that reading before trusting it.
    pub cautions: Vec<String>,
    /// The download limit.
    pub down: Reading,
    /// The upload limit, which is declared apart and defaults lower.
    pub up: Reading,
    /// The household's hours, where any were declared.
    pub rhythm: Option<Rhythm>,
    /// The zone the clients read those hours in, where the stack says.
    pub zone: Option<String>,
    /// The monthly cap, where one was declared.
    pub cap: Option<Cap>,
    /// Where the month stands against it.
    pub reached: Option<Reached>,
    /// What the stack itself moved this month.
    pub metered: Option<Metered>,
    /// The override, where one is running or has just run out.
    pub respite: respite::Standing,
    /// What the override amounts to, in words.
    pub respite_says: Option<String>,
    /// What each download client was asked and what it is doing about it.
    pub clients: Vec<Holding>,
    /// What throttling the upload costs, where an upload limit is in force.
    pub ratio: Option<&'static str>,
    /// What is outside every limit here.
    pub untouched: Vec<&'static str>,
    /// Whether this run wrote the limits to the clients or only read them.
    pub applied: bool,
}

/// Judge what was measured.
///
/// Everything is decided from the values handed in, so a limit lifted by hand in a
/// client's own interface reads as lifted on the next run rather than as whatever
/// lemonfiber last wrote.
#[must_use]
pub fn weigh(measured: &Measured) -> Sharing {
    let declared = &measured.declared;
    let capacity = declared.capacity;
    let down = Reading::of(
        Declared::or_unlimited(declared.down),
        capacity.map(|line| line.down),
    );
    let up = Reading::of(
        Declared::or_unlimited(declared.up),
        capacity.map(|line| line.up),
    );

    let respite = declared.respite.map_or(respite::Standing::None, |asked| {
        asked.standing(measured.now)
    });
    let reached = declared
        .cap
        .zip(measured.metered.as_ref())
        .map(|(cap, month)| cap.reached(month.moved()));

    let restraint = Restraint::reached(
        declared.limited(),
        period(measured),
        respite.lifting(),
        reached,
    );

    Sharing {
        restraint,
        means: restraint.means().to_owned(),
        capacity,
        cautions: capacity
            .map(|line| line.cautions(measured.now))
            .unwrap_or_default(),
        // The consequence is read off the limit rather than set beside it, so a
        // throttled upload can never be reported without what it costs.
        ratio: (up.limit != Limit::Unlimited).then_some(SLOWED_SEEDING),
        down,
        up,
        rhythm: declared.rhythm,
        zone: measured.zone.clone(),
        cap: declared.cap,
        reached,
        metered: measured.metered.clone(),
        respite_says: respite.says(),
        respite,
        clients: measured.clients.clone(),
        untouched: UNTOUCHED.to_vec(),
        applied: measured.applied,
    }
}

/// What this run's throughput says about the line, where it says anything.
///
/// Only a client nothing was holding back is evidence. A rate measured under a
/// limit is a measurement of the limit, and recording it as what the line carries
/// is how a stack throttled to a tenth talks itself down to a tenth of its own
/// connection — and then to a tenth of that.
///
/// Where only one direction was free the other reads as nothing seen, which is a
/// figure that raises no high-water mark and resolves no share: an unmeasured
/// direction and a direction measured at nothing must never come apart here.
#[must_use]
pub fn observed(clients: &[Holding], taken: u64, through_tunnel: bool) -> Option<Capacity> {
    let mut down = 0_u64;
    let mut up = 0_u64;
    for client in clients {
        let Answer::Held {
            down: pulling,
            up: giving,
            ..
        } = &client.answer
        else {
            continue;
        };
        down = down.saturating_add(unrestrained(pulling).unwrap_or(0));
        up = up.saturating_add(unrestrained(giving).unwrap_or(0));
    }
    (down > 0 || up > 0).then_some(Capacity {
        down,
        up,
        source: capacity::Source::Observed,
        taken,
        through_tunnel,
    })
}

/// What one direction was moving, where nothing at all was holding it back.
fn unrestrained(held: &Held) -> Option<u64> {
    matches!(held.verdict, Verdict::Unasked)
        .then_some(held.moving)
        .flatten()
}

/// Which side of the household's day the clients say they are on.
///
/// Read from the clients rather than worked out, because nothing in this product
/// knows the household's local time of day. A client that keeps no schedule has no
/// opinion, and where two disagree the constrained answer wins: a report that said
/// the house was asleep while one client was still throttled would be describing
/// neither client.
fn period(measured: &Measured) -> Option<Period> {
    let said: Vec<Period> = measured
        .clients
        .iter()
        .filter_map(|client| match &client.answer {
            holding::Answer::Held { period, .. } => *period,
            holding::Answer::Silent { .. } => None,
        })
        .collect();
    if said.contains(&Period::Active) {
        return Some(Period::Active);
    }
    said.first().copied()
}

#[cfg(test)]
mod tests {
    use super::{
        cap::{Cap, Metered, Reached, WhenExceeded},
        capacity::{Capacity, Source},
        holding::{Answer, Held, Holding},
        limit::Limit,
        respite::Respite,
        rhythm::{Period, Rhythm},
        weigh, Declared, Measured, Reading, Resolved, Restraint, UNTOUCHED,
    };

    /// A moment every case here reads against.
    const NOW: u64 = 1_790_812_800;

    /// Ten megabytes down, one up, measured today.
    fn a_line() -> Capacity {
        Capacity {
            down: 10 * 1024 * 1024,
            up: 1024 * 1024,
            source: Source::Observed,
            taken: NOW,
            through_tunnel: false,
        }
    }

    /// A stack holding the stack to half the line, with the house awake.
    fn a_household() -> Measured {
        Measured {
            declared: Declared {
                down: Some(Limit::Share(50)),
                up: Some(Limit::Share(25)),
                rhythm: Rhythm::read("07:00-23:00"),
                cap: None,
                capacity: Some(a_line()),
                respite: None,
            },
            now: NOW,
            zone: Some("Europe/Amsterdam".to_owned()),
            clients: vec![client(Some(Period::Active))],
            metered: None,
            applied: false,
        }
    }

    /// A client that would not answer at all.
    fn silent() -> Holding {
        Holding {
            client: "qbittorrent".to_owned(),
            answer: Answer::Silent {
                said: "connection refused".to_owned(),
            },
        }
    }

    /// A client answering that it is holding, on the side of the day given.
    fn client(period: Option<Period>) -> Holding {
        Holding {
            client: "qbittorrent".to_owned(),
            answer: Answer::Held {
                down: Held::of(Some(1_000), Some(1_000), Some(500), true),
                up: Held::of(Some(100), Some(100), Some(50), true),
                period,
            },
        }
    }

    #[test]
    fn a_share_is_weighed_against_the_line_that_direction_was_measured_at() {
        // Down against the downlink and up against the uplink. One figure for
        // both would make every upload share several times what was asked for,
        // because a home connection is asymmetric.
        let shared = weigh(&a_household());
        assert_eq!(shared.down.resolved, Resolved::At(5 * 1024 * 1024));
        assert_eq!(shared.up.resolved, Resolved::At(256 * 1024));
        assert!(
            shared.down.says.contains("10.0 MiB/s"),
            "{}",
            shared.down.says
        );
        assert!(shared.up.says.contains("1.0 MiB/s"), "{}", shared.up.says);
    }

    #[test]
    fn an_upload_limit_is_never_reported_without_what_it_costs_the_ratio() {
        let shared = weigh(&a_household());
        let ratio = shared.ratio.unwrap_or_default();
        assert!(ratio.contains("ratio"), "{ratio}");
        assert!(ratio.contains("standing"), "{ratio}");
        assert!(
            ratio.contains("stopped one does not"),
            "throttling is offered rather than stopping, and says why: {ratio}"
        );
    }

    #[test]
    fn no_upload_limit_has_no_consequence_to_state() {
        let mut measured = a_household();
        measured.declared.up = None;
        assert_eq!(weigh(&measured).ratio, None);
    }

    #[test]
    fn what_is_outside_every_limit_here_is_always_said() {
        // Both of them, on every report. An operator's two fears about a
        // bandwidth feature are that it throttles the household's own viewing
        // and that it meddles with the machine; leaving either to be inferred is
        // how a report gets read as doing them.
        let shared = weigh(&Measured::default());
        assert_eq!(shared.untouched.len(), UNTOUCHED.len());
        assert!(!shared.untouched.is_empty());
        let said = shared.untouched.join(" ");
        assert!(said.contains("watching from your own library"), "{said}");
        assert!(said.contains("never goes out over the line"), "{said}");
        assert!(
            said.contains("does not shape the machine's traffic"),
            "{said}"
        );
    }

    #[test]
    fn which_side_of_the_day_it_is_is_read_from_the_clients_rather_than_a_clock() {
        // Nothing in this product knows the household's local time of day. The
        // client's own scheduler does, so the answer is a measurement.
        assert_eq!(weigh(&a_household()).restraint, Restraint::ScheduledActive);

        let mut asleep = a_household();
        asleep.clients = vec![client(Some(Period::Quiet))];
        assert_eq!(weigh(&asleep).restraint, Restraint::ScheduledQuiet);

        let mut unscheduled = a_household();
        unscheduled.clients = vec![client(None)];
        assert_eq!(weigh(&unscheduled).restraint, Restraint::Limited);
    }

    #[test]
    fn where_two_clients_disagree_the_constrained_answer_wins() {
        // A report saying the house was asleep while one client was still
        // throttled would be describing neither client.
        let mut mixed = a_household();
        mixed.clients = vec![client(Some(Period::Quiet)), client(Some(Period::Active))];
        assert_eq!(weigh(&mixed).restraint, Restraint::ScheduledActive);
    }

    #[test]
    fn a_client_that_would_not_answer_has_no_opinion_about_the_hour() {
        let mut unreachable = a_household();
        unreachable.clients = vec![silent()];
        assert_eq!(weigh(&unreachable).restraint, Restraint::Limited);
    }

    #[test]
    fn a_stack_with_nothing_declared_is_unlimited_and_says_so() {
        let shared = weigh(&Measured::default());
        assert_eq!(shared.restraint, Restraint::Unlimited);
        assert!(shared.means.contains("whatever the line has"));
        assert_eq!(shared.down.limit, Limit::Unlimited);
        assert!(shared.capacity.is_none());
        assert!(shared.cautions.is_empty());
        assert!(shared.rhythm.is_none());
        assert!(shared.reached.is_none());
        assert!(shared.respite_says.is_none());
    }

    #[test]
    fn a_cap_that_is_spent_outranks_everything_else_true_of_the_line() {
        // It is the one with a bill behind it.
        let mut metered = a_household();
        metered.declared.cap = Some(Cap {
            monthly: 100,
            exceeded: WhenExceeded::Pause,
        });
        metered.metered = Some(Metered::of("2026-09", 100, 0, Vec::new()));
        let shared = weigh(&metered);
        assert_eq!(shared.restraint, Restraint::CapExceeded);
        assert_eq!(shared.reached, Some(Reached::Exceeded));
        assert!(shared.restraint.worth_saying());

        metered.metered = Some(Metered::of("2026-09", 95, 0, Vec::new()));
        assert_eq!(weigh(&metered).restraint, Restraint::CapWarning);

        metered.metered = Some(Metered::of("2026-09", 1, 0, Vec::new()));
        assert_eq!(weigh(&metered).restraint, Restraint::ScheduledActive);
    }

    #[test]
    fn a_cap_with_nothing_counting_against_it_is_not_a_verdict() {
        // Declaring a cap on a stack whose clients cannot be read is not the same
        // as being inside it, and must not report as being inside it.
        let mut declared = a_household();
        declared.declared.cap = Some(Cap {
            monthly: 100,
            exceeded: WhenExceeded::Continue,
        });
        assert_eq!(weigh(&declared).reached, None);
    }

    #[test]
    fn an_override_outranks_the_schedule_because_it_is_what_is_happening_now() {
        let mut lifted = a_household();
        lifted.declared.respite = Some(Respite {
            until: NOW + 30 * 60,
        });
        let shared = weigh(&lifted);
        assert_eq!(shared.restraint, Restraint::Overridden);
        assert!(shared
            .respite_says
            .is_some_and(|said| said.contains("come back on their own")));
    }

    #[test]
    fn an_override_that_ran_out_stops_outranking_anything_and_still_reports() {
        let mut expired = a_household();
        expired.declared.respite = Some(Respite {
            until: NOW - 60 * 60,
        });
        let shared = weigh(&expired);
        assert_eq!(shared.restraint, Restraint::ScheduledActive);
        assert!(shared
            .respite_says
            .is_some_and(|said| said.contains("came back")));
    }

    #[test]
    fn what_is_worth_knowing_about_a_stale_reading_travels_with_it() {
        let mut old = a_household();
        old.declared.capacity = Some(Capacity {
            taken: NOW - super::capacity::GOES_STALE_AFTER - 1,
            through_tunnel: true,
            ..a_line()
        });
        let shared = weigh(&old);
        assert_eq!(shared.cautions.len(), 2, "{:?}", shared.cautions);
    }

    #[test]
    fn only_a_client_nothing_was_holding_back_has_measured_the_line() {
        let free = Holding {
            client: "qbittorrent".to_owned(),
            answer: Answer::Held {
                down: Held::of(None, None, Some(20 * 1024 * 1024), true),
                up: Held::of(None, None, Some(2 * 1024 * 1024), true),
                period: None,
            },
        };
        assert!(super::observed(&[free], NOW, false)
            .is_some_and(|seen| seen.down == 20 * 1024 * 1024 && seen.up == 2 * 1024 * 1024));

        // A rate measured under a limit is a measurement of the limit. Recording
        // it would talk a throttled stack down to a tenth of its own connection,
        // and then to a tenth of that.
        assert_eq!(super::observed(&[client(None)], NOW, false), None);
        assert_eq!(super::observed(&[], NOW, false), None);
        assert_eq!(super::observed(&[silent()], NOW, false), None);
    }

    #[test]
    fn a_direction_with_no_figure_to_give_a_client_has_none() {
        assert_eq!(Reading::of(Limit::Unlimited, Some(100)).bytes(), None);
        assert_eq!(Reading::of(Limit::Share(50), None).bytes(), None);
        assert_eq!(Reading::of(Limit::Share(50), Some(100)).bytes(), Some(50));
    }

    #[test]
    fn only_a_line_that_is_going_wrong_interrupts_somebody_asking_about_something_else() {
        assert!(!Restraint::Unlimited.worth_saying());
        assert!(!Restraint::Limited.worth_saying());
        assert!(!Restraint::ScheduledActive.worth_saying());
        assert!(!Restraint::ScheduledQuiet.worth_saying());
        assert!(Restraint::Overridden.worth_saying());
        assert!(Restraint::CapWarning.worth_saying());
        assert!(Restraint::CapExceeded.worth_saying());
    }

    #[test]
    fn every_state_this_module_can_reach_says_what_it_means() {
        for state in [
            Restraint::Unlimited,
            Restraint::Limited,
            Restraint::ScheduledActive,
            Restraint::ScheduledQuiet,
            Restraint::Overridden,
            Restraint::CapWarning,
            Restraint::CapExceeded,
        ] {
            let means = state.means();
            assert!(!means.is_empty(), "{state:?}");
        }
    }

    #[test]
    fn a_declared_limit_of_none_at_all_is_not_a_limited_line() {
        let declared = Declared {
            down: Some(Limit::Unlimited),
            up: Some(Limit::Unlimited),
            ..Declared::default()
        };
        assert!(!declared.limited());
        assert!(Declared {
            down: Some(Limit::Share(50)),
            ..Declared::default()
        }
        .limited());
    }

    #[test]
    fn every_code_this_module_raises_belongs_to_it() {
        for code in [
            super::NOTHING_MEASURED,
            super::NO_ZONE,
            super::UNREADABLE,
            super::NOTHING_TO_LIMIT,
        ] {
            // Bound rather than called inside the message: an argument to a
            // passing assertion is never evaluated, and a line nothing evaluates
            // is one the coverage gate counts against a file that looks tested.
            let named = code.as_str();
            assert!(named.starts_with("RATE-"), "{named} is this feature's own");
        }
    }
}
