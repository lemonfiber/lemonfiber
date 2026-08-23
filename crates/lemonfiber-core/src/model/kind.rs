//! What an envelope calls itself.
//!
//! A `kind` is named here and nowhere else. Naming it at the emit site and again
//! in the contract is two literals that can drift, and the drift is a contract
//! describing a kind nobody emits.

/// The settings asked about, and what a change did to them.
pub const CONFIG: &str = "config";
/// One moment of what the stack is doing, as the dashboard assembles it.
pub const DASHBOARD: &str = "dashboard";
/// What the diagnostic checks found.
pub const DOCTOR: &str = "doctor";
/// A command could not do what was asked.
pub const ERROR: &str = "error";
/// Every form the stack declares.
pub const FORMS: &str = "forms";
/// What the household asked for, member by member.
pub const HOUSEHOLD: &str = "household";
/// What a lifecycle command did, or would have done.
pub const LIFECYCLE: &str = "lifecycle";
/// One line of a service's log.
pub const LOG: &str = "log";
/// The music format chosen, and what became of applying it.
pub const MUSIC: &str = "music";
/// What starting or stopping would do, before it is done.
pub const PREVIEW: &str = "preview";
/// One line the container engine wrote while pulling images.
pub const PULL: &str = "pull";
/// The quality choice, what it means, and what a command did with it.
pub const QUALITY: &str = "quality";
/// What a full reset did, or would do.
pub const RESET: &str = "reset";
/// What seeding wired, and what it left for a re-run.
pub const SEED: &str = "seed";
/// What setup settled on.
pub const SETUP: &str = "setup";
/// One line the container engine wrote while starting services.
pub const START: &str = "start";
/// What each service is doing.
pub const STATUS: &str = "status";
/// The items whose downloads are stuck.
pub const STUCK: &str = "stuck";
/// Where one item is in the pipeline.
pub const TRACE: &str = "trace";
/// What upgrading existing content did, or would do.
pub const UPGRADE: &str = "upgrade";
/// The versions in play: the binary, and the stack it operates.
pub const VERSION: &str = "version";
/// A walkthrough's outcome.
pub const WALKTHROUGH: &str = "walkthrough";
/// A supervision run's findings.
pub const WATCH: &str = "watch";
/// One glossary term.
pub const WORD: &str = "word";

/// Every kind, so the contract cannot describe one that is never emitted.
pub const ALL: &[&str] = &[
    CONFIG,
    DASHBOARD,
    DOCTOR,
    ERROR,
    FORMS,
    HOUSEHOLD,
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
    WORD,
];
