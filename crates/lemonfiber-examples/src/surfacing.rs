//! What the web surface asks of the core on its own behalf: an event a service posted,
//! a member's grant renewed as their client speaks, and the lapsed streams closed.

use lemonfiber_core::app::{dispatch, Command, Surfacing};
use lemonfiber_core::config::paths::NOTIFY_EVENTS;
use lemonfiber_core::notify::events::{Event, Happened, Telling};
use lemonfiber_core::ports::digest::sha256;

use crate::world::World;
use crate::Seen;

/// The service the event comes from.
const SERVICE: &str = "sonarr";

/// The key that service was handed, as the example presents it.
const PRESENTED: &str = "example-events-key";

/// The member whose client speaks.
const MEMBER: &str = "ada";

/// The three commands, each answered as the surface reads it.
fn every_command() -> [Command; 3] {
    [
        Command::Surface(Surfacing::Told(Telling {
            presented: PRESENTED.to_owned(),
            event: Event {
                event: Happened::Imported,
                reference: Some("42".to_owned()),
                name: "Arrival".to_owned(),
                reason: None,
                cleared: false,
            },
            from: None,
        })),
        Command::Surface(Surfacing::Spoke {
            member: MEMBER.to_owned(),
        }),
        Command::Surface(Surfacing::Untune),
    ]
}

/// Each answer, kept.
pub(crate) async fn every(seen: &mut Seen) -> Option<()> {
    let world = World::named("surfacing");
    let keys = serde_json::json!({
        "schema": 1,
        "written_by": "0.0.0",
        "keys": { SERVICE: sha256(PRESENTED) },
    });
    std::fs::write(world.root().join(NOTIFY_EVENTS), keys.to_string()).ok()?;
    for command in every_command() {
        let answered = dispatch(command, &world.ctx).await;
        seen.answered(
            answered
                .ok()
                .and_then(|outcome| world.read(outcome.envelope().to_json())),
        );
    }
    Some(())
}
