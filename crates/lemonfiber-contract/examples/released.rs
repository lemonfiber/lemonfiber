//! Rewrites `contract/capabilities/released.json` from the index this build publishes,
//! refusing to where it would drop an operation without a new major; with `check`, says
//! whether the committed file is already that index.

use std::path::Path;

use lemonfiber_contract::documents::DIRECTORY;
use lemonfiber_contract::released::{Released, RELEASED};

fn main() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(DIRECTORY)
        .join(RELEASED);
    let committed: Released = std::fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    let published = Released::published();
    if std::env::args().nth(1).as_deref() == Some("check") {
        if committed != published {
            eprintln!(
                "{} is not the index this build publishes: run `just released-capabilities`",
                path.display()
            );
            std::process::exit(1);
        }
        return;
    }
    let dropped = committed.dropped_by(&published);
    if !dropped.is_empty() {
        eprintln!(
            "refusing to rewrite {}:\n{}",
            path.display(),
            dropped.join("\n")
        );
        std::process::exit(1);
    }
    let mut text = serde_json::to_string_pretty(&published).unwrap_or_default();
    text.push('\n');
    if let Err(error) = std::fs::write(&path, text) {
        eprintln!("{}: {error}", path.display());
        std::process::exit(1);
    }
}
