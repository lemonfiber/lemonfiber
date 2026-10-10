//! Every problem code lemonfiber can raise, declared in one place.
//!
//! A code is a family and a number. It is never renumbered and never recycled, so an
//! operator who searches for one finds the same answer a year later. Each family is a
//! module here, and every crate names a code through them, so a code exists exactly
//! where these lists say it does.
//!
//! A code is declared with everything said about it everywhere: how much it matters,
//! the status the web API answers with, what a run ending on it leaves with, the
//! version it appeared in, and what it means and what to do about it as a reader is
//! told who meets it with no context. A [`crate::Problem`] reads its severity from
//! here rather than being given one, so a raise site cannot disagree with what is
//! published. `contract/codes.json` is written from [`every_declared`], and the
//! reference beside it from that file.
//!
//! `WIRE` and `WIRING` are both the wiring domain's: the two prefixes were published
//! apart, and a published code keeps its spelling.

use crate::{Code, Severity};

/// What a run that ends on a problem leaves with, for a script that reads nothing else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Leaves {
    /// A general failure.
    Failure,
    /// Something outside lemonfiber has to be fixed before it can act.
    Preflight,
    /// Something the operator wrote was refused.
    Validation,
    /// Started, and a service never became usable.
    NeverSettled,
}

impl Leaves {
    /// The exit code a run leaving with this ends on.
    ///
    /// `2` is not here: it is a flag the operator gave that could not be understood,
    /// which is refused before any code is raised.
    #[must_use]
    pub const fn exit(self) -> u8 {
        match self {
            Self::Failure => 1,
            Self::Preflight => 3,
            Self::NeverSettled => 4,
            Self::Validation => 5,
        }
    }
}

/// Declares one family's codes, each with what is published about it, and the list of
/// them. Invoked once in each family's file.
macro_rules! codes {
    ($(
        $(#[doc = $doc:literal])*
        $name:ident = $id:literal {
            severity: $severity:ident,
            status: $status:literal,
            $(leaves: $leaves:ident,)?
            since: $since:literal,
            meaning: $meaning:literal,
            remedy: $remedy:literal $(,)?
        }
    )*) => {
        use crate::Code;

        $($(#[doc = $doc])* pub const $name: Code = Code::declared($id);)*

        /// Every code this family declares, with what is published about each.
        pub(super) const DECLARED: &[super::Declared] = &[$(super::Declared {
            code: $name,
            name: stringify!($name),
            said: concat!($($doc),*),
            severity: crate::Severity::$severity,
            status: $status,
            leaves: codes!(@leaves $($leaves)?),
            since: $since,
            meaning: $meaning,
            remedy: $remedy,
        },)*];
    };
    (@leaves) => { super::Leaves::Failure };
    (@leaves $leaves:ident) => { super::Leaves::$leaves };
}

/// Declares each family's module, with the line saying what it covers, and the list
/// of every family.
macro_rules! families {
    ($($family:ident => $prefix:literal: $covers:literal,)*) => {
        $(
            #[doc = concat!("The `", $prefix, "` codes: ", $covers, ".")]
            pub mod $family;
        )*

        /// Every family, its prefix and what it covers, with the codes it declares.
        const FAMILIES: &[Family] = &[$(Family {
            prefix: $prefix,
            covers: $covers,
            declared: $family::DECLARED,
        },)*];
    };
}

families! {
    ack => "ACK": "answering a warning",
    admit => "ADMIT": "who the web interface lets in",
    ask => "ASK": "putting a request to the web surface",
    backup => "BACKUP": "capturing your configuration",
    bind => "BIND": "where the stack is actually listening",
    bundle => "BUNDLE": "the support bundle",
    config => "CONFIG": "your settings",
    cred => "CRED": "credentials a service refuses",
    decline => "DECLINE": "the service that answers an invitation's decline",
    diag => "DIAG": "narrowing a diagnosis",
    docker => "DOCKER": "talking to the engine",
    env => "ENV": "the container engine",
    form => "FORM": "choosing what to run",
    gate => "GATE": "the request gate",
    gone => "GONE": "taking lemonfiber off this machine",
    handoff => "HANDOFF": "pointing somebody's device at the stack",
    host => "HOST": "keeping a command running without a terminal",
    invite => "INVITE": "offering somebody an account",
    kept => "KEPT": "what lemonfiber keeps here",
    key => "KEY": "keys another program reaches the stack with",
    library => "LIBRARY": "what the media server holds",
    life => "LIFE": "starting and stopping",
    migrate => "MIGRATE": "taking over a setup already here",
    pair => "PAIR": "pairing a phone with the stack",
    play => "PLAY": "playing what the household holds",
    plugin => "PLUGIN": "installing and running plugins",
    proc => "PROC": "the program underneath",
    provider => "PROVIDER": "accounts and indexers",
    qual => "QUAL": "quality against what is available",
    quota => "QUOTA": "what the household may ask for",
    rate => "RATE": "holding the stack to a share of the line",
    read => "READ": "asking the web surface a question",
    rehearse => "REHEARSE": "asking what a command would do",
    reissue => "REISSUE": "letting somebody set a new password",
    remove => "REMOVE": "taking somebody out of the household",
    repair => "REPAIR": "putting right what the doctor found",
    restore => "RESTORE": "putting configuration back",
    seed => "SEED": "wiring the services together",
    serve => "SERVE": "the web surface",
    setup => "SETUP": "the first run",
    space => "SPACE": "the disk, and letting a download go",
    stack => "STACK": "the stack description",
    storage => "STORAGE": "the data location",
    telling => "TELLING": "what the household is told about",
    tui => "TUI": "the terminal interface",
    undo => "UNDO": "putting a run back",
    update => "UPDATE": "moving the stack onto newer versions",
    vpn => "VPN": "traffic leaving the tunnel",
    watch => "WATCH": "guarding the data location",
    wire => "WIRE": "choosing what fills a capability",
    wiring => "WIRING": "drift between services",
    word => "WORD": "the glossary",
}

/// One family as it is declared: its prefix, what it covers and its codes.
#[derive(Debug, Clone, Copy)]
pub struct Family {
    prefix: &'static str,
    covers: &'static str,
    declared: &'static [Declared],
}

impl Family {
    /// The prefix every code of the family is spelled with.
    #[must_use]
    pub const fn prefix(self) -> &'static str {
        self.prefix
    }

    /// What the family covers, as a phrase.
    #[must_use]
    pub const fn covers(self) -> &'static str {
        self.covers
    }

    /// The family's codes, in number order.
    #[must_use]
    pub fn declared(self) -> Vec<Declared> {
        let mut declared = self.declared.to_vec();
        declared.sort_by_key(|one| ordering(one.code.as_str()));
        declared
    }
}

/// One code as it is declared, with everything published about it.
///
/// Read by whatever has to publish a code rather than only raise it, and by a problem
/// for the severity it carries. The contract lists the codes a refusal carries, and a
/// client generating a value per code needs a name to give each value and a sentence
/// to document it with: the ones written here, so that a published name cannot drift
/// from the declared one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Declared {
    code: Code,
    name: &'static str,
    said: &'static str,
    severity: Severity,
    status: u16,
    leaves: Leaves,
    since: &'static str,
    meaning: &'static str,
    remedy: &'static str,
}

impl Declared {
    /// The code itself.
    #[must_use]
    pub const fn code(self) -> Code {
        self.code
    }

    /// The name it is declared under, as a constant is spelled.
    #[must_use]
    pub const fn name(self) -> &'static str {
        self.name
    }

    /// The line written above it, as one sentence.
    #[must_use]
    pub fn description(self) -> &'static str {
        self.said.trim()
    }

    /// How much a problem carrying it matters.
    #[must_use]
    pub const fn severity(self) -> Severity {
        self.severity
    }

    /// The HTTP status the web API answers a problem carrying it with.
    #[must_use]
    pub const fn status(self) -> u16 {
        self.status
    }

    /// What a run ending on it leaves with.
    #[must_use]
    pub const fn leaves(self) -> Leaves {
        self.leaves
    }

    /// The version of lemonfiber it first appeared in.
    #[must_use]
    pub const fn since(self) -> &'static str {
        self.since
    }

    /// What it means, for a reader shown the code and nothing else.
    #[must_use]
    pub const fn meaning(self) -> &'static str {
        self.meaning
    }

    /// What to do about it, for the same reader.
    #[must_use]
    pub const fn remedy(self) -> &'static str {
        self.remedy
    }
}

/// Every family, in prefix order.
#[must_use]
pub const fn families() -> &'static [Family] {
    FAMILIES
}

/// How one code is declared, or nothing where no family declares it.
#[must_use]
pub fn declared(code: Code) -> Option<Declared> {
    FAMILIES
        .iter()
        .flat_map(|family| family.declared)
        .find(|declared| declared.code == code)
        .copied()
}

/// Every code there is as it is declared, family by family, in number order.
#[must_use]
pub fn every_declared() -> Vec<Declared> {
    FAMILIES
        .iter()
        .flat_map(|family| family.declared())
        .collect()
}

/// Every code there is, family by family, in number order.
#[must_use]
pub fn every() -> Vec<Code> {
    every_declared().into_iter().map(Declared::code).collect()
}

/// Numbers no family declares and none may declare again.
///
/// Each was published with a meaning and nothing raises it. An operator who searches for
/// one must never land on a different problem that was given its number.
pub const RETIRED: &[&str] = &["QUOTA-3"];

/// Where a code sorts: its family, then its number.
fn ordering(code: &str) -> (&str, u32) {
    let (family, number) = code.rsplit_once('-').unwrap_or((code, ""));
    (family, number.parse().unwrap_or_default())
}

/// What a run that ends on `code` leaves with: a general failure where nothing
/// declares it.
#[must_use]
pub fn leaves(code: Code) -> Leaves {
    declared(code).map_or(Leaves::Failure, Declared::leaves)
}

#[cfg(test)]
mod tests;
