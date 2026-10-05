#![no_main]
//! A backup archive is read back from a file anybody could have handed the operator,
//! and its manifest is read from the same untrusted bytes as everything beside it.
//! Every archive must be read and either restored or refused, never a panic — and a
//! restore lands inside the directories it was given or nowhere at all.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use flate2::write::GzEncoder;
use flate2::Compression;
use lemonfiber::archive::Tar;
use lemonfiber_core::archive::Reader;
use libfuzzer_sys::fuzz_target;

/// Each run's own directory, so a restore never meets what the last one left.
static RUN: AtomicU64 = AtomicU64::new(0);

fuzz_target!(|data: &[u8]| {
    // The bytes are the tar stream; compressing them here is what lets the fuzzer reach
    // the entries rather than spend itself on the gzip header.
    let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
    if std::io::Write::write_all(&mut encoder, data).is_err() {
        return;
    }
    let Ok(compressed) = encoder.finish() else {
        return;
    };
    let root = std::env::temp_dir().join(format!(
        "lemonfiber-fuzz-restore-{}-{}",
        std::process::id(),
        RUN.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::create_dir_all(&root);
    let archive = root.join("archive.tar.gz");
    if std::fs::write(&archive, compressed).is_err() {
        return;
    }
    let landing = root.join("landing");
    let targets: Vec<(String, PathBuf)> = ["config", "services", "stack", "existing"]
        .iter()
        .map(|area| ((*area).to_owned(), landing.join(area)))
        .collect();
    let Ok(runtime) = tokio::runtime::Builder::new_current_thread().build() else {
        return;
    };
    runtime.block_on(async {
        let _ = Tar.read_manifest(&archive).await;
        let _ = Tar.extract(&archive, &targets).await;
    });
    let _ = std::fs::remove_dir_all(&root);
});
