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

use super::{archives, backup, repair, restore, support};

use crate::model::{
    kind, AdoptReport, AlertReport, BesideReport, ConfigReport, DoctorReport, Envelope,
    FormsReport, FrontDoorReport, HostingReport, HouseholdReport, LifecycleReport, MigrationReport,
    MusicReport, QualityReport, ReplaceReport, ResetReport, StatusReport, StuckReport,
    SupervisionReport, TraceReport, UpgradeReport, VersionReport, WalkthroughReport, WizardReport,
};

/// What dispatching produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The answer to [`Command::Version`].
    Version(VersionReport),
    /// The answer to [`Command::Forms`].
    Forms(FormsReport),
    /// The answer to [`Command::Preview`].
    Preview(Plan),
    /// What a lifecycle command did, or would have done.
    Lifecycle(LifecycleReport),
    /// The answer to a configuration command.
    Config(ConfigReport),
    /// The quality choice, what it means, and what a command did with it.
    Quality(QualityReport),
    /// What the operator is told about, and what changing it came to.
    Alerts(AlertReport),
    /// What is already on this machine, before anything is proposed.
    Migration(MigrationReport),
    /// What adopting a setup already here came to, or would come to.
    Adoption(AdoptReport),
    /// What standing beside a setup already here came to, or would come to.
    Beside(BesideReport),
    /// What standing in place of a setup already here came to, or would come to.
    Replacement(ReplaceReport),
    /// What upgrading existing content did, or would do, and its stated cost.
    Upgrade(UpgradeReport),
    /// The music format chosen, and what became of applying it.
    Music(MusicReport),
    /// Where one item is in the pipeline.
    Trace(TraceReport),
    /// What the household asked for, member by member.
    Household(HouseholdReport),
    /// What this machine keeps running for lemonfiber, and what a change to it did.
    Hosting(HostingReport),
    /// The one address to hand somebody who lives here.
    FrontDoor(FrontDoorReport),
    /// The items whose downloads are stuck, each linkable to its trace.
    Stuck(StuckReport),
    /// What one of this product's words means.
    Word(Term),
    /// Every word this product explains.
    Glossary(Vocabulary),
    /// Which app to use on which device.
    Clients(crate::clients::Guidance),
    /// An account offered to somebody in the house.
    Invited(crate::model::Invitation),
    /// Somebody taken out of the household, or what taking them would cost.
    Removed(crate::model::HouseholdRemoval),
    /// Everything that leaves this machine, and what refusing each of them costs.
    Outbound(crate::outbound::Leaving),
    /// Every credential this stack holds, and what became of acting on one.
    Credentials(crate::credential::Inventory),
    /// Everything this machine keeps of lemonfiber's, and what became of it.
    Stored(crate::stored::Stored),
    /// Where the disk stands, where the room went, and what could be got back.
    Space(crate::space::Reckoning),
    /// What letting one completed download go would cost, and what became of it.
    Letting(crate::space::Letting),
    /// How the line is shared, what that costs, and whether the clients keep to it.
    Bandwidth(crate::bandwidth::Sharing),
    /// What each service is doing.
    Status(StatusReport),
    /// What the diagnostic checks found.
    Doctor(DoctorReport),
    /// What could be put right, and what became of the ones agreed to.
    Repair(repair::Report),
    /// What putting back the last repair came to.
    Undo(repair::Reversal),
    /// What seeding wired, and what it left for a re-run.
    Seed(crate::seed::Report),
    /// What a full reset did, or would do — the operator edits reverted to lemonfiber's.
    Reset(ResetReport),
    /// What taking lemonfiber off this machine would come to, or came to.
    Uninstall(crate::uninstall::Uninstall),
    /// Where setup stands, and what it is still asking for.
    Wizard(WizardReport),
    /// Where a backup archive was written, and what it covers.
    Backup(backup::Report),
    /// What a support bundle would hold, or where one went.
    Support(support::Bundle),
    /// The backup archives this machine has kept.
    Archives(archives::Listing),
    /// What a restore would overwrite, or what it put back.
    Restore(restore::Restoration),
    /// How a guard ended, and whether it got the services stopped.
    Watch(SupervisionReport),
    /// How far a walk got, and what it proved.
    Walkthrough(WalkthroughReport),
}

impl Outcome {
    /// Wrap this outcome for machine-readable output.
    #[must_use]
    pub fn envelope(self) -> Envelope<Self> {
        let kind = match self {
            Self::Version(_) => kind::VERSION,
            Self::Forms(_) => kind::FORMS,
            Self::Preview(_) => crate::model::kind::PREVIEW,
            Self::Lifecycle(_) => crate::model::kind::LIFECYCLE,
            Self::Config(_) => crate::model::kind::CONFIG,
            Self::Quality(_) => kind::QUALITY,
            Self::Alerts(_) => kind::ALERTS,
            Self::Migration(_) => kind::MIGRATION,
            Self::Adoption(_) => kind::ADOPTION,
            Self::Beside(_) => kind::BESIDE,
            Self::Replacement(_) => kind::REPLACEMENT,
            Self::Upgrade(_) => kind::UPGRADE,
            Self::Music(_) => kind::MUSIC,
            Self::Trace(_) => kind::TRACE,
            Self::Household(_) => kind::HOUSEHOLD,
            Self::Hosting(_) => kind::HOSTING,
            Self::FrontDoor(_) => kind::FRONT_DOOR,
            Self::Stuck(_) => kind::STUCK,
            Self::Word(_) => kind::WORD,
            Self::Glossary(_) => kind::GLOSSARY,
            Self::Clients(_) => kind::CLIENTS,
            Self::Invited(_) => kind::INVITATION,
            Self::Removed(_) => kind::REMOVAL,
            Self::Outbound(_) => crate::model::kind::OUTBOUND,
            Self::Credentials(_) => crate::model::kind::CREDENTIALS,
            Self::Stored(_) => crate::model::kind::STORED,
            Self::Space(_) => kind::SPACE,
            Self::Letting(_) => kind::STOP_SEEDING,
            Self::Bandwidth(_) => kind::BANDWIDTH,
            Self::Status(_) => crate::model::kind::STATUS,
            Self::Doctor(_) => kind::DOCTOR,
            Self::Repair(_) => kind::REPAIR,
            Self::Undo(_) => kind::UNDO,
            Self::Seed(_) => kind::SEED,
            Self::Reset(_) => kind::RESET,
            Self::Uninstall(_) => kind::UNINSTALL,
            Self::Wizard(_) => kind::WIZARD,
            Self::Backup(_) => kind::BACKUP,
            Self::Support(_) => kind::BUNDLE,
            Self::Archives(_) => kind::ARCHIVES,
            Self::Restore(_) => kind::RESTORE,
            Self::Watch(_) => kind::WATCH,
            Self::Walkthrough(_) => kind::WALKTHROUGH,
        };
        Envelope::new(kind, self)
    }
}

impl serde::Serialize for Outcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Version(report) => report.serialize(serializer),
            Self::Forms(report) => report.serialize(serializer),
            Self::Preview(plan) => plan.serialize(serializer),
            Self::Lifecycle(report) => report.serialize(serializer),
            Self::Config(report) => report.serialize(serializer),
            Self::Quality(report) => report.serialize(serializer),
            Self::Alerts(report) => report.serialize(serializer),
            Self::Migration(report) => report.serialize(serializer),
            Self::Adoption(report) => report.serialize(serializer),
            Self::Beside(report) => report.serialize(serializer),
            Self::Replacement(report) => report.serialize(serializer),
            Self::Upgrade(report) => report.serialize(serializer),
            Self::Music(report) => report.serialize(serializer),
            Self::Trace(report) => report.serialize(serializer),
            Self::Household(report) => report.serialize(serializer),
            Self::Hosting(report) => report.serialize(serializer),
            Self::FrontDoor(report) => report.serialize(serializer),
            Self::Stuck(report) => report.serialize(serializer),
            Self::Word(term) => term.serialize(serializer),
            Self::Glossary(report) => report.serialize(serializer),
            Self::Clients(report) => report.serialize(serializer),
            Self::Invited(report) => report.serialize(serializer),
            Self::Removed(report) => report.serialize(serializer),
            Self::Outbound(report) => report.serialize(serializer),
            Self::Credentials(inventory) => inventory.serialize(serializer),
            Self::Stored(report) => report.serialize(serializer),
            Self::Space(report) => report.serialize(serializer),
            Self::Letting(offer) => offer.serialize(serializer),
            Self::Bandwidth(report) => report.serialize(serializer),
            Self::Status(report) => report.serialize(serializer),
            Self::Doctor(report) => report.serialize(serializer),
            Self::Repair(report) => report.serialize(serializer),
            Self::Undo(report) => report.serialize(serializer),
            Self::Seed(report) => report.serialize(serializer),
            Self::Reset(report) => report.serialize(serializer),
            Self::Uninstall(report) => report.serialize(serializer),
            Self::Wizard(report) => report.serialize(serializer),
            Self::Backup(report) => report.serialize(serializer),
            Self::Support(report) => report.serialize(serializer),
            Self::Archives(listing) => listing.serialize(serializer),
            Self::Restore(report) => report.serialize(serializer),
            Self::Watch(report) => report.serialize(serializer),
            Self::Walkthrough(report) => report.serialize(serializer),
        }
    }
}
