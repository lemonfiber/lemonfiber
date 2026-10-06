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

/// The three key commands reach the core as the operator at this terminal asks them.
#[test]
fn the_key_commands_reach_the_core_as_the_operator_asks_them() {
    use lemonfiber::cli::{KeyCommand, RawKey};
    use lemonfiber_core::keys::run::Asked as Keyed;
    use lemonfiber_core::keys::Minter;
    let minted = super::keyed(RawKey {
        action: KeyCommand::Mint {
            name: "ha".to_owned(),
            scope: "act".to_owned(),
            purpose: "home-assistant".to_owned(),
        },
    });
    assert_eq!(
        minted,
        Command::Keys(Keyed::Mint {
            name: "ha".to_owned(),
            scope: "act".to_owned(),
            purpose: "home-assistant".to_owned(),
            by: Minter::Operator,
        })
    );
    assert_eq!(
        super::keyed(RawKey {
            action: KeyCommand::List,
        }),
        Command::Keys(Keyed::List)
    );
    assert_eq!(
        super::keyed(RawKey {
            action: KeyCommand::Revoke {
                name: "ha".to_owned(),
            },
        }),
        Command::Keys(Keyed::Revoke {
            name: "ha".to_owned(),
            by: Minter::Operator,
        })
    );
}

/// Pairing and the certificate reach the core as asked.
#[test]
fn pairing_and_the_certificate_reach_the_core_as_asked() {
    use lemonfiber::cli::{CompanionCommand, RawCompanion};
    use lemonfiber_core::companion::Asked as Paired;
    assert_eq!(
        super::paired(&RawCompanion {
            action: CompanionCommand::Pair,
        }),
        Command::Companion(Paired::Pair)
    );
    assert_eq!(
        super::paired(&RawCompanion {
            action: CompanionCommand::Certificate { confirm: true },
        }),
        Command::Companion(Paired::Certificate { confirm: true })
    );
}
