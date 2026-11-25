//! What a capture and a restore tell the copy's meter as they write.

use std::sync::{Arc, Mutex};

use lemonfiber_core::archive::{Archive, Reader};
use lemonfiber_core::backup::{self, Manifest, Scope};
use lemonfiber_core::copying::Meter;
use lemonfiber_fixtures::ports::STAMPED;

use super::{install, install_empty, scratch, unmetered, write_file};
use crate::archive::Tar;

/// What a meter was told, in the order it was told it.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Heard {
    Whole(u64),
    Writing(String),
    Wrote(u64),
}

/// A meter keeping what it was told.
#[derive(Default)]
pub(super) struct Told(Mutex<Vec<Heard>>);

impl Told {
    fn heard(&self) -> Vec<Heard> {
        self.0.lock().map(|heard| heard.clone()).unwrap_or_default()
    }

    fn wrote(&self) -> u64 {
        self.heard()
            .iter()
            .map(|heard| match heard {
                Heard::Wrote(bytes) => *bytes,
                Heard::Whole(_) | Heard::Writing(_) => 0,
            })
            .sum()
    }

    fn writing(&self) -> Vec<String> {
        self.heard()
            .into_iter()
            .filter_map(|heard| match heard {
                Heard::Writing(entry) => Some(entry),
                Heard::Whole(_) | Heard::Wrote(_) => None,
            })
            .collect()
    }

    fn tell(&self, heard: Heard) {
        if let Ok(mut told) = self.0.lock() {
            told.push(heard);
        }
    }
}

impl Meter for Told {
    fn whole(&self, bytes: u64) {
        self.tell(Heard::Whole(bytes));
    }

    fn writing(&self, entry: &str) {
        self.tell(Heard::Writing(entry.to_owned()));
    }

    fn wrote(&self, bytes: u64) {
        self.tell(Heard::Wrote(bytes));
    }
}

#[tokio::test]
async fn a_capture_tells_its_meter_each_entry_and_every_byte_the_room_check_measured() {
    let root = scratch("metered-capture");
    let paths = install(&root);
    write_file(
        &paths.service_config().join("radarr/radarr.db"),
        &"r".repeat(5000),
    );
    let plan = backup::plan(&paths, &Scope::WholeStack, &[]);
    let manifest = Manifest::describe(&plan, "0.3.0", STAMPED, "/srv/media");
    let dest = paths.backups().join("metered.tar.gz");
    let told = Arc::new(Told::default());

    let space = Tar.space(&paths.backups(), &plan.items).await;
    let written = Tar
        .write(
            &dest,
            &manifest,
            &plan.items,
            Arc::clone(&told) as Arc<dyn Meter>,
        )
        .await;

    assert!(written.is_ok(), "{written:?}");
    assert_eq!(space.ok().map(|space| space.needed), Some(told.wrote()));
    let entries = told.writing();
    for entry in [
        "config/.env",
        "services/sonarr",
        "services/radarr/radarr.db",
        "stack",
    ] {
        assert!(
            entries.iter().any(|told| told == entry),
            "{entry} not told: {entries:?}"
        );
    }
}

#[tokio::test]
async fn a_restore_tells_its_meter_the_whole_first_and_then_all_of_it() {
    let root = scratch("metered-restore");
    let paths = install(&root);
    let plan = backup::plan(&paths, &Scope::WholeStack, &[]);
    let manifest = Manifest::describe(&plan, "0.3.0", STAMPED, "/srv/media");
    let dest = paths.backups().join("metered.tar.gz");
    assert!(Tar
        .write(&dest, &manifest, &plan.items, unmetered())
        .await
        .is_ok());
    let told = Arc::new(Told::default());

    let restored = install_empty(&scratch("metered-restore-into"));
    let extracted = Tar
        .extract(
            &dest,
            &backup::destinations(&restored),
            Arc::clone(&told) as Arc<dyn Meter>,
        )
        .await;

    assert!(extracted.is_ok(), "{extracted:?}");
    let heard = told.heard();
    let whole = match heard.first() {
        Some(Heard::Whole(whole)) => *whole,
        other => unreachable!("the whole was not told first: {other:?}"),
    };
    assert!(whole > 0);
    assert_eq!(
        told.wrote(),
        whole,
        "less or more was written than the whole"
    );
    assert!(told
        .writing()
        .iter()
        .any(|entry| entry == "services/sonarr/config.xml"));
}
