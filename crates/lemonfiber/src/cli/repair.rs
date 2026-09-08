//! What a repair is allowed to do, and what it is allowed to undo.
//!
//! Two carriers and the one rule between them: a run states which way it is going
//! before anything reads either flag.

use clap::Args;

/// What `doctor` was asked to put right.
///
/// Declared here beside the subcommand rather than in the surface that reads them, as every
/// other set of flags is — a flag added to one list and not the other is a flag that
/// silently does nothing.
#[derive(Debug, Args)]
pub struct Mending {
    /// Changing things forward, and what was agreed to first.
    #[command(flatten)]
    pub fixing: Fixing,
    /// Put back what the last repair changed.
    ///
    /// Asked for the same way a repair is, because it is the same errand read
    /// backwards. It reverses that one repair and nothing else: the wiring lemonfiber
    /// seeded and the choices your first run wrote are left where they are.
    #[arg(long, conflicts_with = "fix")]
    pub undo: bool,
}

impl Mending {
    /// Whether this run was asked to change anything at all, forwards or back.
    ///
    /// Asked as one question so the caller deciding between looking and acting does not
    /// have to know which combination of flags amounts to acting.
    #[must_use]
    pub fn acts(&self) -> bool {
        self.fixing.fix || self.undo
    }
}

/// How much of the putting-right was agreed to in advance.
///
/// Apart from `--undo` because these are two errands rather than four settings: the three
/// here describe one run that changes things forward, and each is meaningless without the
/// first of them.
#[derive(Debug, Args)]
pub struct Fixing {
    /// Offer to put right what lemonfiber can, asking about each first.
    ///
    /// A plain run only looks. This one says what each repair would do and what else
    /// changes if it does, and waits to be told.
    #[arg(long)]
    pub fix: bool,
    /// Carry the repairs out without asking, having decided in advance.
    #[arg(long, requires = "fix")]
    pub yes: bool,
    /// Include the checks that disturb the running system while repairing.
    ///
    /// Named apart from the field it sits beside: `doctor` already has a `--disruptive`,
    /// and clap keys an argument by the field name unless told otherwise — so two flags
    /// that read differently on the command line would be one argument underneath.
    #[arg(id = "fix-disruptive", long = "fix-disruptive", requires = "fix")]
    pub disruptive: bool,
}

/// What a diagnosis was asked for, as the command line spells it.
///
/// Flattened into one carrier rather than four fields on the request, for the reason
/// the other carriers here have one: `cli.rs` is the subcommand tree, and the flags of
/// one branch are a detail of that branch.
#[derive(Debug, Args)]
pub struct RawDoctor {
    /// Run one category of check, such as `vpn`, or one check by the name a
    /// finding gives it, such as `vpn.killswitch`.
    #[arg(long, value_name = "CATEGORY_OR_CHECK", conflicts_with = "fix")]
    pub only: Option<String>,
    /// Include the checks that disturb the running system.
    #[arg(long)]
    pub disruptive: bool,
    /// Answer a warning about a choice — `vpn.unprotected`, say — so it stops
    /// leading. Only something this run warns about can be answered.
    #[arg(long, value_name = "CHECK", conflicts_with = "fix")]
    pub accept: Option<String>,
    /// How much putting-right this run was given consent for.
    #[command(flatten)]
    pub mending: Mending,
}
