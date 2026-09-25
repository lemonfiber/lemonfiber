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

use crate::error::codes::rate::{NOTHING_MEASURED, NOTHING_TO_LIMIT, NO_ZONE, UNREADABLE};
use serde::{Deserialize, Serialize};

pub mod cap;
pub mod capacity;
pub mod holding;
pub mod limit;
pub mod respite;
pub mod rhythm;
pub(crate) mod run;

pub use cap::{Cap, Metered, Reached, WhenExceeded, CRAWL};
pub use capacity::Capacity;
pub use holding::{Answer, Held, Holding, Pulling, Verdict};
pub use limit::{Limit, Resolved};
pub use respite::Respite;
pub use rhythm::{Period, Rhythm, Wall};

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
pub(crate) const SLOWED_SEEDING: &str =
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

/// Everything the operator has declared about the line, kept between runs — and
/// the one thing lemonfiber records about itself.
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
    /// Whether the last run stopped the clients because the cap was spent.
    ///
    /// The one field here nobody declared. It exists so that what lemonfiber
    /// stopped is the only thing lemonfiber starts again: an operator who paused a
    /// client by hand for reasons of their own must not find it running because a
    /// month turned over, and a run that started everything it found stopped would
    /// do exactly that.
    #[serde(default)]
    pub stopped: bool,
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
    pub(crate) fn or_unlimited(limit: Option<Limit>) -> Limit {
        limit.unwrap_or(Limit::Unlimited)
    }
}

/// One direction's limit, as declared and as it comes to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[schemars(rename = "BandwidthReading")]
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
    /// What a spent cap is doing to the figures above, where one is spent.
    pub acting: Option<&'static str>,
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
    let reached = declared
        .cap
        .zip(measured.metered.as_ref())
        .map(|(cap, month)| cap.reached(month.moved()));
    // The figures a spent cap leaves in force rather than the ones declared, so
    // that what the report shows and what the clients were handed are the same
    // pair — read off one function so they cannot come apart.
    let (down, up) = in_force(declared, reached);
    let down = Reading::of(down, capacity.map(|line| line.down));
    let up = Reading::of(up, capacity.map(|line| line.up));

    let respite = declared.respite.map_or(respite::Standing::None, |asked| {
        asked.standing(measured.now)
    });

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
        acting: at_the_cap(declared, reached).map(WhenExceeded::at_the_cap),
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

/// What was chosen for a cap, where one is declared and spent.
///
/// The one place that decides a cap is being acted on, so the report, the limits
/// handed to the clients and the request to stop them cannot disagree about
/// whether the month is over.
#[must_use]
pub(crate) fn at_the_cap(declared: &Declared, reached: Option<Reached>) -> Option<WhenExceeded> {
    matches!(reached, Some(Reached::Exceeded))
        .then(|| declared.cap.map(|cap| cap.exceeded))
        .flatten()
}

/// What both directions come to once a spent cap has had its say.
#[must_use]
pub fn in_force(declared: &Declared, reached: Option<Reached>) -> (Limit, Limit) {
    let spent = at_the_cap(declared, reached);
    let capacity = declared.capacity;
    (
        crawling(
            Declared::or_unlimited(declared.down),
            capacity.map(|line| line.down),
            spent,
        ),
        crawling(
            Declared::or_unlimited(declared.up),
            capacity.map(|line| line.up),
            spent,
        ),
    )
}

/// One direction's limit once a spent cap has had its say.
///
/// Only throttling moves a figure, and only downwards. A crawl written over a
/// limit already below it would be a spent cap making the stack faster, which is
/// the one direction a cap may never move anything.
fn crawling(limit: Limit, carried: Option<u64>, spent: Option<WhenExceeded>) -> Limit {
    if spent != Some(WhenExceeded::Throttle) {
        return limit;
    }
    match limit.against(carried) {
        Resolved::At(bytes) if bytes <= CRAWL => limit,
        Resolved::At(_) | Resolved::Unlimited | Resolved::Unmeasured => Limit::Absolute(CRAWL),
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
mod tests;
