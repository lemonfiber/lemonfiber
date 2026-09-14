//! What a seed did, connection by connection.
//!
//! The states a wiring can rest in and the report they add up to. Kept apart from the
//! wiring itself so that what is *said* about a connection has one definition rather
//! than one per operation.

use super::{client_field, Baselines, DownloadClient, Failure};
use serde::Serialize;

/// How one connection turned out after a seed pass.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(tag = "state", rename_all = "kebab-case")]
#[schemars(rename = "SeedState")]
pub enum State {
    /// Written and read back.
    Wired,
    /// Present and correct; nothing was done.
    AlreadyWired,
    /// Present but operator-changed; preserved.
    Drifted,
    /// Present and still lemonfiber's own value, but behind lemonfiber's intent —
    /// it should be brought up to date. Reported until an update path applies it,
    /// and never overwritten in the meantime.
    Stale,
    /// Both the service's value and lemonfiber's intent moved away from the
    /// baseline. The conflict is presented — the value the operator set beside the
    /// one lemonfiber would write — and the value left as it is; lemonfiber does not
    /// resolve it on its own.
    ///
    /// Both values are shown in the report and serialized with it, so a
    /// secret-bearing field must not report a conflict through this variant: a
    /// conflict in a secret is to be reported without either value on show, and
    /// wants a masked shape of its own rather than this one.
    Conflicted {
        /// The value the service now holds, as the operator set it; `None` where
        /// they cleared it.
        yours: Option<String>,
        /// The value lemonfiber would write in its place.
        ours: String,
    },
    /// An operator's edit adopted as the accepted state, kept across seeds and
    /// restores. Settled: lemonfiber leaves it as it is.
    Adopted,
    /// A value the service already held that lemonfiber never wrote — the operator's
    /// own, pre-existing. Adopted as the baseline this run rather than reported as
    /// drift, so an existing setup is taken on instead of flagged wholesale. Its
    /// value is not shown, so a secret among the adopted is never put on display.
    Unmanaged,
    /// Not there, or not at what lemonfiber would have it be, and this run only said
    /// so.
    ///
    /// The one outcome a pass that writes nothing can reach where a pass that writes
    /// would have written. What a real run would leave the service holding sits beside
    /// what it holds now, because a report saying a connection would be made without
    /// saying what it would be made *to* is a count rather than an account — and a
    /// count is what an operator asking for a rehearsal already has.
    ///
    /// Both values are serialized, so a secret-bearing field must not report through
    /// this variant carrying one. It does not have to: `ours` is absent exactly where
    /// a real run would generate the value rather than read it, which is the only
    /// place a credential arises — the torrent client's web UI password and the media
    /// server's admin account. A value minted to describe a rehearsal is a secret that
    /// exists because somebody asked a question, and it would then have to be kept or
    /// thrown away.
    ///
    /// `yours` is absent where the service holds nothing, and where this run could not
    /// ask without writing — reading the household's telling means signing in as the
    /// owner, and a session is state on somebody else's service.
    WouldWire {
        /// What the service holds now, or `None` where it holds nothing or could not
        /// be asked.
        yours: Option<String>,
        /// What a real run would leave it holding, or `None` where that value would be
        /// generated rather than read.
        ours: Option<String>,
    },
    /// An operator's own value a real run would take on as the accepted state, and
    /// this one did not.
    ///
    /// Apart from [`Self::WouldWire`] because it is the other direction: nothing would
    /// be written to the service at all, and what would move is lemonfiber's record of
    /// what it expects. Its value is not shown, exactly as [`Self::Unmanaged`] does not
    /// show one, so a secret among the adopted is never put on display by a question.
    WouldAdopt,
    /// An area the operator declared unmanaged. Nothing was read from the service and
    /// nothing was written to it, and the reason they gave is carried so a report says
    /// whose decision it was.
    ///
    /// Apart from [`Self::Unmanaged`], which lemonfiber *infers* from a value it has
    /// no record of having written, and which it adopts as the baseline so that later
    /// runs recognise it. This one is a decision somebody wrote down, and it holds
    /// whether or not lemonfiber would have had anything to say — nothing is adopted,
    /// because adopting would be the first half of managing it again.
    Observed {
        /// Why the operator said to leave it alone, in their own words.
        reason: String,
    },
    /// Prerequisite unavailable; a later run will complete it.
    Skipped {
        /// Why it could not be attempted.
        reason: String,
    },
    /// Attempted and rejected, carrying the service's own words.
    Failed {
        /// What the service said.
        detail: String,
    },
    /// Refused by lemonfiber's own policy, carrying the reason a re-run will not
    /// resolve — such as two \*arrs pointed at one root folder, or a service that
    /// does not serve the API version this build speaks. Either way nothing it
    /// names was written, whether it was refused before the write or the write
    /// itself found nothing to land in.
    Refused {
        /// Why it was refused, in lemonfiber's own words.
        reason: String,
    },
}

impl State {
    /// Whether this connection is settled: wired one way or another, or left in a
    /// working state — the operator's own edit, an adopted or pre-existing value that
    /// is theirs to keep, an area they declared unmanaged, or lemonfiber's own value
    /// that is merely behind its newer intent. A skip, a failure, a refusal or a
    /// conflict is not settled: a re-run or an operator's decision must return to it.
    ///
    /// Neither of the two a rehearsal reports is settled, and that is the whole of what
    /// a rehearsal's last line says: these are the connections a real run would still
    /// have to make. A pass that only said what it would do settled nothing by saying
    /// it.
    #[must_use]
    pub const fn is_settled(&self) -> bool {
        matches!(
            self,
            Self::Wired
                | Self::AlreadyWired
                | Self::Drifted
                | Self::Stale
                | Self::Adopted
                | Self::Unmanaged
                | Self::Observed { .. }
        )
    }
}

/// How serious a reported connection is.
///
/// Drift is normal and usually the operator's own harmless edit, so it is reported
/// as information rather than a failure. It escalates to a warning only when the
/// drift breaks the stack — a root folder pointing where nothing exists, a download
/// client that no longer answers — and a warning that cannot be acted on is noise,
/// so a warning always names both what broke and what to do about it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(tag = "severity", rename_all = "kebab-case")]
#[schemars(rename = "SeedSeverity")]
pub enum Severity {
    /// Nothing is broken: the connection is settled, or its drift is the operator's
    /// own edit that still works.
    #[default]
    Informational,
    /// The connection breaks the stack. Both the breakage and a remediation are
    /// named, because a warning the operator cannot act on is noise.
    Warning {
        /// What is broken, in the operator's terms.
        breakage: String,
        /// What to do about it.
        remediation: String,
    },
}

impl Severity {
    /// Whether this is a warning — a drift that broke something, not the ordinary
    /// informational kind.
    #[must_use]
    pub const fn is_warning(&self) -> bool {
        matches!(self, Self::Warning { .. })
    }
}

/// One connection, and how it turned out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Wiring {
    /// What was being connected, such as `SABnzbd into Sonarr`.
    pub connection: String,
    /// How it turned out.
    pub state: State,
    /// How serious the outcome is — information by default, a warning where the
    /// connection breaks the stack.
    pub severity: Severity,
}

impl Wiring {
    /// A connection reported at the ordinary, informational severity — the common
    /// case, and the shape every wiring starts as before anything escalates it.
    #[must_use]
    pub fn settled(connection: String, state: State) -> Self {
        Self {
            connection,
            state,
            severity: Severity::Informational,
        }
    }

    /// Escalate this connection to a warning, naming what broke and how to fix it.
    /// Applied to a drift a later check found to break the stack.
    pub fn escalate(&mut self, breakage: String, remediation: String) {
        self.severity = Severity::Warning {
            breakage,
            remediation,
        };
    }
}

/// Whether a pass could assess drift — whether it had the record of what lemonfiber
/// last wrote to compare against.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Assessment {
    /// The expected-state record was read, or genuinely absent as on a first seed,
    /// so each connection was judged against it.
    #[default]
    Assessed,
    /// The expected-state record was there but could not be read — lost. Drift could
    /// not be judged this pass, and re-baselining from the current state is offered.
    Unassessable,
}

/// What a seed pass amounted to.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[schemars(rename = "SeedReport")]
pub struct Report {
    /// Every connection attempted, and how each turned out.
    pub wirings: Vec<Wiring>,
    /// Whether drift could be assessed, or the expected-state record was lost.
    pub assessment: Assessment,
    /// Whether this pass only said what it would do.
    ///
    /// A flag rather than a second shape, because every connection above means the
    /// same thing either way: what the service holds is what it holds, and what
    /// lemonfiber would write is what it would write. What changes is that two of the
    /// states are reachable only here, and that the last line of the report is an
    /// instruction to run it for real rather than to run it again.
    pub rehearsed: bool,
    /// Services this pass could not wire because it cannot speak to them, each with
    /// why.
    ///
    /// Not wirings, because nothing was attempted and a wiring says how an attempt
    /// turned out. Not absences either, which is the point: a pass that skipped a
    /// service declaring an API shape this build does not speak used to say nothing at
    /// all, and an operator who wrote that declaration had no way to tell it from a
    /// service lemonfiber had simply forgotten.
    #[serde(default)]
    pub unsupported: Vec<crate::model::UnsupportedReport>,
}

impl Report {
    /// Whether every connection is settled, so nothing needs a re-run.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.wirings.iter().all(|wiring| wiring.state.is_settled())
    }

    /// The connections not settled — the skipped, the failed and the refused —
    /// so the report says exactly what is left rather than only that something is.
    #[must_use]
    pub fn outstanding(&self) -> Vec<&Wiring> {
        self.wirings
            .iter()
            .filter(|wiring| !wiring.state.is_settled())
            .collect()
    }

    /// The connections a re-run will not lift — refused by policy, or a conflict
    /// lemonfiber will not resolve on its own — unlike the skipped and failed that
    /// [`Self::outstanding`] also holds. Named apart so the operator is told to
    /// resolve the clash rather than merely to run again, and so a script can tell
    /// "fix your config" from "retry".
    #[must_use]
    pub fn blocked(&self) -> Vec<&Wiring> {
        self.wirings
            .iter()
            .filter(|wiring| {
                matches!(
                    wiring.state,
                    State::Refused { .. } | State::Conflicted { .. }
                )
            })
            .collect()
    }

    /// The connections a drift broke — reported at warning severity, each naming
    /// what broke and its remediation. Drawn out on their own so a surface can raise
    /// the ones the operator must act on above the ordinary informational drift the
    /// rest of the report carries.
    #[must_use]
    pub fn warnings(&self) -> Vec<&Wiring> {
        self.wirings
            .iter()
            .filter(|wiring| wiring.severity.is_warning())
            .collect()
    }
}

/// A failure as the state it leaves a connection in: a service not answering is
/// skipped and retried; one that refuses is a failure, carrying its own words; one
/// that does not serve this build's API version is refused, since a re-run against
/// the same service will not resolve it — the operator must align the versions.
pub(super) fn unreached(failure: &Failure) -> State {
    match failure {
        Failure::Unavailable { .. } => State::Skipped {
            reason: "the service is not answering; a later run will complete it".to_owned(),
        },
        Failure::Unauthorised { service } => State::Failed {
            detail: format!("{service} refused the credential"),
        },
        Failure::Refused { detail, .. } => State::Failed {
            detail: detail.clone(),
        },
        Failure::Unsupported { service, detail } => State::Refused {
            reason: format!("{service} does not serve the API version this build speaks: {detail}"),
        },
    }
}

/// What a read that would have needed a session comes to on a pass that opens none.
///
/// A rehearsal signs in to nothing: a session is state left on somebody else's service,
/// and the rule a rehearsed seed keeps is that it issues nothing but reads. So an
/// unauthorised answer to one of the reads made as the owner is this run declining to
/// open a session, not the service refusing a credential — and reporting the second
/// would put a fault in front of an operator that nobody has.
///
/// What it reports instead is that it could not tell. Naming what a real run would
/// write here would be a guess: what the service already holds is exactly what the
/// session would have shown, and a rehearsal that guesses at the half it could not see
/// has done its one job wrong. Every other failure is the failure it is, in both
/// tenses.
pub(super) fn unread(failure: &Failure, rehearsing: bool) -> State {
    if rehearsing && matches!(failure, Failure::Unauthorised { .. }) {
        return State::Skipped {
            reason: WITHOUT_A_SESSION.to_owned(),
        };
    }
    unreached(failure)
}

/// Why a rehearsal has nothing to say about a connection it reads as the owner.
const WITHOUT_A_SESSION: &str = "reading this needs a session with the service, and \
     opening one is a write that a run only saying what it would do does not make; a \
     real run signs in and reports what it found";

/// A service's existing resources, observed once — or, where it could not be
/// reached, every wanted connection as the state that unreachability leaves it in.
/// Each driver opens the same way; this is that opening, so the three do not each
/// repeat it: `Ok` yields what the service holds, `Err` yields the wirings to
/// return in its place, described by the caller's own namer.
pub(super) fn observe_or_skip<T, W>(
    observed: Result<Vec<T>, Failure>,
    wanted: &[W],
    describe: impl Fn(&W) -> String,
) -> Result<Vec<T>, Vec<Wiring>> {
    observed.map_err(|failure| {
        let state = unreached(&failure);
        wanted
            .iter()
            .map(|want| Wiring::settled(describe(want), state.clone()))
            .collect()
    })
}

/// [`observe_or_skip`] for a read made as the owner, which a pass that opens no session
/// cannot make.
///
/// The same opening, with the unauthorised answer read through [`unread`] rather than
/// [`unreached`] — so a rehearsal says it could not be told instead of naming a
/// credential fault the operator would go looking for and not find.
pub(super) fn observe_or_untold<T, W>(
    observed: Result<Vec<T>, Failure>,
    wanted: &[W],
    describe: impl Fn(&W) -> String,
    rehearsing: bool,
) -> Result<Vec<T>, Vec<Wiring>> {
    observed.map_err(|failure| {
        let state = unread(&failure, rehearsing);
        wanted
            .iter()
            .map(|want| Wiring::settled(describe(want), state.clone()))
            .collect()
    })
}

/// Record the expected state a client's wiring leaves. An ordinary pass records the
/// category lemonfiber set for a client it wrote (`Wired`) or found already at that
/// value (`AlreadyWired`). A reset records only the value a revert actually wrote back
/// — a landed revert is the sole `Wired` a reset produces, since a non-drift it leaves
/// is `AlreadyWired` and must keep whatever the loaded baseline held rather than
/// re-recording a value for a client it did not touch (or, when absent, does not
/// exist). A value an adopt pass took on records what the service holds, marked
/// adopted. Everything else leaves the baseline as it was.
pub(super) fn record_outcome(
    baselines: &mut Baselines<'_>,
    service: &str,
    want: &DownloadClient,
    state: &State,
    adopting: bool,
    found: Option<&String>,
    at: &str,
) {
    let field = client_field(want);
    let records_ours = if baselines.reset {
        matches!(state, State::Wired)
    } else {
        matches!(state, State::Wired | State::AlreadyWired)
    };
    if records_ours {
        baselines
            .records
            .record(service, &field, &want.category.value, at);
    } else if adopting {
        if let Some(value) = found {
            baselines.records.adopt(service, &field, value, at);
        }
    }
}
