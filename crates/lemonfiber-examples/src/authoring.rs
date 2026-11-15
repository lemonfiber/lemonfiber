//! What a plugin's author asks of this build, about one plugin the bundle carries, copied
//! into a world of its own: what it claims, what its conformance recordings come to, what
//! is said about the images it pins, and its run proofs held to what was recorded.

use std::path::Path;

use lemonfiber_core::app::authoring::Asked;
use lemonfiber_core::app::{dispatch, Command};
use lemonfiber_core::plugin::Retaking;

use crate::world::World;
use crate::Seen;

/// The plugin asked about, as the bundle carries it.
const PLUGIN: &str = "flaresolverr";

/// Every file beneath `from` copied beneath `to`, directories included.
fn copied(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copied(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

/// Each question an author asks, of the plugin at `path`.
fn every_question(path: &Path) -> [Asked; 4] {
    let path = path.to_path_buf();
    [
        Asked::Claims { path: path.clone() },
        Asked::Conform { path: path.clone() },
        Asked::Provenance {
            path: path.clone(),
            keys: Vec::new(),
        },
        Asked::Record {
            path,
            taking: Retaking {
                check: true,
                ..Retaking::default()
            },
        },
    ]
}

/// Every question an author asks answered, and what each answer said kept.
pub(crate) async fn every(seen: &mut Seen) -> Option<()> {
    let world = World::named("authoring");
    let carried = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/plugins/bundle/plugins")
        .join(PLUGIN);
    let copy = world.root().join(PLUGIN);
    copied(&carried, &copy).ok()?;
    for asked in every_question(&copy) {
        let answered = dispatch(Command::Authoring(asked), &world.ctx).await;
        seen.answered(
            answered
                .ok()
                .and_then(|outcome| world.read(outcome.envelope().to_json())),
        );
    }
    Some(())
}
