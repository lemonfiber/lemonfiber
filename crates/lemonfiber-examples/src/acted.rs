//! Every action, sent to a world of its own, and what its work came to.

use serde_json::{json, Map, Value};

use crate::companion;
use lemonfiber_fixtures::passwords::another_password;

use crate::house::{house, CHANNEL, FOUND, HELD, MEMBER, SEEDED};
use crate::surface::Surface;
use crate::world::World;
use crate::Seen;

/// The action whose work is released before it ran.
const RELEASED: &str = "up";

/// Whom an action is about: the stack alone, or the household it serves.
#[derive(Clone, Copy)]
enum About {
    /// The stack and its services.
    Stack,
    /// The household, through its media server and its request service.
    Household,
    /// The stack with nothing running, a backup already taken whose name the action is
    /// sent.
    Archived,
    /// The stack with nothing running.
    Stopped,
    /// The stack, its data location gone after the first look at it.
    Unmounted,
    /// The stack served encrypted on the network, with the certificate a phone pins.
    Served,
    /// The household, the action sent by a member signed in on a session of their own.
    Member,
    /// The stack, its engine writing a line of a pull for every command.
    Pulling,
}

/// Each action, whom it is about, and what it is sent with.
fn every_action() -> Vec<(&'static str, About, Value)> {
    stack()
        .into_iter()
        .chain(household())
        .chain(operating())
        .collect()
}

/// Each action about running the stack, its quality, its migration and its removal.
fn stack() -> Vec<(&'static str, About, Value)> {
    use About::{Pulling, Stack};
    vec![
        ("up", Stack, json!({ "forms": ["library"] })),
        ("down", Stack, json!({ "forms": ["library"] })),
        ("switch", Stack, json!({ "forms": ["library"] })),
        ("restart", Stack, json!({ "forms": ["library"] })),
        ("pull", Pulling, json!({ "forms": ["library"] })),
        (
            "config-set",
            Stack,
            json!({ "key": "LEMONFIBER_EXPLANATIONS", "value": "off" }),
        ),
        (
            "quality-set",
            Stack,
            json!({ "preset": "balanced", "media_type": "tv", "confirm": true }),
        ),
        ("quality-reapply", Stack, json!({})),
        ("quality-upgrade", Stack, json!({ "confirm": true })),
        ("seed", Stack, json!({})),
        ("adopt", Stack, json!({})),
        ("migrate-adopt", Stack, json!({ "confirm": true })),
        ("migrate-beside", Stack, json!({ "confirm": true })),
        ("migrate-replace", Stack, json!({})),
        ("migrate-import", Stack, json!({ "confirm": true })),
        ("reset", Stack, json!({})),
        ("forget", Stack, json!({ "confirm": true })),
        ("uninstall", Stack, json!({ "tier": "services" })),
        ("space", Stack, json!({})),
    ]
}

/// Each action about the household and what it downloads, and the backup and update
/// between them.
fn household() -> Vec<(&'static str, About, Value)> {
    use About::{Household, Member, Served, Stack, Stopped};
    vec![
        ("stop-seeding", Household, json!({ "download": SEEDED })),
        (
            "bandwidth",
            Household,
            json!({ "line": "60MiB/6MiB", "down": "37%", "up": "37%" }),
        ),
        ("downloads-pause", Household, json!({})),
        ("downloads-resume", Household, json!({})),
        ("update", Stack, json!({})),
        ("backup", Stopped, json!({})),
        ("invite", Household, json!({ "name": "bo" })),
        ("remove", Household, json!({ "name": MEMBER })),
        ("reissue", Household, json!({ "name": MEMBER })),
        ("companion-pair", Served, json!({})),
        (
            "household-allow",
            Household,
            json!({ "name": MEMBER, "policy": "within-a-limit", "requests": 5, "days": 30 }),
        ),
        (
            "household-approve",
            Household,
            json!({ "request": "music-7" }),
        ),
        (
            "household-decline",
            Household,
            json!({ "request": "music-7", "reason": "there is no room this month" }),
        ),
        ("household-handoff", Household, json!({ "name": MEMBER })),
        (
            "grant",
            Household,
            json!({ "name": MEMBER, "device": "a-phone-2468" }),
        ),
        (
            "play",
            Member,
            json!({ "id": HELD, "device": "a-phone-2468" }),
        ),
        ("tune", Member, json!({ "id": CHANNEL })),
        (
            "watched",
            Household,
            json!({ "name": MEMBER, "id": HELD, "position": 1234 }),
        ),
        (
            "ask",
            Household,
            json!({ "name": MEMBER, "id": FOUND, "kind": "movies" }),
        ),
    ]
}

/// Each action about operating the stack: notifying, support, hosting, repairs, plugins
/// and settings.
fn operating() -> Vec<(&'static str, About, Value)> {
    use About::{Archived, Household, Stack, Unmounted};
    vec![
        ("notify-choose", Stack, json!({ "backends": ["home-ntfy"] })),
        (
            "push-woken",
            Household,
            json!({ "name": MEMBER, "id": HELD }),
        ),
        ("support", Stack, json!({})),
        ("restore", Archived, json!({})),
        ("watch", Unmounted, json!({ "forms": ["library"] })),
        (
            "hosting-install",
            Stack,
            json!({ "kept": "watch", "forms": ["library"] }),
        ),
        ("hosting-remove", Stack, json!({ "kept": "watch" })),
        ("walkthrough", Household, json!({ "item": "Sintel" })),
        ("diagnose", Stack, json!({ "disruptive": true })),
        ("repair", Stack, json!({})),
        ("undo", Stack, json!({})),
        (
            "accept",
            Stack,
            json!({ "check": "storage.quality-headroom" }),
        ),
        ("search", Household, json!({ "term": "The Expanse" })),
        (
            "wiring-fill",
            Stack,
            json!({ "capability": "indexer.search", "service": "nzbhydra2" }),
        ),
        (
            "plugin-install",
            Stack,
            json!({ "source": "./plugins/komga" }),
        ),
        (
            "plugin-update",
            Stack,
            json!({ "plugin": "komga", "source": "./plugins/komga" }),
        ),
        ("plugin-remove", Stack, json!({ "plugin": "komga" })),
        ("plugin-prove", Stack, json!({ "plugin": "komga" })),
        (
            "plugin-run",
            Stack,
            json!({ "plugin": "komga", "recipe": "rescan" }),
        ),
        (
            "credentials-revoke",
            Stack,
            json!({ "plugin": "komga", "credential": "SONARR_API_KEY" }),
        ),
        ("internal-certificate-replace", Stack, json!({})),
        (
            "settings-set",
            Stack,
            json!({ "values": { "gluetun:countries": "Belgium" } }),
        ),
        (
            "settings-clear",
            Stack,
            json!({ "settings": ["gluetun:countries"] }),
        ),
    ]
}

/// Each action sent again with what reaches a kind its example does not: music chosen
/// rather than a resolution.
fn beyond() -> Vec<(&'static str, About, Value)> {
    vec![(
        "quality-set",
        About::Stack,
        json!({ "preset": "lossless", "media_type": "music", "confirm": true }),
    )]
}

/// Every action taken, each in a world of its own, and what each answered and came to
/// kept; then each sent again as [`beyond`] lists, and what that came to kept.
pub(crate) async fn every(seen: &mut Seen) -> Option<()> {
    for (action, about, arguments) in every_action() {
        let (sent, answered) = taken(seen, action, about, &arguments).await?;
        seen.acted(action, sent, answered);
    }
    for (action, about, arguments) in beyond() {
        taken(seen, action, about, &arguments).await?;
    }
    released(seen).await
}

/// The work [`RELEASED`] starts, released before it ran, and what releasing it answered
/// kept.
async fn released(seen: &mut Seen) -> Option<()> {
    let surface = Surface::over(World::named("released")).await?;
    let started = surface
        .asked(
            "POST",
            &format!("/api/actions/{RELEASED}"),
            Some(&json!({ "forms": ["library"] })),
        )
        .await;
    let job = started
        .envelopes
        .first()
        .and_then(|envelope| envelope.pointer("/data/job"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    seen.answered(
        surface
            .asked("DELETE", &format!("/api/jobs/{job}"), None)
            .await
            .envelopes,
    );
    Some(())
}

/// `action` sent `arguments` in a world `about` makes, what it answered and came to
/// kept, and what it was sent and answered.
async fn taken(
    seen: &mut Seen,
    action: &str,
    about: About,
    arguments: &Value,
) -> Option<(Map<String, Value>, Value)> {
    let world = World::named(&format!("action-{action}"));
    let world = match about {
        About::Stack => world,
        About::Household => world.housing(house()),
        About::Member => world.housing(house()).passing(),
        About::Stopped | About::Archived => world.stopped(),
        About::Unmounted => world.unmounting(),
        About::Pulling => world.pulling(),
        About::Served => {
            companion::serving(&world);
            world
        }
    };
    let mut sent = arguments.as_object().cloned().unwrap_or_else(Map::new);
    let mut surface = Surface::over(world).await?;
    if matches!(about, About::Archived) {
        sent.insert("archive".to_owned(), archived(&surface).await);
    }
    let body = Value::Object(sent.clone());
    let path = format!("/api/actions/{action}");
    let member = if matches!(about, About::Member) {
        surface.signed_in(MEMBER, &another_password()).await.0
    } else {
        None
    };
    let answer = match member.as_deref() {
        Some(token) => surface.asked_as(token, "POST", &path, Some(&body)).await,
        None => surface.asked("POST", &path, Some(&body)).await,
    };
    let answered = answer.envelopes.first().cloned().unwrap_or_default();
    let came_to = surface.redeemed(&answered).await;
    seen.answered(answer.envelopes);
    seen.answered(came_to);
    seen.answered(surface.heard().await);
    Some((sent, answered))
}

/// The name of a backup taken through `surface`, as a restore is sent it.
async fn archived(surface: &Surface) -> Value {
    let answer = surface
        .asked("POST", "/api/actions/backup", Some(&json!({})))
        .await;
    let started = answer.envelopes.first().cloned().unwrap_or_default();
    let came_to = surface.redeemed(&started).await;
    let named = came_to
        .first()
        .and_then(|report| report.pointer("/data/path"))
        .and_then(Value::as_str)
        .and_then(|path| path.rsplit('/').next())
        .unwrap_or_default();
    Value::from(named)
}
