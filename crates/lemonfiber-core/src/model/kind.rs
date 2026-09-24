//! What an envelope calls itself.
//!
//! A `kind` is named here and nowhere else. Naming it at the emit site and again
//! in the contract is two literals that can drift, and the drift is a contract
//! describing a kind nobody emits.
//!
//! Nowhere else is enforced by the type rather than by a reader: a kind cannot be
//! built outside this module, so a call site cannot spell one out. Each kind is
//! declared once, in the list below, and [`ALL`] is generated from that list, so a
//! kind cannot exist without the contract reading it.

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
/// Declares each kind and [`ALL`] from one list.
macro_rules! kinds {
    ($($(#[doc = $doc:literal])* $name:ident = $wire:literal,)*) => {
        $($(#[doc = $doc])* pub const $name: Kind = Kind($wire);)*

        /// Every kind there is, in the order the list declares them.
        pub const ALL: &[Kind] = &[$($name,)*];
    };
}

kinds! {
    /// A session opened by proving the operator's password.
    ADMISSION = "admission",
    /// What adopting a setup already on this machine came to.
    ADOPTION = "adoption",
    /// What the operator will be told about, and what changing it came to.
    ALERTS = "alerts",
    /// The backup archives this machine has kept.
    ARCHIVES = "archives",
    /// Where a backup archive was written, and what it covers.
    BACKUP = "backup",
    /// How the line is shared, what that costs, and whether the clients keep to it.
    BANDWIDTH = "bandwidth",
    /// What standing lemonfiber beside an existing setup came to.
    BESIDE = "beside",
    /// What a support bundle would hold, or where one went.
    BUNDLE = "bundle",
    /// What each service in the stack is for, and what became of the ones that went.
    CATALOGUE = "catalogue",
    /// The settings asked about, and what a change did to them.
    CONFIG = "config",
    /// Every credential this stack holds, and what became of acting on one.
    CREDENTIALS = "credentials",
    /// One moment of what the stack is doing, as the dashboard assembles it.
    DASHBOARD = "dashboard",
    /// What the diagnostic checks found.
    DOCTOR = "doctor",
    /// A command could not do what was asked.
    ERROR = "error",
    /// Every form the stack declares.
    FORMS = "forms",
    /// The one address to hand somebody who lives here.
    FRONT_DOOR = "front-door",
    /// Every word this product explains.
    GLOSSARY = "glossary",
    /// Which app to use on which device.
    CLIENTS = "clients",
    /// What this machine keeps running for lemonfiber.
    HOSTING = "hosting",
    /// Everything lemonfiber changed, and how far each could be put back.
    HISTORY = "history",
    /// What the household asked for, member by member.
    HOUSEHOLD = "household",
    /// What one member can watch, as the media server answers it for them.
    HELD = "held",
    /// What copying an operator's own records across came to.
    IMPORT = "import",
    /// An account offered to somebody in the house.
    INVITATION = "invitation",
    /// Somebody taken out of the household, or what taking them would cost.
    REMOVAL = "removal",
    /// The name given to work that outlives the request that started it.
    JOB = "job",
    /// What a lifecycle command did, or would have done.
    LIFECYCLE = "lifecycle",
    /// One line of a service's log.
    LOG = "log",
    /// What is already on this machine, before anything is proposed.
    MIGRATION = "migration",
    /// The music format chosen, and what became of applying it.
    MUSIC = "music",
    /// Everything that leaves this machine, and what the stack's own services reach.
    OUTBOUND = "outbound",
    /// Every plugin installed on this machine, and what installing one came to.
    PLUGINS = "plugins",
    /// What starting or stopping would do, before it is done.
    PREVIEW = "preview",
    /// Where each service in the stack comes from, and under what licence.
    PROVENANCE = "provenance",
    /// One line the container engine wrote while pulling images.
    PULL = "pull",
    /// The quality choice, what it means, and what a command did with it.
    QUALITY = "quality",
    /// What could be put right, and what became of the ones agreed to.
    REPAIR = "repair",
    /// What standing in place of a setup already here came to.
    REPLACEMENT = "replacement",
    /// What a full reset did, or would do.
    RESET = "reset",
    /// What a restore would overwrite, or what it put back.
    RESTORE = "restore",
    /// What seeding wired, and what it left for a re-run.
    SEED = "seed",
    /// Where this copy of lemonfiber stands, and what moving it would come to.
    SELF_UPDATE = "self-update",
    /// What setup settled on.
    SETUP = "setup",
    /// Where the disk stands, where the room went, and what could be got back.
    SPACE = "space",
    /// One line said while services are starting: what the container engine wrote, or
    /// what the wait after it is still waiting for.
    START = "start",
    /// What each service is doing.
    STATUS = "status",
    /// One step of a walkthrough, said the moment it is true.
    STEP = "step",
    /// What letting one completed download go costs, and what became of letting it.
    STOP_SEEDING = "stop-seeding",
    /// Everything lemonfiber keeps on this machine, and what became of it.
    STORED = "stored",
    /// The items whose downloads are stuck.
    STUCK = "stuck",
    /// Where one item is in the pipeline.
    TRACE = "trace",
    /// What putting back the last repair came to.
    UNDO = "undo",
    /// What taking lemonfiber off this machine would come to, or came to.
    UNINSTALL = "uninstall",
    /// What moving the stack onto this build's pinned versions would change, or came to.
    UPDATE = "update",
    /// What upgrading existing content did, or would do.
    UPGRADE = "upgrade",
    /// The versions in play: the binary, and the stack it operates.
    VERSION = "version",
    /// A walkthrough's outcome.
    WALKTHROUGH = "walkthrough",
    /// A supervision run's findings.
    WATCH = "watch",
    /// Where a setup run stands, and what it is still asking for.
    WIZARD = "wizard",
    /// One glossary term.
    WORD = "word",
    /// What this stack wires to what, and how each link was settled.
    WIRING = "wiring",
    /// A change of which service fills a capability, and what it costs.
    SUBSTITUTION = "substitution",
}
