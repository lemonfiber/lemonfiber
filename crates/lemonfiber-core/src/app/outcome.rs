//! What dispatching produced, and how it is written down.
//!
//! Its own file because it answers a different question from the module beside it:
//! `app.rs` is how a command is carried out, and this is what one comes back as. The
//! two move for different reasons — a new command adds an arm there, a new kind of
//! answer adds a variant here — and each was pushing the other over the line cap.
//!
//! The variants are tagged by name rather than told apart by which field is present,
//! because a surface that is not in this process reads them back.

use crate::glossary::{Term, Vocabulary};
use crate::stack::closure::Plan;

use super::{archives, backup, repair, restore, support, update};

use crate::model::{
    kind::{self, Kind},
    AdoptReport, AlertReport, BesideReport, ConfigReport, DoctorReport, Envelope, FormsReport,
    FrontDoorReport, HistoryReport, HostingReport, HouseholdReport, ImportReport, LifecycleReport,
    MigrationReport, MusicReport, QualityReport, ReplaceReport, ResetReport, StatusReport,
    StuckReport, SupervisionReport, TraceReport, UpgradeReport, VersionReport, WalkthroughReport,
    WizardReport,
};

/// Declares [`Outcome`] from one list: each variant, the payload it carries and the
/// `kind` its envelope is written under.
///
/// The enum, its envelope, how it serialises, the kinds it can be written under and
/// the contract's shape for each are all generated from that list, so a new answer is
/// one line here rather than an arm in five tables that have to agree.
macro_rules! outcomes {
    ($($(#[doc = $doc:literal])* $variant:ident($payload:ty) => $kind:ident,)*) => {
        /// What dispatching produced.
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub enum Outcome {
            $($(#[doc = $doc])* $variant($payload),)*
        }

        impl Outcome {
            /// Every kind an outcome is written under, in declaration order.
            pub const KINDS: &'static [Kind] = &[$(kind::$kind,)*];

            /// The kind this outcome is written under.
            #[must_use]
            pub const fn kind(&self) -> Kind {
                match self {
                    $(Self::$variant(_) => kind::$kind,)*
                }
            }

            /// The schema of the envelope carrying each kind of outcome, handed to
            /// `each` one kind at a time.
            pub fn schemas(mut each: impl FnMut(Kind, schemars::Schema)) {
                $(each(kind::$kind, schemars::schema_for!(Envelope<$payload>));)*
            }
        }

        impl serde::Serialize for Outcome {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                match self {
                    $(Self::$variant(payload) => payload.serialize(serializer),)*
                }
            }
        }
    };
}

outcomes! {
    /// The answer to [`Command::Version`].
    Version(VersionReport) => VERSION,
    /// The answer to [`Command::Forms`].
    Forms(FormsReport) => FORMS,
    /// The answer to [`Command::Preview`].
    Preview(Plan) => PREVIEW,
    /// What a lifecycle command did, or would have done.
    Lifecycle(LifecycleReport) => LIFECYCLE,
    /// The answer to a configuration command.
    Config(ConfigReport) => CONFIG,
    /// The quality choice, what it means, and what a command did with it.
    Quality(QualityReport) => QUALITY,
    /// What the operator is told about, and what changing it came to.
    Alerts(AlertReport) => ALERTS,
    /// Everything lemonfiber changed, and how far each could be put back.
    History(HistoryReport) => HISTORY,
    /// What is already on this machine, before anything is proposed.
    Migration(MigrationReport) => MIGRATION,
    /// What adopting a setup already here came to, or would come to.
    Adoption(AdoptReport) => ADOPTION,
    /// What standing beside a setup already here came to, or would come to.
    Beside(BesideReport) => BESIDE,
    /// What standing in place of a setup already here came to, or would come to.
    Replacement(ReplaceReport) => REPLACEMENT,
    /// What copying an operator's own records across came to, or would come to.
    Import(ImportReport) => IMPORT,
    /// What upgrading existing content did, or would do, and its stated cost.
    Upgrade(UpgradeReport) => UPGRADE,
    /// The music format chosen, and what became of applying it.
    Music(MusicReport) => MUSIC,
    /// Where one item is in the pipeline.
    Trace(TraceReport) => TRACE,
    /// What the household asked for, member by member.
    Household(HouseholdReport) => HOUSEHOLD,
    /// What one member can watch, as the media server answers it for them.
    Held(crate::model::HeldReport) => HELD,
    /// What this machine keeps running for lemonfiber, and what a change to it did.
    Hosting(HostingReport) => HOSTING,
    /// The one address to hand somebody who lives here.
    FrontDoor(FrontDoorReport) => FRONT_DOOR,
    /// The items whose downloads are stuck, each linkable to its trace.
    Stuck(StuckReport) => STUCK,
    /// What one of this product's words means.
    Word(Term) => WORD,
    /// Every word this product explains.
    Glossary(Vocabulary) => GLOSSARY,
    /// Which app to use on which device.
    Clients(crate::clients::Guidance) => CLIENTS,
    /// An account offered to somebody in the house.
    Invited(crate::model::Invitation) => INVITATION,
    /// Somebody taken out of the household, or what taking them would cost.
    Removed(crate::model::HouseholdRemoval) => REMOVAL,
    /// What each service in this stack is for, and what became of the ones that went.
    Catalogue(crate::model::CatalogueReport) => CATALOGUE,
    /// What this stack wires to what, and what nothing fills.
    Wiring(crate::model::WiringReport) => WIRING,
    /// A change of which service fills a capability, and what it costs.
    Substituted(crate::model::SubstitutionReport) => SUBSTITUTION,
    /// Everything that leaves this machine, and what refusing each of them costs.
    Outbound(crate::outbound::Leaving) => OUTBOUND,
    /// Every plugin installed on this machine, and what installing one came to.
    Plugins(crate::plugin::Installs) => PLUGINS,
    /// Where every service in this stack comes from, and under what licence.
    Provenance(crate::model::ProvenanceReport) => PROVENANCE,
    /// Every credential this stack holds, and what became of acting on one.
    Credentials(crate::credential::Inventory) => CREDENTIALS,
    /// Everything this machine keeps of lemonfiber's, and what became of it.
    Stored(crate::stored::Stored) => STORED,
    /// Where this copy of lemonfiber stands, and what moving it would come to.
    SelfUpdate(crate::model::UpdateReport) => SELF_UPDATE,
    /// Where the disk stands, where the room went, and what could be got back.
    Space(crate::space::Reckoning) => SPACE,
    /// What letting one completed download go would cost, and what became of it.
    Letting(crate::space::Letting) => STOP_SEEDING,
    /// How the line is shared, what that costs, and whether the clients keep to it.
    Bandwidth(crate::bandwidth::Sharing) => BANDWIDTH,
    /// What each service is doing.
    Status(StatusReport) => STATUS,
    /// What the diagnostic checks found.
    Doctor(DoctorReport) => DOCTOR,
    /// What could be put right, and what became of the ones agreed to.
    Repair(repair::Report) => REPAIR,
    /// What putting back the last repair came to.
    Undo(repair::Reversal) => UNDO,
    /// What seeding wired, and what it left for a re-run.
    Seed(crate::seed::Report) => SEED,
    /// What a full reset did, or would do — the operator edits reverted to lemonfiber's.
    Reset(ResetReport) => RESET,
    /// What taking lemonfiber off this machine would come to, or came to.
    Uninstall(crate::uninstall::Uninstall) => UNINSTALL,
    /// Where setup stands, and what it is still asking for.
    Wizard(WizardReport) => WIZARD,
    /// What moving the stack onto this build's pins would change, or came to.
    Update(update::Report) => UPDATE,
    /// Where a backup archive was written, and what it covers.
    Backup(backup::Report) => BACKUP,
    /// What a support bundle would hold, or where one went.
    Support(support::Bundle) => BUNDLE,
    /// The backup archives this machine has kept.
    Archives(archives::Listing) => ARCHIVES,
    /// What a restore would overwrite, or what it put back.
    Restore(restore::Restoration) => RESTORE,
    /// How a guard ended, and whether it got the services stopped.
    Watch(SupervisionReport) => WATCH,
    /// How far a walk got, and what it proved.
    Walkthrough(WalkthroughReport) => WALKTHROUGH,
}

impl Outcome {
    /// This outcome in the envelope every surface writes it in.
    #[must_use]
    pub fn envelope(self) -> Envelope<Self> {
        Envelope::new(self.kind(), self)
    }
}
