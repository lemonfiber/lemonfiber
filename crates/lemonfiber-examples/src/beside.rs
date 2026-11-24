//! The routes beside the reads and the actions, and what the stream says on its own.

use std::sync::Arc;

use lemonfiber_api::events::dashboard::Dashboard;
use lemonfiber_fixtures::passwords::a_password;
use lemonfiber_fixtures::phones::{a_phone_key, a_phone_secret};
use serde_json::{json, Value};

use lemonfiber::cli::RawSetup;
use lemonfiber::setting_up::{by_flags, SetupFlags};
use lemonfiber_core::app::{dispatch, Command};
use lemonfiber_core::companion::Asked as Paired;
use lemonfiber_core::config::paths::Paths;

use crate::companion;
use crate::surface::Surface;
use crate::world::World;
use crate::Seen;

/// Each route beside the reads and the actions, as it is asked, in order.
fn every_route() -> Vec<(&'static str, &'static str, Option<Value>)> {
    vec![
        ("GET", "/api/capabilities", None),
        (
            "POST",
            "/api/session",
            Some(json!({ "password": a_password() })),
        ),
        ("GET", "/api/setup", None),
        (
            "POST",
            "/api/keys",
            Some(
                json!({ "name": "home", "scope": "read", "purpose": "home-assistant", "password": a_password() }),
            ),
        ),
        (
            "POST",
            "/api/keys",
            Some(
                json!({ "name": "mcp", "scope": "read", "purpose": "mcp", "claim": true, "password": a_password() }),
            ),
        ),
        ("GET", "/api/keys", None),
        (
            "POST",
            "/api/push-endpoints",
            Some(json!({
                "name": "a-phone",
                "endpoint": "https://push.example/send/a-phone",
                "p256dh": a_phone_key(),
                "auth": a_phone_secret(),
            })),
        ),
        ("GET", "/api/push-endpoints", None),
        ("GET", "/api/device-keys", None),
    ]
}

/// Every route beside the reads and the actions asked, what the stream says when the
/// dashboard is gathered, and what setup by flags concludes, kept.
pub(crate) async fn every(seen: &mut Seen) -> Option<()> {
    routes(seen).await?;
    set_up(seen).await
}

/// Every route beside the reads and the actions asked, and what the stream says when the
/// dashboard is gathered, kept.
async fn routes(seen: &mut Seen) -> Option<()> {
    let world = World::named("beside");
    companion::serving(&world);
    let mut surface = Surface::over(world).await?;
    let dashboard = Dashboard::against(Arc::new(surface.world().ctx.clone()));
    surface.live().refresh(&dashboard).await;
    for (method, path, body) in every_route() {
        seen.answered(surface.asked(method, path, body.as_ref()).await.envelopes);
    }
    surface.live().refresh(&dashboard).await;
    let _ = tokio::fs::remove_dir_all(surface.world().root().join("data")).await;
    surface.live().refresh(&dashboard).await;
    seen.answered(surface.heard().await);
    seen.answered(surface.resumed("0-0").await);
    let replacing = Command::Companion(Paired::Certificate { confirm: false });
    let replaced = dispatch(replacing, &surface.world().ctx).await;
    seen.answered(
        replaced
            .ok()
            .and_then(|outcome| surface.world().read(outcome.envelope().to_json())),
    );
    Some(())
}

/// What setup by flags concludes, kept.
async fn set_up(seen: &mut Seen) -> Option<()> {
    let world = World::named("setup");
    let paths = Paths::at(world.root(), world.root());
    let given = RawSetup {
        yes: true,
        protocols: Some("torrent".to_owned()),
        data_location: Some(world.root().join("media")),
        library: Some("docker".to_owned()),
        household: Some(true),
        autostart: Some(false),
        ..RawSetup::default()
    };
    let flags = SetupFlags::parse(given).ok()?;
    let concluded = by_flags(&world.ctx, &paths, flags).await;
    seen.answered(
        concluded
            .ok()
            .and_then(|report| world.read(report.to_json())),
    );
    Some(())
}
