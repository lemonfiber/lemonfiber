//! What the command line asks the core for itself: whether a walkthrough is worth taking,
//! the logs read, what is still coming down, how the web surface is served, and a new
//! password for it.

use lemonfiber_core::app::{dispatch, Command, ReadingLogs};
use lemonfiber_core::companion::served::Served;
use lemonfiber_fixtures::passwords::another_password;
use lemonfiber_fixtures::typing::Typing;

use crate::world::World;
use crate::Seen;

/// The port the web surface is served on.
const PORT: u16 = 8443;

/// The lines read from the end of each service's log.
const TAIL: u32 = 20;

/// Each command the command line dispatches for itself.
fn every_command() -> [Command; 5] {
    [
        Command::WalkthroughOffer,
        Command::Logs(ReadingLogs {
            forms: Vec::new(),
            services: Vec::new(),
            tail: TAIL,
            follow: false,
        }),
        Command::InFlight {
            forms: Vec::new(),
            services: Vec::new(),
        },
        Command::Served(Served {
            port: PORT,
            encrypted: true,
            network: false,
        }),
        Command::Password,
    ]
}

/// Every command the command line dispatches for itself answered, and what each said kept.
pub(crate) async fn every(seen: &mut Seen) -> Option<()> {
    let world = World::named("terminal");
    crate::companion::certified(&world.root().join("state").join("companion"));
    let ctx = world
        .ctx
        .clone()
        .with_asking(Typing::answering_each(vec![another_password()]));
    for command in every_command() {
        let answered = dispatch(command, &ctx).await;
        seen.answered(
            answered
                .ok()
                .and_then(|outcome| world.read(outcome.envelope().to_json())),
        );
    }
    Some(())
}
