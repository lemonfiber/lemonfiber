//! What an envelope calls itself.
//!
//! A `kind` is named here and nowhere else. Naming it at the emit site and again
//! in the contract is two literals that can drift, and the drift is a contract
//! describing a kind nobody emits.
//!
//! Nowhere else is enforced by the type rather than by a reader: a kind cannot be
//! built outside this module, so a call site cannot spell one out. It was a plain
//! string until two kinds reached the wire without a schema — and neither the
//! contract check nor the emitters could see it, because both read [`ALL`] and a
//! spelled-out kind never reaches [`ALL`].

/// What an envelope calls itself, so a consumer can branch before parsing `data`.
///
/// Every value is a constant below. The field is private and there is no public
/// constructor, so the only kinds that exist are the ones [`ALL`] holds and the
/// contract describes.
///
/// It writes itself as the bare string it wraps, so the wrapper is a rule about
/// the source and changes nothing a caller reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(transparent)]
pub struct Kind(&'static str);

impl Kind {
    /// The kind as it is written on the wire.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl std::fmt::Display for Kind {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str(self.0)
    }
}

/// The settings asked about, and what a change did to them.
pub const CONFIG: Kind = Kind("config");
/// One moment of what the stack is doing, as the dashboard assembles it.
pub const DASHBOARD: Kind = Kind("dashboard");
/// What the diagnostic checks found.
pub const DOCTOR: Kind = Kind("doctor");
/// A command could not do what was asked.
pub const ERROR: Kind = Kind("error");
/// Every form the stack declares.
pub const FORMS: Kind = Kind("forms");
/// What the household asked for, member by member.
pub const HOUSEHOLD: Kind = Kind("household");
/// The name given to work that outlives the request that started it.
pub const JOB: Kind = Kind("job");
/// What a lifecycle command did, or would have done.
pub const LIFECYCLE: Kind = Kind("lifecycle");
/// One line of a service's log.
pub const LOG: Kind = Kind("log");
/// The music format chosen, and what became of applying it.
pub const MUSIC: Kind = Kind("music");
/// What starting or stopping would do, before it is done.
pub const PREVIEW: Kind = Kind("preview");
/// One line the container engine wrote while pulling images.
pub const PULL: Kind = Kind("pull");
/// The quality choice, what it means, and what a command did with it.
pub const QUALITY: Kind = Kind("quality");
/// What a full reset did, or would do.
pub const RESET: Kind = Kind("reset");
/// What seeding wired, and what it left for a re-run.
pub const SEED: Kind = Kind("seed");
/// What setup settled on.
pub const SETUP: Kind = Kind("setup");
/// One line the container engine wrote while starting services.
pub const START: Kind = Kind("start");
/// What each service is doing.
pub const STATUS: Kind = Kind("status");
/// The items whose downloads are stuck.
pub const STUCK: Kind = Kind("stuck");
/// Where one item is in the pipeline.
pub const TRACE: Kind = Kind("trace");
/// What upgrading existing content did, or would do.
pub const UPGRADE: Kind = Kind("upgrade");
/// The versions in play: the binary, and the stack it operates.
pub const VERSION: Kind = Kind("version");
/// A walkthrough's outcome.
pub const WALKTHROUGH: Kind = Kind("walkthrough");
/// A supervision run's findings.
pub const WATCH: Kind = Kind("watch");
/// Where a setup run stands, and what it is still asking for.
pub const WIZARD: Kind = Kind("wizard");
/// One glossary term.
pub const WORD: Kind = Kind("word");

/// Every kind, so the contract cannot describe one that is never emitted.
pub const ALL: &[Kind] = &[
    CONFIG,
    DASHBOARD,
    DOCTOR,
    ERROR,
    FORMS,
    HOUSEHOLD,
    JOB,
    LIFECYCLE,
    LOG,
    MUSIC,
    PREVIEW,
    PULL,
    QUALITY,
    RESET,
    SEED,
    SETUP,
    START,
    STATUS,
    STUCK,
    TRACE,
    UPGRADE,
    VERSION,
    WALKTHROUGH,
    WATCH,
    WIZARD,
    WORD,
];

#[cfg(test)]
mod tests {
    use super::ALL;

    /// The part of this file that ships, which is everything before these tests.
    ///
    /// Read rather than opened: the text is compiled in, so there is no file that
    /// could be missing and no failure to handle.
    fn shipped() -> &'static str {
        let mut parts = include_str!("kind.rs").split("#[cfg(test)]");
        parts.next().unwrap_or_default()
    }

    /// The kinds declared above, in the order they are written.
    fn declared() -> Vec<&'static str> {
        shipped()
            .lines()
            .filter_map(|line| line.strip_prefix("pub const "))
            .filter_map(|rest| rest.split_once(": Kind = "))
            .map(|(name, _)| name)
            .collect()
    }

    /// The kinds `ALL` lists, read from the block it is written as.
    fn listed() -> Vec<&'static str> {
        shipped()
            .split("] = &[")
            .skip(1)
            .flat_map(|rest| rest.split("];").take(1))
            .flat_map(str::lines)
            .map(str::trim)
            .filter_map(|line| line.strip_suffix(','))
            .collect()
    }

    /// A kind that exists must be one the contract reads.
    ///
    /// The type stops a kind being spelled out at a call site. It does not stop one
    /// being declared here and left out of `ALL`, which lands in the same place: it
    /// is emitted, it is never described, and the check that the contract and the
    /// emitters agree cannot see it, because both of its halves read `ALL`.
    #[test]
    fn every_kind_declared_here_is_one_the_contract_reads() {
        let declared = declared();
        assert!(
            !declared.is_empty(),
            "the scanner read no declarations, so it is reading the wrong text"
        );
        assert_eq!(declared, listed());
    }

    /// And the block that was read is the one that was compiled.
    ///
    /// Without this the two readings above could agree with each other about a
    /// region of text that is not the list anything uses.
    #[test]
    fn the_list_that_was_read_is_the_list_that_is_compiled() {
        assert_eq!(declared().len(), ALL.len());
    }
}
