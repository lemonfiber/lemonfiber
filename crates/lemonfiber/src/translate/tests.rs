use lemonfiber_core::app::plugins;
use lemonfiber_core::app::{Allowance, Command, Filling, Linking, QualityAction, Setting, Waiting};
use lemonfiber_core::audio::Format;
use lemonfiber_core::quality::Preset;

use super::{alerts, diagnosing, moving, named, narrowed, Authoring};
use super::{
    bundling, configuration, credentials, hosting, household, invitation, letting, quality,
    restarting, sharing, traced, Asking, Destination, Hostable, Keeping, Wanted,
};
use crate::exit::USAGE;
use lemonfiber::cli::{
    AlertCommand, Asked, ConfigAction, HostingCommand, HouseholdCommand, Kept, QualityCommand,
    RawAllowance, RawBandwidth, RawCredentials, RawUnrated, UpdateCommand,
};
use lemonfiber_core::alert::Appetite;
use lemonfiber_core::app::{AlertAction, BandwidthAsked};
use lemonfiber_core::app::{Answer, Arranged, Chosen, Decision};
use lemonfiber_core::asking::Policy;
use lemonfiber_core::bundle::Filenames;
use lemonfiber_core::doctor::Narrowing;
use lemonfiber_core::ports::service::{Quota, Unrated};

mod choices;
mod household;
mod machine;
