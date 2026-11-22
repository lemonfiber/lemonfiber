//! Every read, asked of a world that holds something for it to answer.

use crate::house::{house, HELD, MEMBER};
use crate::surface::Surface;
use crate::world::World;
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
