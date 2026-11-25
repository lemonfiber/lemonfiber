//! Every read, asked of a world that holds something for it to answer.

use std::sync::Arc;
use std::time::Duration;

use lemonfiber_core::backup::{self, Scope};
use lemonfiber_core::config::paths::Paths;
use lemonfiber_core::copying::pacing::Stopwatch;
use lemonfiber_core::copying::{Meter as _, Starting, What};
use lemonfiber_fixtures::ports::{Stopped, Waited};

use crate::house::{house, HELD, MEMBER};
use crate::surface::Surface;
use crate::world::{World, AT};
use crate::Seen;

/// The reads a stack answers on its own, each as it is asked.
const STACK: &[&str] = &[
    "/api/version",
    "/api/forms",
    "/api/forms?form=library",
    "/api/status",
    "/api/services?form=library",
    "/api/checks",
    "/api/storage",
    "/api/hosting",
    "/api/front-door",
    "/api/stack",
    "/api/notify",
    "/api/news",
    "/api/stuck",
    "/api/boot",
    "/api/watcher",
    "/api/queue",
    "/api/config",
    "/api/quality",
    "/api/explain",
    "/api/explain?word=indexer",
    "/api/backups",
    "/api/outbound",
    "/api/provenance",
    "/api/catalogue",
    "/api/stored",
    "/api/uninstall?tier=services",
    "/api/space",
    "/api/bandwidth",
    "/api/clients",
    "/api/alerts",
    "/api/credentials",
    "/api/migration",
    "/api/history",
    "/api/update",
    "/api/update?what=self",
    "/api/plugins",
    "/api/plugins/catalogue",
    "/api/wiring",
    "/api/settings",
    "/api/settled",
    "/api/trace?term=Sintel",
    "/api/logs?service=sonarr",
    "/api/logs?service=sonarr&follow=true",
];

/// The reads about the household, each as it is asked.
fn household() -> Vec<String> {
    vec![
        "/api/requests".to_owned(),
        format!("/api/requests?member={MEMBER}"),
        format!("/api/held?member={MEMBER}"),
        format!("/api/held/{HELD}?member={MEMBER}"),
        format!("/api/watching?member={MEMBER}"),
        format!("/api/askable?member={MEMBER}&term=Matrix&kind=movies"),
        format!("/api/askable/kinds?member={MEMBER}"),
        "/api/playing".to_owned(),
        format!("/api/channels?member={MEMBER}"),
        format!("/api/guide?member={MEMBER}"),
        format!("/api/notify/mine?member={MEMBER}"),
    ]
}

/// Every read asked, and what each answered kept.
pub(crate) async fn every(seen: &mut Seen) -> Option<()> {
    copying(seen).await?;
    reads(
        seen,
        World::named("stack-reads"),
        STACK.iter().map(ToString::to_string),
    )
    .await?;
    reads(
        seen,
        World::named("household-reads").housing(house()),
        household(),
    )
    .await
}

/// Each of `paths` read in `world`, and what each answered kept.
/// A backup held to a limit, part-way through the services, asked of while it runs.
async fn copying(seen: &mut Seen) -> Option<()> {
    let world = World::named("copying");
    let paths = Paths::at(world.root(), world.root());
    let stopwatch = Waited::started();
    let meter = Starting::on(
        Stopped::at(AT),
        Arc::clone(&stopwatch) as Arc<dyn Stopwatch>,
        Some("9f2c1a7e04b3d815".to_owned()),
        Some(50 << 20),
    )
    .kept_under(&paths)
    .begin(
        What::Backup,
        Scope::WholeStack,
        2 << 30,
        backup::plan(&paths, &Scope::WholeStack, &[]).areas(),
    );
    meter.writing("services/sonarr");
    stopwatch.pass(Duration::from_secs(14));
    meter.wrote(700 << 20);
    let asked = reads(seen, world, ["/api/copying".to_owned()]).await;
    drop(meter);
    asked
}

async fn reads(
    seen: &mut Seen,
    world: World,
    paths: impl IntoIterator<Item = String>,
) -> Option<()> {
    let surface = Surface::over(world).await?;
    for path in paths {
        seen.answered(surface.asked("GET", &path, None).await.envelopes);
    }
    Some(())
}
